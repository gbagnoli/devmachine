use anyhow::{anyhow, Context, Result};
use clap::Parser;
use skillet_cli_common::hosts::ApplyPhase;
use std::{fs, io::Write as _, path::PathBuf, process::Command};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

mod secret_delivery;
mod secret_output;
mod vm;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// Manage host secrets backed by `KeePassXC`
    #[command(name = "secrets", visible_alias = "secret")]
    Secret {
        #[command(subcommand)]
        command: SecretCommands,
    },
    /// Apply host configuration
    Apply {
        #[arg(long, value_enum, default_value_t = ApplyPhase::Full)]
        phase: ApplyPhase,
        #[arg(long)]
        host: Option<String>,
        #[arg(long)]
        host_file: Option<PathBuf>,
        /// Optional diagnostic recording of resource operations
        #[arg(long)]
        record: Option<PathBuf>,
    },
    /// Run the real-runtime smoke scenario against a disposable VM
    Test {
        #[command(subcommand)]
        command: TestCommands,
    },
}

#[derive(clap::Subcommand, Debug)]
enum SecretCommands {
    /// Audit required and unused entries without changing the vault or providers
    Check(SecretCheckArgs),
    /// Deliver credentials for a configured host service
    Deliver(SecretDeliverArgs),
    /// Verify and cache the `KeePassXC` password for this session
    Unlock(SecretUnlockArgs),
    /// Remove the cached vault password from the kernel keyring
    Lock(SecretLockArgs),
}

#[derive(clap::Args, Debug)]
struct SecretCheckArgs {
    /// Check requirements for this host; defaults to all declared profiles
    #[arg(long)]
    host: Option<String>,
    #[arg(long, value_enum, default_value_t = UiEnvironmentName::Production)]
    environment: UiEnvironmentName,
    #[arg(long)]
    database: Option<PathBuf>,
    #[arg(long)]
    key_file: Option<PathBuf>,
}

#[derive(clap::Args, Debug)]
struct SecretUnlockArgs {
    #[arg(long)]
    database: Option<PathBuf>,
    #[arg(long)]
    key_file: Option<PathBuf>,
}

#[derive(clap::Args, Debug)]
struct SecretLockArgs {
    #[arg(long)]
    database: Option<PathBuf>,
}

#[derive(clap::Args, Debug)]
struct SecretDeliverArgs {
    hostname: String,
    #[arg(value_parser = ["pihole", "tailscale", "caddy", "ddns", "datadog"])]
    service: String,
    #[arg(long)]
    database: Option<PathBuf>,
    #[arg(long)]
    key_file: Option<PathBuf>,
    /// `KeePassXC` environment to use for service configuration
    #[arg(long, value_enum, default_value_t = UiEnvironmentName::Production)]
    environment: UiEnvironmentName,
    #[arg(long)]
    target: String,
    #[arg(long, default_value_t = 22)]
    port: u16,
    #[arg(long)]
    identity: PathBuf,
    #[arg(long)]
    known_hosts: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
enum UiEnvironmentName {
    Production,
    Test,
}

impl UiEnvironmentName {
    fn policy(self) -> skillet_workstation::provisioning_policy::ProvisioningPolicy {
        use skillet_workstation::provisioning_policy::{Environment, ProvisioningPolicy};
        match self {
            Self::Production => ProvisioningPolicy::new(Environment::Production),
            Self::Test => ProvisioningPolicy::new(Environment::Test),
        }
    }
}

#[derive(clap::Subcommand, Debug)]
enum TestCommands {
    /// Apply a selected configuration phase twice in a disposable Podman container
    Run(ContainerArgs),
    /// Exercise the real-systemd VM scenario using an explicit disposable SSH target
    Smoke(SmokeArgs),
    /// Create or destroy a disposable host VM
    Vm {
        #[command(subcommand)]
        command: VmCommands,
    },
}

#[derive(clap::Subcommand, Debug)]
enum VmCommands {
    /// Provision a disposable host VM and wait for it to become ready
    Create(VmCreateArgs),
    /// Destroy a disposable host VM and remove its temporary key and artifacts
    Destroy(VmDestroyArgs),
    /// List available host templates and their recorded disposable VMs
    List(VmListArgs),
    /// Inspect the UUID and disks recorded for an owned disposable VM
    Status(VmTargetArgs),
    /// Reboot a retained, owned disposable VM
    Reboot(VmTargetArgs),
    /// Open an interactive SSH shell to a retained, owned disposable VM
    Ssh(VmSshArgs),
    /// Read bootstrap and Skillet journals from a retained VM
    Logs(VmTargetArgs),
    /// Verify signed boot and apply the captured base/user environment
    Ready(VmTargetArgs),
    /// Compatibility adapter for the former `RUN_DIR` readiness helper
    #[command(hide = true)]
    ReadyDirectory(VmDirectoryArgs),
    /// Provision Pi-hole and enroll the disposable VM in Tailscale
    Provision(VmProvisionArgs),
    /// Install the current host binary on a retained disposable VM
    Update(VmDestroyArgs),
}

#[derive(clap::Args, Debug)]
struct VmCreateArgs {
    hostname: String,
    instance: String,
    #[arg(long, default_value_t = 2201)]
    port: u16,
    /// Use a specific Fedora `CoreOS` QEMU image instead of selecting the cached image
    #[arg(long)]
    image: Option<PathBuf>,
}

#[derive(clap::Args, Debug)]
struct VmTargetArgs {
    hostname: String,
    instance: String,
}

#[derive(clap::Args, Debug)]
struct VmSshArgs {
    #[command(flatten)]
    target: VmTargetArgs,
    /// Forward a workstation localhost TCP port to guest localhost (repeatable)
    #[arg(long, value_name = "LOCAL:GUEST")]
    forward: Vec<skillet_vm::transport::LoopbackForward>,
}

#[derive(clap::Args, Debug)]
struct VmDirectoryArgs {
    run_dir: PathBuf,
}

#[derive(clap::Args, Debug)]
struct VmDestroyArgs {
    hostname: String,
    instance: String,
    #[arg(long)]
    database: Option<PathBuf>,
    #[arg(long)]
    key_file: Option<PathBuf>,
}

#[derive(clap::Args, Debug)]
// These CLI switches select independent optional provisioning operations.
#[allow(clippy::struct_excessive_bools)]
struct VmProvisionArgs {
    hostname: String,
    instance: String,
    /// Also provision private UI DNS, Caddy, and disposable Cloudflare credentials
    #[arg(long)]
    with_ui: bool,
    /// Provision public DDNS records from test-environment `KeePassXC` config
    #[arg(long)]
    with_ddns: bool,
    /// Deliver the shared Datadog key and enable test-tagged VM telemetry
    #[arg(long)]
    with_datadog: bool,
    #[arg(long)]
    database: Option<PathBuf>,
    #[arg(long)]
    key_file: Option<PathBuf>,
    /// Replace the disposable password and restart its consumer
    #[arg(long)]
    rotate: bool,
}

#[derive(clap::Args, Debug)]
struct VmListArgs {
    hostname: Option<String>,
}

#[derive(clap::Args, Debug)]
struct ContainerArgs {
    hostname: String,
    /// Configuration phase (defaults to base because full apply may require host mounts)
    #[arg(long, value_enum, default_value = "base")]
    phase: ApplyPhase,
    #[arg(long, default_value = "fedora:latest")]
    image: String,
    #[arg(long)]
    inspect: bool,
}

#[derive(clap::Args, Debug)]
struct SmokeArgs {
    /// Host profile from the recorded disposable VM
    hostname: String,
    /// Recorded disposable VM instance
    #[arg(long, default_value = "smoke")]
    instance: String,
    /// Optional target override; must match the recorded disposable VM
    #[arg(long)]
    target: Option<String>,
    /// SSH port
    #[arg(long)]
    port: Option<u16>,
    /// SSH private key (defaults to the key generated for `test vm create`)
    #[arg(long)]
    identity: Option<PathBuf>,
    /// Also verify and repeat-apply the selected host's provisioned applications
    #[arg(long)]
    with_applications: bool,
    /// Stop after persisting the pre-reboot checkpoint to exercise recovery
    #[arg(long, hide = true)]
    interrupt_before_reboot: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let subscriber = FmtSubscriber::builder()
        .with_max_level(if args.verbose {
            Level::DEBUG
        } else {
            Level::INFO
        })
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .context("setting default subscriber failed")?;

    match args.command {
        Commands::Secret {
            command: SecretCommands::Check(args),
        } => secret_delivery::check_vault(&args)?,
        Commands::Secret {
            command: SecretCommands::Deliver(args),
        } => {
            secret_delivery::deliver_from_vault(&args)?;
        }
        Commands::Secret {
            command: SecretCommands::Unlock(args),
        } => secret_delivery::unlock_vault(args.database.as_deref(), args.key_file.as_deref())?,
        Commands::Secret {
            command: SecretCommands::Lock(args),
        } => secret_delivery::lock_vault(args.database.as_deref())?,
        Commands::Apply {
            phase,
            host,
            host_file,
            record,
        } => {
            let mut hostname = host.unwrap_or_else(|| "agent".to_string());
            if let Some(path) = host_file {
                hostname = std::fs::read_to_string(path)
                    .context("Failed to read host file")?
                    .trim()
                    .to_string();
            }
            skillet_cli_common::handle_host_apply(
                &hostname,
                phase,
                record,
                |system, files, credentials| {
                    skillet_cli_common::hosts::apply_host_phase(
                        &hostname,
                        phase,
                        system,
                        files,
                        credentials,
                    )
                    .map_err(|error| error.to_string())
                },
            )
            .map_err(|error| anyhow!("Failed to apply configuration: {error}"))?;
        }
        Commands::Test {
            command: TestCommands::Run(args),
        } => run_container_test(&args)?,
        Commands::Test {
            command: TestCommands::Smoke(args),
        } => run_smoke(&args)?,
        Commands::Test {
            command: TestCommands::Vm { command },
        } => run_vm_command(command)?,
    }
    Ok(())
}

fn run_vm_command(command: VmCommands) -> Result<()> {
    match command {
        VmCommands::Create(args) => run_vm_create(&args)?,
        VmCommands::Destroy(args) => run_vm_destroy(&args)?,
        VmCommands::List(args) => run_vm_list(&args)?,
        VmCommands::Status(args) => vm::status(&args)?,
        VmCommands::Reboot(args) => vm::reboot(&args)?,
        VmCommands::Ssh(args) => vm::ssh(&args)?,
        VmCommands::Logs(args) => vm::logs(&args)?,
        VmCommands::Ready(args) => vm::ready(&args)?,
        VmCommands::ReadyDirectory(args) => vm::ready_directory(&args)?,
        VmCommands::Provision(args) => secret_delivery::provision_vm(&args)?,
        VmCommands::Update(args) => run_vm_update(&args)?,
    }
    Ok(())
}

fn run_smoke(args: &SmokeArgs) -> Result<()> {
    let root = workspace_root()?;
    let profile = skillet_hosts::profile_for_name(&args.hostname)
        .ok_or_else(|| anyhow!("unknown host profile: {}", args.hostname))?;
    if profile.signed_image.is_none() || !profile.requires_data_mount {
        return Err(anyhow!(
            "host {} does not declare the boot and storage capabilities required by VM smoke",
            args.hostname
        ));
    }
    let butane = butane_root()?;
    let run_identity = skillet_vm::RunIdentity::new(&args.hostname, &args.instance)?;
    let store = skillet_vm::ManifestStore::new(&butane.join("runs"), skillet_vm::current_uid())?;
    let _lock = store.lock(&run_identity)?;
    let run = store.load(&run_identity)?;
    if run.phase != skillet_vm::Phase::Ready {
        return Err(anyhow!(
            "run `{} {}` must be ready before smoke",
            args.hostname,
            args.instance
        ));
    }
    let deployed_hashes = run.deployed.as_ref().ok_or_else(|| {
        anyhow!("ready VM has no recorded deployed artifact hashes; rerun `test vm ready`")
    })?;
    let target = format!("{}@{}", run.ssh.user, run.ssh.address);
    if args
        .target
        .as_ref()
        .is_some_and(|provided| provided != &target)
    {
        return Err(anyhow!(
            "--target must match the SSH target recorded in the VM manifest: {target}"
        ));
    }
    if args.port.is_some_and(|port| port != run.ssh.port) {
        return Err(anyhow!(
            "--port must match the SSH port recorded in the VM manifest: {}",
            run.ssh.port
        ));
    }
    let identity = if let Some(path) = &args.identity {
        if !path.is_file() {
            return Err(anyhow!("SSH key not found at {}", path.display()));
        }
        if std::fs::canonicalize(path)? != std::fs::canonicalize(&run.ssh.identity)? {
            return Err(anyhow!(
                "--identity must match the SSH key recorded in the VM manifest"
            ));
        }
        path.clone()
    } else {
        run.ssh.identity.clone()
    };
    if !identity.is_file() {
        return Err(anyhow!(
            "SSH key not found at {}; create the VM with `skillet test vm create {} {}`",
            identity.display(),
            args.hostname,
            args.instance
        ));
    }
    let host_binary = format!("/var/usrlocal/bin/skillet-{}", run.identity.host());
    let credentials_required = profile.requires_full_apply_credentials();
    let binary = std::env::current_exe().context("locating the running Skillet binary failed")?;
    let (fixture_artifact, fixture_hash, cargo_target, cargo_profile) =
        build_smoke_fixture(&root, &binary)?;
    info!(
        target = cargo_target,
        profile = cargo_profile,
        generic_sha256 = deployed_hashes.generic,
        host_sha256 = deployed_hashes.host,
        fixture_sha256 = fixture_hash,
        "Resolved smoke binaries"
    );
    let guest_script = root.join("integration_tests/smoke-guest.sh");
    if !guest_script.is_file() {
        return Err(anyhow!(
            "smoke guest assertions not found at {}",
            guest_script.display()
        ));
    }
    SmokeLifecycle {
        butane: &butane,
        store: &store,
        run: &run,
        fixture: &fixture_artifact,
        fixture_hash: &fixture_hash,
        guest_script: &guest_script,
        host_binary: &host_binary,
        credentials_required,
        with_applications: args.with_applications,
        interrupt_before_reboot: args.interrupt_before_reboot,
    }
    .run()
}

struct SmokeLifecycle<'a> {
    butane: &'a std::path::Path,
    store: &'a skillet_vm::ManifestStore,
    run: &'a skillet_vm::VmRun,
    fixture: &'a std::path::Path,
    fixture_hash: &'a str,
    guest_script: &'a std::path::Path,
    host_binary: &'a str,
    credentials_required: bool,
    with_applications: bool,
    interrupt_before_reboot: bool,
}

impl SmokeLifecycle<'_> {
    fn run(&self) -> Result<()> {
        use skillet_vm::{
            backend::{VirshBackend, VmBackend},
            transport::{HostKeyPolicy, OwnershipCheckedTransport},
        };
        let backend = VirshBackend::for_run(self.run, &self.butane.join("bin/virsh"))?;
        let ownership_run = self.run.clone();
        let ownership = || -> skillet_vm::Result<()> {
            backend
                .inspect(&ownership_run)?
                .ok_or_else(|| skillet_vm::Error::Invalid("owned VM domain is absent".into()))?
                .validate_owned(&ownership_run)
        };
        ownership()?;

        let base =
            skillet_vm::transport::SshTransport::new(self.run.ssh.clone(), HostKeyPolicy::Verify)?;
        let transport = OwnershipCheckedTransport::new(&base, &ownership);
        self.install_and_verify(&transport)?;
        let boot_before = guest_text(&transport, "cat", &["/proc/sys/kernel/random/boot_id"])?;
        if self.with_applications {
            self.write_persistence_marker(&transport)?;
        }
        self.run_guest_phase(&transport, "before-reboot")?;

        let run = self.store.mark_reboot_pending(&self.run.identity)?;
        if self.interrupt_before_reboot {
            return Err(anyhow!(
                "injected interruption after persisting the pre-reboot checkpoint"
            ));
        }
        ownership()?;
        backend.reboot(&run)?;
        ownership()?;
        Self::wait_for_new_boot(&run, &ownership, &boot_before)?;
        self.run_guest_phase(&transport, "after-reboot")?;

        if self.with_applications {
            self.check_applications(&transport, &ownership)?;
            smoke_guest_command(
                &transport,
                "sudo",
                &["-n", "rm", "-f", "--", &self.persistence_marker_path()],
            )?;
        }

        vm::ready_locked(self.butane, self.store, &self.run.identity)?;
        if self.store.load(&self.run.identity)?.phase != skillet_vm::Phase::Ready {
            return Err(anyhow!(
                "VM readiness did not restore the Ready phase after smoke"
            ));
        }
        Ok(())
    }

    fn install_and_verify(
        &self,
        transport: &impl skillet_vm::transport::GuestTransport,
    ) -> Result<()> {
        for (source, destination) in [
            (self.fixture, "/var/tmp/skillet-smoke-fixture"),
            (self.guest_script, "/var/tmp/skillet-smoke-guest.sh"),
        ] {
            transport.upload(source, destination)?;
        }
        smoke_guest_command(
            transport,
            "sudo",
            &[
                "-n",
                "install",
                "-m",
                "0755",
                "/var/tmp/skillet-smoke-fixture",
                "/var/usrlocal/bin/skillet-smoke-fixture",
            ],
        )?;
        verify_guest_hash(
            transport,
            "/var/usrlocal/bin/skillet-smoke-fixture",
            self.fixture_hash,
        )?;
        let deployed = self
            .run
            .deployed
            .as_ref()
            .ok_or_else(|| anyhow!("VM has no deployed artifact hashes"))?;
        verify_guest_hash(transport, "/var/usrlocal/bin/skillet", &deployed.generic)?;
        verify_guest_hash(
            transport,
            &format!("/var/usrlocal/bin/skillet-{}", self.run.identity.host()),
            &deployed.host,
        )
    }

    fn run_guest_phase(
        &self,
        transport: &impl skillet_vm::transport::GuestTransport,
        phase: &str,
    ) -> Result<()> {
        let credentials = if self.credentials_required {
            "yes"
        } else {
            "no"
        };
        smoke_guest_command(
            transport,
            "sudo",
            &[
                "-n",
                "bash",
                "/var/tmp/skillet-smoke-guest.sh",
                phase,
                self.host_binary,
                credentials,
            ],
        )
    }

    fn persistence_marker_path(&self) -> String {
        format!(
            "/var/lib/data/.skillet-smoke-{}-{}.marker",
            self.run.identity.host(),
            self.run.identity.instance()
        )
    }

    fn write_persistence_marker(
        &self,
        transport: &impl skillet_vm::transport::GuestTransport,
    ) -> Result<()> {
        use skillet_vm::transport::GuestCommand;
        const MARKER: &[u8] = b"skillet disposable application persistence check\n";
        smoke_guest_command(
            transport,
            "sudo",
            &["-n", "rm", "-f", "--", &self.persistence_marker_path()],
        )?;
        let output = transport.execute(
            &GuestCommand {
                program: "sudo",
                arguments: &["-n", "tee", &self.persistence_marker_path()],
            },
            Some(MARKER),
        )?;
        if !output.status.success() {
            return Err(anyhow!("writing disposable persistence marker failed"));
        }
        Ok(())
    }

    fn verify_persistence_marker(
        &self,
        transport: &impl skillet_vm::transport::GuestTransport,
    ) -> Result<()> {
        use sha2::{Digest as _, Sha256};
        let expected = hex::encode(Sha256::digest(
            b"skillet disposable application persistence check\n",
        ));
        verify_guest_hash(transport, &self.persistence_marker_path(), &expected)
    }

    fn check_applications(
        &self,
        transport: &impl skillet_vm::transport::GuestTransport,
        ownership: &dyn Fn() -> skillet_vm::Result<()>,
    ) -> Result<()> {
        let profile = skillet_hosts::profile_for_name(self.run.identity.host())
            .ok_or_else(|| anyhow!("unknown host acceptance profile"))?;
        let plan = profile.acceptance_plan();
        if plan.requires_data_mount {
            let mount = guest_text(
                transport,
                "findmnt",
                &["-n", "-o", "FSTYPE,FSROOT", "--mountpoint", "/var/lib/data"],
            )?;
            require_data_mount(&mount)?;
        }
        self.verify_persistence_marker(transport)?;
        let first = application_snapshot(transport, ownership, &plan)?;
        self.repeat_host_apply(transport, &profile)?;
        let second = application_snapshot(transport, ownership, &plan)?;
        if first != second {
            let changed = first
                .iter()
                .zip(&second)
                .filter(|(before, after)| before != after)
                .map(|(before, _)| application_state_label(before))
                .collect::<Vec<_>>();
            return Err(anyhow!(
                "repeat host apply changed declared application runtime state: {}",
                changed.join(", ")
            ));
        }
        tracing::info!(
            services = ?plan.services.iter().map(|service| service.unit.as_str()).collect::<Vec<_>>(),
            "Host application acceptance passed"
        );
        println!(
            "Application acceptance passed for: {}",
            plan.services
                .iter()
                .map(|service| service.unit.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        Ok(())
    }

    fn repeat_host_apply(
        &self,
        transport: &impl skillet_vm::transport::GuestTransport,
        profile: &skillet_hosts::HostProfile,
    ) -> Result<()> {
        if profile.requires_full_apply_credentials() {
            smoke_guest_command(
                transport,
                "sudo",
                &["-n", "systemctl", "restart", "skillet-full-apply.service"],
            )?;
        } else {
            smoke_guest_command(
                transport,
                "sudo",
                &["-n", self.host_binary, "apply", "--phase", "full"],
            )?;
        }
        if profile.services.iter().any(|service| service.ui.is_some()) {
            smoke_guest_command(
                transport,
                "sudo",
                &["-n", "systemctl", "restart", "skillet-caddy-apply.service"],
            )?;
        }
        Ok(())
    }

    fn wait_for_new_boot(
        run: &skillet_vm::VmRun,
        ownership: &dyn Fn() -> skillet_vm::Result<()>,
        boot_before: &str,
    ) -> Result<()> {
        use skillet_vm::transport::{HostKeyPolicy, OwnershipCheckedTransport};
        use std::time::{Duration, Instant};
        let mut probe =
            skillet_vm::transport::SshTransport::new(run.ssh.clone(), HostKeyPolicy::Verify)?;
        probe.timeout = Duration::from_secs(15);
        let checked = OwnershipCheckedTransport::new(&probe, ownership);
        let deadline = Instant::now() + Duration::from_mins(4);
        loop {
            ownership()?;
            match guest_text(&checked, "cat", &["/proc/sys/kernel/random/boot_id"]) {
                Ok(boot_after) if boot_after != boot_before => return Ok(()),
                Ok(_) | Err(_) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_secs(5));
                }
                Ok(_) => return Err(anyhow!("VM did not return from reboot within 240 seconds")),
                Err(error) => {
                    ownership()?;
                    if Instant::now() >= deadline {
                        return Err(anyhow!(
                            "VM did not return from reboot within 240 seconds: {error}"
                        ));
                    }
                    std::thread::sleep(Duration::from_secs(5));
                }
            }
        }
    }
}

fn application_state_label(state: &str) -> &str {
    let mut fields = state.split(':');
    match fields.next() {
        Some("container" | "unit" | "config" | "bind" | "listener") => {
            fields.next().unwrap_or("unknown")
        }
        Some(name) => name,
        None => "unknown",
    }
}

fn application_snapshot(
    transport: &impl skillet_vm::transport::GuestTransport,
    ownership: &dyn Fn() -> skillet_vm::Result<()>,
    plan: &skillet_hosts::HostAcceptancePlan,
) -> Result<Vec<String>> {
    application_snapshot_with_probe_timeout(
        transport,
        ownership,
        plan,
        std::time::Duration::from_mins(2),
    )
}

fn application_snapshot_with_probe_timeout(
    transport: &impl skillet_vm::transport::GuestTransport,
    ownership: &dyn Fn() -> skillet_vm::Result<()>,
    plan: &skillet_hosts::HostAcceptancePlan,
    probe_timeout: std::time::Duration,
) -> Result<Vec<String>> {
    let mut snapshot = Vec::new();
    for service in &plan.services {
        ownership()?;
        let unit_state = check_unit_active(transport, &service.unit, probe_timeout)?;
        if unit_state != "active" {
            return Err(anyhow!(
                "declared service unit {} is not active",
                service.unit
            ));
        }
        snapshot.push(format!("unit:{}:{unit_state}", service.unit));
        if let Some(container) = &service.container {
            snapshot.push(check_container(transport, service, container)?);
        }
        for listener in &service.listeners {
            check_listener(transport, *listener, probe_timeout)?;
            snapshot.push(format!(
                "listener:{:?}:{}",
                listener.protocol, listener.port
            ));
        }
        if let Some(health) = check_health(transport, service.health_probe)? {
            snapshot.push(health);
        }
    }
    Ok(snapshot)
}

fn check_unit_active(
    transport: &impl skillet_vm::transport::GuestTransport,
    unit: &str,
    timeout: std::time::Duration,
) -> Result<String> {
    use std::time::{Duration, Instant};
    let deadline = Instant::now() + timeout;
    loop {
        let output = transport.execute(
            &skillet_vm::transport::GuestCommand {
                program: "systemctl",
                arguments: &["is-active", unit],
            },
            None,
        )?;
        let last_state = String::from_utf8(output.stdout)
            .context("systemd service state was not UTF-8")?
            .trim()
            .to_string();
        if output.status.success() && last_state == "active" {
            return Ok(last_state);
        }
        if Instant::now() >= deadline {
            return Err(anyhow!(
                "declared service unit {unit} did not become active within 120 seconds (last state: {last_state})"
            ));
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

fn require_data_mount(observed: &str) -> Result<()> {
    if observed != "btrfs /data" {
        return Err(anyhow!(
            "host data mount differs from the profile expectation"
        ));
    }
    Ok(())
}

fn check_container(
    transport: &impl skillet_vm::transport::GuestTransport,
    service: &skillet_hosts::AcceptanceService,
    container: &str,
) -> Result<String> {
    use std::fmt::Write as _;
    let inspect = guest_text(
        transport,
        "sudo",
        &[
            "-n",
            "podman",
            "inspect",
            "--format",
            "{{.State.Running}}|{{.HostConfig.NetworkMode}}|{{.Id}}|{{json .NetworkSettings.Networks}}|{{range .Mounts}}{{.Source}};{{end}}",
            container,
        ],
    )?;
    let fields = inspect.splitn(5, '|').collect::<Vec<_>>();
    if fields.len() != 5 || fields[0] != "true" {
        return Err(anyhow!("declared container {container} is not running"));
    }
    if let Some(expected) = &service.network_mode {
        let actual_networks: serde_json::Value = serde_json::from_str(fields[3])
            .context("Podman returned invalid network inspection JSON")?;
        let network_matches = if expected == "host" {
            fields[1] == "host"
        } else {
            actual_networks
                .as_object()
                .is_some_and(|networks| networks.contains_key(expected))
        };
        if !network_matches {
            return Err(anyhow!(
                "container {container} has an unexpected network attachment"
            ));
        }
    }
    let mut state = format!(
        "container:{container}:{}:{}:{}",
        fields[1], fields[2], fields[3]
    );
    for path in &service.bind_paths {
        if !fields[4].split(';').any(|source| source == path) {
            return Err(anyhow!(
                "container {container} does not mount declared path {path}"
            ));
        }
        if let Some(owner_expectation) = &service.owner {
            use skillet_hosts::AcceptanceOwner;
            let (format, expected) = match owner_expectation {
                AcceptanceOwner::Named { user, group } => {
                    let uid = guest_text(transport, "id", &["-u", user])?
                        .parse::<u32>()
                        .context("resolving declared data owner UID")?;
                    let group_record = guest_text(transport, "getent", &["group", group])?;
                    let gid = group_record
                        .split(':')
                        .nth(2)
                        .ok_or_else(|| anyhow!("resolving declared data owner group GID"))?
                        .parse::<u32>()
                        .context("parsing declared data owner group GID")?;
                    ("%u:%g", format!("{uid}:{gid}"))
                }
                AcceptanceOwner::Numeric { uid, gid } => ("%u:%g", format!("{uid}:{gid}")),
            };
            let owner = guest_text(transport, "stat", &["-c", format, path])?;
            if owner != expected {
                return Err(anyhow!(
                    "data path ownership differs from declaration: {path} (expected {expected}, found {owner})"
                ));
            }
            write!(state, ";bind:{path}:{owner}")?;
        }
    }
    let unit_name = service
        .unit
        .strip_suffix(".service")
        .ok_or_else(|| anyhow!("container service unit has an invalid name"))?;
    let config_path = format!("/etc/containers/systemd/{unit_name}.container");
    let config_hash = guest_text(transport, "sha256sum", &[&config_path])?;
    write!(state, ";config:{config_hash}")?;
    Ok(state)
}

fn check_listener(
    transport: &impl skillet_vm::transport::GuestTransport,
    listener: skillet_hosts::AcceptanceListener,
    timeout: std::time::Duration,
) -> Result<()> {
    use skillet_hosts::ListenerProtocol;
    use std::time::{Duration, Instant};
    let protocol = match listener.protocol {
        ListenerProtocol::Tcp => "-lnt",
        ListenerProtocol::Udp => "-lnu",
    };
    let port = format!(":{}", listener.port);
    let deadline = Instant::now() + timeout;
    loop {
        let listening = guest_text(
            transport,
            "sudo",
            &["-n", "ss", "-H", protocol, "sport", "=", &port],
        )?;
        if !listening.is_empty() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(anyhow!(
                "declared listener on port {} did not appear within 120 seconds",
                listener.port
            ));
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

fn check_health(
    transport: &impl skillet_vm::transport::GuestTransport,
    probe: Option<skillet_hosts::HealthProbe>,
) -> Result<Option<String>> {
    use skillet_hosts::HealthProbe;
    match probe {
        Some(HealthProbe::Pihole) => {
            smoke_guest_command(
                transport,
                "sudo",
                &["-n", "podman", "exec", "pihole", "pihole", "status"],
            )?;
            Ok(Some("health:pihole:running".into()))
        }
        Some(HealthProbe::Tailscale) => {
            let status = guest_text(
                transport,
                "sudo",
                &[
                    "-n",
                    "podman",
                    "exec",
                    "tailscale",
                    "tailscale",
                    "status",
                    "--json",
                ],
            )?;
            let status: serde_json::Value =
                serde_json::from_str(&status).context("Tailscale returned invalid status JSON")?;
            if status
                .get("BackendState")
                .and_then(serde_json::Value::as_str)
                != Some("Running")
            {
                return Err(anyhow!("Tailscale is not connected"));
            }
            Ok(Some("health:tailscale:running".into()))
        }
        None => Ok(None),
    }
}

fn smoke_guest_command(
    transport: &impl skillet_vm::transport::GuestTransport,
    program: &str,
    arguments: &[&str],
) -> Result<()> {
    let output = transport.execute(
        &skillet_vm::transport::GuestCommand { program, arguments },
        None,
    )?;
    std::io::stdout().write_all(&output.stdout)?;
    std::io::stderr().write_all(&output.stderr)?;
    if !output.status.success() {
        return Err(anyhow!(
            "guest smoke command {program} failed with status {:?}",
            output.status.code()
        ));
    }
    Ok(())
}

fn guest_text(
    transport: &impl skillet_vm::transport::GuestTransport,
    program: &str,
    arguments: &[&str],
) -> Result<String> {
    let output = transport.execute(
        &skillet_vm::transport::GuestCommand { program, arguments },
        None,
    )?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let command = std::iter::once(program)
            .chain(arguments.iter().copied())
            .collect::<Vec<_>>()
            .join(" ");
        return Err(anyhow!(
            "guest probe `{command}` failed with status {:?}{}",
            output.status.code(),
            if detail.is_empty() {
                String::new()
            } else {
                format!(": {detail}")
            }
        ));
    }
    String::from_utf8(output.stdout)
        .context("guest probe output was not UTF-8")
        .map(|value| value.trim().to_owned())
}

fn verify_guest_hash(
    transport: &impl skillet_vm::transport::GuestTransport,
    path: &str,
    expected: &str,
) -> Result<()> {
    let output = guest_text(transport, "sha256sum", &[path])?;
    if output.split_whitespace().next() != Some(expected) {
        return Err(anyhow!(
            "guest artifact at {path} does not match the recorded SHA-256"
        ));
    }
    Ok(())
}

fn artifact_target_profile(binary: &std::path::Path) -> Result<(&str, &str)> {
    let profile_directory = binary
        .parent()
        .ok_or_else(|| anyhow!("running binary has no parent directory"))?;
    let profile_name = profile_directory
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow!("cannot infer Cargo profile from running binary path"))?;
    let profile = if profile_name == "debug" {
        "dev"
    } else {
        profile_name
    };
    let target = profile_directory
        .parent()
        .and_then(|path| path.file_name())
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow!("cannot infer Cargo target from running binary path"))?;
    Ok((target, profile))
}

fn build_smoke_fixture<'a>(
    workspace: &std::path::Path,
    running_binary: &'a std::path::Path,
) -> Result<(PathBuf, String, &'a str, &'a str)> {
    let (target, profile) = artifact_target_profile(running_binary)?;
    let binary = skillet_vm::artifacts::build(&skillet_vm::artifacts::BuildRequest {
        workspace,
        packages: &["skillet-smoke-fixture"],
        target,
        profile,
    })?
    .remove("skillet-smoke-fixture")
    .ok_or_else(|| anyhow!("Cargo did not report the smoke fixture executable"))?;
    let hash = skillet_vm::delivery::sha256(&binary)?;
    Ok((binary, hash, target, profile))
}

fn run_vm_create(args: &VmCreateArgs) -> Result<()> {
    vm::create(args)
}

fn run_vm_destroy(args: &VmDestroyArgs) -> Result<()> {
    vm::destroy(args)
}

fn run_vm_update(args: &VmDestroyArgs) -> Result<()> {
    vm::update(args)
}

fn run_vm_list(args: &VmListArgs) -> Result<()> {
    vm::list(args)
}

fn butane_root() -> Result<PathBuf> {
    let root = workspace_root()?;
    let butane = root
        .parent()
        .ok_or_else(|| anyhow!("Skillet workspace has no parent directory"))?
        .join("butane");
    if !butane.join("bin/test-vm").is_file() || !butane.join("bin/test-vm-ready").is_file() {
        return Err(anyhow!(
            "test VM helpers not found under {}; check out the sibling butane directory",
            butane.display()
        ));
    }
    Ok(butane)
}

fn workspace_root() -> Result<PathBuf> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        if path.join("integration_tests").is_dir() && path.join("Cargo.toml").is_file() {
            return Ok(path);
        }
        if !path.pop() {
            return Err(anyhow!("could not locate workspace root"));
        }
    }
}

fn run_container_test(args: &ContainerArgs) -> Result<()> {
    skillet_hosts::profile_for_name(&args.hostname)
        .ok_or_else(|| anyhow!("unknown host profile: {}", args.hostname))?;
    let root = workspace_root()?;
    if !root
        .join("crates/hosts")
        .join(&args.hostname)
        .join("Cargo.toml")
        .is_file()
    {
        return Err(anyhow!(
            "host profile {} has no standalone host apply binary",
            args.hostname
        ));
    }
    let package = format!("skillet-{}", args.hostname);
    let artifacts = skillet_vm::artifacts::build(&skillet_vm::artifacts::BuildRequest {
        workspace: &root,
        packages: &[&package],
        target: "x86_64-unknown-linux-musl",
        profile: "dev",
    })?;
    let binary = artifacts
        .get(&package)
        .ok_or_else(|| anyhow!("Cargo did not report {package}"))?;
    info!(artifact = %binary.display(), "Using Cargo-reported host binary");
    let id = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    );
    let name = format!("skillet-test-{}-{id}", args.hostname);
    let creds = root
        .join("target")
        .join(format!("skillet-test-credentials-{id}"));
    fs::create_dir_all(&creds)?;
    fs::write(
        creds.join("pihole_web_password"),
        "skillet-test-dummy-secret",
    )?;
    fs::write(
        creds.join("tailscale_auth_key"),
        "skillet-test-dummy-auth-key",
    )?;
    let start = Command::new("podman")
        .args([
            "run",
            "-d",
            "--rm",
            "--security-opt",
            "label=disable",
            "--name",
            &name,
            "-v",
        ])
        .arg(format!("{}:/usr/bin/skillet:ro", binary.display()))
        .arg("-v")
        .arg(format!("{}:/run/credentials:ro", creds.display()))
        .args([
            "-e",
            "CREDENTIALS_DIRECTORY=/run/credentials",
            &args.image,
            "sleep",
            "infinity",
        ])
        .status();
    let start = match start {
        Ok(status) => status,
        Err(error) => {
            fs::remove_dir_all(&creds)
                .context("cleaning test credentials after Podman error failed")?;
            return Err(error).context("starting isolated Podman container failed");
        }
    };
    if !start.success() {
        fs::remove_dir_all(&creds)
            .context("cleaning test credentials after Podman start failure failed")?;
        return Err(anyhow!("Podman failed to start {name}"));
    }
    let result = run_twice_and_check(&name, args.phase, args.inspect);
    let cleanup = Command::new("podman")
        .args(["rm", "-f", &name])
        .status()
        .context("removing disposable test container failed");
    let credential_cleanup =
        fs::remove_dir_all(&creds).context("removing disposable test credentials failed");
    result?;
    if !cleanup?.success() {
        return Err(anyhow!("failed to remove own container {name}"));
    }
    credential_cleanup?;
    Ok(())
}

fn run_twice_and_check(name: &str, phase: ApplyPhase, inspect: bool) -> Result<()> {
    let entry = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/test_entrypoint.sh");
    let phase = match phase {
        ApplyPhase::Base => "base",
        ApplyPhase::Full => "full",
        ApplyPhase::Caddy => "caddy",
        ApplyPhase::Ddns => "ddns",
        ApplyPhase::Datadog => "datadog",
    };
    for round in ["first", "second"] {
        let status = Command::new("podman")
            .args(["exec", "-i", name, "/bin/sh", "-s", "--", round, phase])
            .stdin(fs::File::open(&entry)?)
            .status()
            .context("running apply in container failed")?;
        if !status.success() {
            return Err(anyhow!("{round} apply failed in {name}"));
        }
    }
    let first = podman_capture(
        name,
        "sed -n '1,/^SECOND$/p' /tmp/skillet-test.systemctl.log",
    )?;
    let second = podman_capture(
        name,
        "sed -n '/^SECOND$/,$p' /tmp/skillet-test.systemctl.log",
    )?;
    if first.trim().is_empty() {
        return Err(anyhow!("first apply emitted no systemd operations"));
    }
    if second
        .lines()
        .any(|line| line.starts_with("start ") || line.starts_with("restart "))
    {
        return Err(anyhow!("second apply restarted a service: {second}"));
    }
    if inspect {
        let status = Command::new("podman")
            .args(["exec", "-it", name, "/bin/bash"])
            .status()
            .context("starting inspection shell failed")?;
        if !status.success() {
            return Err(anyhow!("inspection shell failed"));
        }
    }
    info!("Both applies succeeded; repeat apply issued no service start/restart");
    Ok(())
}

fn podman_capture(name: &str, shell: &str) -> Result<String> {
    let out = Command::new("podman")
        .args(["exec", name, "/bin/sh", "-c", shell])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "failed to inspect apply operation log: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    String::from_utf8(out.stdout).context("operation log was not UTF-8")
}

#[cfg(test)]
mod tests {
    use super::Args;
    use clap::Parser;

    #[test]
    fn test_run_accepts_explicit_base_phase() {
        let parsed = Args::try_parse_from([
            "skillet",
            "test",
            "run",
            "beezelbot",
            "--phase",
            "base",
            "--image",
            "fedora:latest",
        ]);
        assert!(parsed.is_ok());
    }

    #[test]
    fn test_smoke_parses_without_binary_arguments() {
        let parsed = Args::try_parse_from([
            "skillet",
            "test",
            "smoke",
            "clamps",
            "--target",
            "core@192.0.2.5",
            "--with-applications",
            "--interrupt-before-reboot",
        ]);
        assert!(parsed.is_ok());
    }

    #[test]
    fn vm_create_parses_identity_and_optional_image() {
        let parsed = Args::try_parse_from([
            "skillet",
            "test",
            "vm",
            "create",
            "clamps",
            "smoke",
            "--image",
            "/tmp/fcos.qcow2",
        ]);
        assert!(parsed.is_ok());
    }

    #[test]
    fn secret_unlock_accepts_database_and_key_file_overrides() {
        let parsed = Args::try_parse_from([
            "skillet",
            "secret",
            "unlock",
            "--database",
            "/tmp/secrets.kdbx",
            "--key-file",
            "/tmp/secrets.keyx",
        ]);
        assert!(parsed.is_ok());
    }

    #[test]
    fn secret_unlock_uses_default_database_when_no_path_is_given() {
        let parsed = Args::try_parse_from(["skillet", "secret", "unlock"]);
        assert!(parsed.is_ok());
    }
}

#[cfg(test)]
#[path = "smoke_artifact_tests.rs"]
mod smoke_artifact_tests;

#[cfg(test)]
#[path = "application_acceptance_tests.rs"]
mod application_acceptance_tests;

#[cfg(test)]
mod vm_ssh_tests;

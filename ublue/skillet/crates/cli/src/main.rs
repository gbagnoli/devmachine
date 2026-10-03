use anyhow::{anyhow, Context, Result};
use clap::Parser;
use skillet_cli_common::hosts::ApplyPhase;
use std::{fs, path::PathBuf, process::Command};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

mod cloudflare;
mod secret_delivery;
mod tailscale;
mod test_fixture;
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
    /// Deliver credentials for a configured host service
    Deliver(SecretDeliverArgs),
    /// Remove the cached vault password from the kernel keyring
    Lock(SecretLockArgs),
}

#[derive(clap::Args, Debug)]
struct SecretLockArgs {
    #[arg(long)]
    database: Option<PathBuf>,
}

#[derive(clap::Args, Debug)]
struct SecretDeliverArgs {
    hostname: String,
    #[arg(value_parser = ["pihole", "tailscale", "caddy"])]
    service: String,
    #[arg(long)]
    database: Option<PathBuf>,
    #[arg(long)]
    key_file: Option<PathBuf>,
    /// `KeePassXC` environment to use for private UI configuration
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
    fn as_str(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Test => "test",
        }
    }

    fn acme_staging(self) -> bool {
        matches!(self, Self::Test)
    }
}

#[derive(clap::Subcommand, Debug)]
enum TestCommands {
    /// Apply a selected configuration phase twice in a disposable Podman container
    Run(ContainerArgs),
    /// Exercise the real-systemd VM scenario using an explicit disposable SSH target
    Smoke(SmokeArgs),
    /// Apply the disposable fixture used by the VM acceptance script
    #[command(hide = true)]
    FixtureApply,
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
    Ssh(VmTargetArgs),
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
struct VmProvisionArgs {
    hostname: String,
    instance: String,
    /// Also provision private UI DNS, Caddy, and disposable Cloudflare credentials
    #[arg(long)]
    with_ui: bool,
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
    /// Explicit disposable SSH target, USER@HOST
    #[arg(long)]
    target: Option<String>,
    /// SSH port
    #[arg(long)]
    port: Option<u16>,
    /// SSH private key (defaults to the key generated for `test vm create`)
    #[arg(long)]
    identity: Option<PathBuf>,
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
            command: SecretCommands::Deliver(args),
        } => {
            secret_delivery::deliver_from_vault(&args)?;
        }
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
            command: TestCommands::FixtureApply,
        } => skillet_cli_common::handle_apply("smoke fixture", None, |system, files| {
            test_fixture::apply(system, files).map_err(|error| error.to_string())
        })
        .map_err(|error| anyhow!("Failed to apply smoke fixture: {error}"))?,
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
    let run = store.load(&run_identity)?;
    if run.phase != skillet_vm::Phase::Ready {
        return Err(anyhow!(
            "run `{} {}` must be ready before smoke",
            args.hostname,
            args.instance
        ));
    }
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
    let script = root.join("integration_tests/smoke-ssh.sh");
    if !script.is_file() {
        return Err(anyhow!("smoke runner not found at {}", script.display()));
    }
    let status = std::process::Command::new("bash")
        .arg(script)
        .args(["--target", &target, "--disposable-target", "--port"])
        .arg(run.ssh.port.to_string())
        .args(["--host", run.identity.host()])
        .args([
            "--credentials-required",
            if credentials_required { "yes" } else { "no" },
        ])
        .args(["--binary"])
        .arg(&binary)
        .args(["--host-binary"])
        .arg(host_binary)
        .args(["--identity"])
        .arg(&identity)
        .status()
        .context("starting disposable-VM smoke runner failed")?;
    if !status.success() {
        return Err(anyhow!(
            "disposable-VM smoke scenario failed with status {status}"
        ));
    }
    Ok(())
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

fn vm_name(hostname: &str, instance: &str) -> Result<String> {
    Ok(skillet_vm::RunIdentity::new(hostname, instance)?.domain_name())
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
}

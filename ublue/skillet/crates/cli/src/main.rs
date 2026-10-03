use anyhow::{anyhow, Context, Result};
use clap::Parser;
use skillet_cli_common::hosts::ApplyPhase;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

mod cloudflare;
mod secret_delivery;
mod tailscale;
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
    /// Persist a recoverable VM creation intent for the source-tree helper
    #[command(hide = true)]
    Prepare(VmPrepareArgs),
    /// Record successful domain definition for the source-tree helper
    #[command(hide = true)]
    Defined(VmTargetArgs),
    /// Record successful VM start for the source-tree helper
    #[command(hide = true)]
    Started(VmTargetArgs),
    /// Render a native domain definition from the recorded run
    #[command(hide = true)]
    RenderDomain(VmRenderDomainArgs),
    /// Destroy a disposable host VM and remove its temporary key and artifacts
    Destroy(VmDestroyArgs),
    /// List available host templates and their recorded disposable VMs
    List(VmListArgs),
    /// Inspect the UUID and disks recorded for an owned disposable VM
    Status(VmTargetArgs),
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
}

#[derive(clap::Args, Debug)]
struct VmPrepareArgs {
    hostname: String,
    instance: String,
    #[arg(long, value_enum)]
    backend: VmBackendName,
    #[arg(long)]
    runtime_dir: PathBuf,
    #[arg(long, default_value = "qemu:///session")]
    uri: String,
    #[arg(long, default_value_t = 2201)]
    port: u16,
}

#[derive(clap::Args, Debug)]
struct VmRenderDomainArgs {
    hostname: String,
    instance: String,
    #[arg(long)]
    emulator: PathBuf,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum VmBackendName {
    Native,
    Flatpak,
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
    /// Host configuration to exercise (currently `clamps`)
    hostname: String,
    /// Explicit disposable SSH target, USER@HOST
    #[arg(long, default_value = "giacomo@127.0.0.1")]
    target: String,
    /// SSH port
    #[arg(long, default_value_t = 2201)]
    port: u16,
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
            let mut hostname = host.unwrap_or_else(|| "(Agent Mode)".to_string());
            if let Some(path) = host_file {
                hostname = std::fs::read_to_string(path)
                    .context("Failed to read host file")?
                    .trim()
                    .to_string();
            }
            skillet_cli_common::handle_apply(&hostname, record, |system, files| {
                skillet_cli_common::hosts::apply_host_phase(&hostname, phase, system, files)
                    .map_err(|error| error.to_string())
            })
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
        VmCommands::Prepare(args) => run_vm_prepare(&args)?,
        VmCommands::Defined(args) => vm::record_defined(&args)?,
        VmCommands::Started(args) => vm::record_started(&args)?,
        VmCommands::RenderDomain(args) => vm::render_domain(&args)?,
        VmCommands::Destroy(args) => run_vm_destroy(&args)?,
        VmCommands::List(args) => run_vm_list(&args)?,
        VmCommands::Status(args) => vm::status(&args)?,
        VmCommands::Ready(args) => vm::ready(&args)?,
        VmCommands::ReadyDirectory(args) => vm::ready_directory(&args)?,
        VmCommands::Provision(args) => secret_delivery::provision_vm(&args)?,
        VmCommands::Update(args) => run_vm_update(&args)?,
    }
    Ok(())
}

fn run_smoke(args: &SmokeArgs) -> Result<()> {
    if args.hostname != "clamps" {
        return Err(anyhow!(
            "the VM smoke scenario currently supports only hostname `clamps`"
        ));
    }
    let root = workspace_root()?;
    let identity = args.identity.clone().unwrap_or_else(|| {
        root.parent()
            .unwrap_or(&root)
            .join("butane/runs/clamps-test-smoke/ssh/id_ed25519")
    });
    if !identity.is_file() {
        return Err(anyhow!(
            "SSH key not found at {}; create the default VM with `skillet test vm create clamps smoke` or pass --identity",
            identity.display()
        ));
    }
    let binary = std::env::current_exe().context("locating the running Skillet binary failed")?;
    let script = root.join("integration_tests/smoke-ssh.sh");
    if !script.is_file() {
        return Err(anyhow!("smoke runner not found at {}", script.display()));
    }
    let status = std::process::Command::new("bash")
        .arg(script)
        .args(["--target", &args.target, "--disposable-target", "--port"])
        .arg(args.port.to_string())
        .args(["--binary"])
        .arg(&binary)
        .args(["--clamps-binary"])
        .arg("/var/usrlocal/bin/skillet-clamps")
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
    let name = vm_name(&args.hostname, &args.instance)?;
    if !(2200..=2299).contains(&args.port) {
        return Err(anyhow!("VM SSH port must be between 2200 and 2299"));
    }
    let butane = butane_root()?;
    let helper = butane.join("bin/test-vm");
    let port = args.port.to_string();
    let binary = std::env::current_exe().context("locating the running Skillet binary failed")?;
    run_helper_with_binary(
        &helper,
        &[&args.hostname, "create", &args.instance, "--port", &port],
        &binary,
    )?;

    let run_dir = butane.join("runs").join(&name);
    if let Err(error) = vm::ready(&VmTargetArgs {
        hostname: args.hostname.clone(),
        instance: args.instance.clone(),
    }) {
        return Err(anyhow!(
            "VM created but readiness failed; inspect it with `test-vm {} logs {}` or destroy it with `skillet test vm destroy {} {}`: {error}",
            args.hostname, args.instance, args.hostname, args.instance
        ));
    }
    info!(
        "Disposable VM {} is ready at giacomo@127.0.0.1:{}; smoke key: {}",
        name,
        args.port,
        run_dir.join("ssh/id_ed25519").display()
    );
    Ok(())
}

fn run_vm_prepare(args: &VmPrepareArgs) -> Result<()> {
    let identity = skillet_vm::RunIdentity::new(&args.hostname, &args.instance)?;
    let root = butane_root()?.join("runs");
    let source_commit = Command::new("git")
        .args(["-C"])
        .arg(workspace_root()?)
        .args(["rev-parse", "--verify", "HEAD"])
        .output()
        .context("reading the source revision for VM creation failed")?;
    if !source_commit.status.success() {
        return Err(anyhow!("git could not identify the source revision"));
    }
    let source_commit = String::from_utf8(source_commit.stdout)
        .context("git returned a non-UTF-8 source revision")?
        .trim()
        .to_owned();
    let backend = match args.backend {
        VmBackendName::Native => skillet_vm::Backend::Native,
        VmBackendName::Flatpak => skillet_vm::Backend::Flatpak,
    };
    let run = skillet_vm::ManifestStore::new(&root, skillet_vm::current_uid())?.prepare_intent(
        &identity,
        skillet_vm::Connection {
            backend,
            uri: args.uri.clone(),
            runtime_dir: args.runtime_dir.clone(),
        },
        args.port,
        &source_commit,
    )?;
    println!("{}", run.uuid);
    Ok(())
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

fn run_helper_with_binary(path: &Path, args: &[&str], binary: &Path) -> Result<()> {
    let status = Command::new(path)
        .env("SKILLET_BINARY", binary)
        .args(args)
        .status()
        .with_context(|| format!("running {} failed", path.display()))?;
    if !status.success() {
        return Err(anyhow!("{} failed with status {status}", path.display()));
    }
    Ok(())
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
    if !matches!(args.hostname.as_str(), "beezelbot" | "clamps") {
        return Err(anyhow!(
            "unsupported host {}; expected beezelbot or clamps",
            args.hostname
        ));
    }
    let root = workspace_root()?;
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
}

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
enum TestCommands {
    /// Apply configuration twice in a fresh disposable Podman container
    Run(ContainerArgs),
    /// Exercise the real-systemd VM scenario using an explicit disposable SSH target
    Smoke(SmokeArgs),
    /// Create or destroy the default disposable clamps VM
    Vm {
        #[command(subcommand)]
        command: VmCommands,
    },
}

#[derive(clap::Subcommand, Debug)]
enum VmCommands {
    /// Provision a disposable clamps VM and wait for it to become ready
    Create(VmCreateArgs),
    /// Destroy the disposable clamps VM and remove its temporary key and artifacts
    Destroy(VmDestroyArgs),
}

#[derive(clap::Args, Debug)]
struct VmCreateArgs {
    #[arg(long, default_value = "clamps-test-smoke")]
    name: String,
    #[arg(long, default_value_t = 2201)]
    port: u16,
}

#[derive(clap::Args, Debug)]
struct VmDestroyArgs {
    #[arg(long, default_value = "clamps-test-smoke")]
    name: String,
}

#[derive(clap::Args, Debug)]
struct ContainerArgs {
    hostname: String,
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
            command:
                TestCommands::Vm {
                    command: VmCommands::Create(args),
                },
        } => run_vm_create(&args)?,
        Commands::Test {
            command:
                TestCommands::Vm {
                    command: VmCommands::Destroy(args),
                },
        } => run_vm_destroy(&args)?,
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
            "SSH key not found at {}; create the default VM with `skillet test vm create` or pass --identity",
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
    if !valid_test_vm_name(&args.name) {
        return Err(anyhow!(
            "VM name must start with clamps-test- and contain lowercase letters, digits, or hyphens"
        ));
    }
    if !(2200..=2299).contains(&args.port) {
        return Err(anyhow!("VM SSH port must be between 2200 and 2299"));
    }
    let butane = butane_root()?;
    let helper = butane.join("bin/clamps-vm");
    let port = args.port.to_string();
    run_helper(&helper, &["create", &args.name, &port])?;

    let run_dir = butane.join("runs").join(&args.name);
    let ready = butane.join("bin/clamps-ready");
    let run_dir_arg = run_dir
        .to_str()
        .ok_or_else(|| anyhow!("VM run path is not valid UTF-8"))?;
    if let Err(error) = run_helper(&ready, &[run_dir_arg]) {
        return Err(anyhow!(
            "VM created but readiness failed; inspect it with `clamps-vm logs {}` or destroy it with `skillet test vm destroy --name {}`: {error}",
            args.name, args.name
        ));
    }
    info!(
        "Disposable VM {} is ready at giacomo@127.0.0.1:{}; smoke key: {}",
        args.name,
        args.port,
        run_dir.join("ssh/id_ed25519").display()
    );
    Ok(())
}

fn run_vm_destroy(args: &VmDestroyArgs) -> Result<()> {
    if !valid_test_vm_name(&args.name) {
        return Err(anyhow!("VM name must start with clamps-test-"));
    }
    let helper = butane_root()?.join("bin/clamps-vm");
    run_helper(&helper, &["destroy", &args.name])
}

fn valid_test_vm_name(name: &str) -> bool {
    name.len() > "clamps-test-".len()
        && name.starts_with("clamps-test-")
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn run_helper(path: &Path, args: &[&str]) -> Result<()> {
    let status = Command::new(path)
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
    if !butane.join("bin/clamps-vm").is_file() || !butane.join("bin/clamps-ready").is_file() {
        return Err(anyhow!(
            "clamps VM helpers not found under {}; check out the sibling butane directory",
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
    let status = Command::new("cargo")
        .current_dir(&root)
        .args(["build", "--package", &package])
        .status()
        .context("building host binary failed")?;
    if !status.success() {
        return Err(anyhow!("cargo build --package {package} failed"));
    }
    let mut binary = None;
    for subdir in [
        "target/x86_64-unknown-linux-musl/debug",
        "target/debug",
        "target/x86_64-unknown-linux-musl/release",
        "target/release",
    ] {
        let candidate = root.join(subdir).join(&package);
        if candidate.is_file() {
            binary = Some(fs::canonicalize(candidate)?);
            break;
        }
    }
    let binary = binary.ok_or_else(|| anyhow!("built binary {package} not found under target"))?;
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
    let start = Command::new("podman")
        .args(["run", "-d", "--rm", "--name", &name, "-v"])
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
    let result = run_twice_and_check(&name, args.inspect);
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

fn run_twice_and_check(name: &str, inspect: bool) -> Result<()> {
    let entry = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test_entrypoint.sh");
    for round in ["first", "second"] {
        let status = Command::new("podman")
            .args(["exec", name, "/bin/sh", "-s", "--", round])
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
        return Err(anyhow!("failed to inspect apply operation log"));
    }
    String::from_utf8(out.stdout).context("operation log was not UTF-8")
}

#[cfg(test)]
mod tests {
    use super::Args;
    use clap::Parser;

    #[test]
    fn test_run_accepts_legacy_command() {
        let parsed = Args::try_parse_from([
            "skillet",
            "test",
            "run",
            "beezelbot",
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

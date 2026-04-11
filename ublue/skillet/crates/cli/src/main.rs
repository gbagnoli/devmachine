use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use skillet_core::credentials::CredentialManager;
use skillet_core::resource_op::ResourceOp;
use skillet_podman::{QuadletSecret, SecretTarget};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Apply configuration (Agent Mode)
    Apply {
        /// Optional: Hostname to apply configuration for
        #[arg(long)]
        host: Option<String>,
        /// Optional: Output recorded actions to this file path
        #[arg(long)]
        record: Option<PathBuf>,
    },
    /// Manage integration tests (Runner Mode)
    Test {
        #[command(subcommand)]
        test_command: TestCommands,
    },
}

#[derive(Subcommand, Debug)]
enum TestCommands {
    Record {
        hostname: String,
        /// Container image to use
        #[arg(long, default_value = "fedora:latest")]
        image: String,
    },
    Run {
        hostname: String,
        #[arg(long, default_value = "fedora:latest")]
        image: String,
    },
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
        Commands::Apply { host, record } => {
            let hostname = host.as_deref().unwrap_or("(Agent Mode)");
            skillet_cli_common::handle_apply(hostname, record, |system, files| {
                match hostname {
                    "beezelbot" => {
                        skillet_hardening::apply(system, files).map_err(|e| e.to_string())?;
                    }
                    "clamps" => {
                        skillet_hardening::apply(system, files).map_err(|e| e.to_string())?;

                        // 1. Ingest secret from systemd
                        let cred_manager = CredentialManager::new().map_err(|e| e.to_string())?;
                        let secret_payload = cred_manager
                            .read_secret("test_secret")
                            .map_err(|e| e.to_string())?;

                        // 2. Provision to Podman
                        system
                            .ensure_podman_secret("pihole_web_password", &secret_payload)
                            .map_err(|e| e.to_string())?;

                        // 3. Apply pihole with the secret
                        let secrets = vec![QuadletSecret {
                            secret_name: "pihole_web_password".to_string(),
                            target: SecretTarget::File {
                                target_path: "/etc/pihole/webpassword".to_string(),
                                mode: Some("0400".to_string()),
                                uid: Some(40000),
                                gid: Some(40000),
                            },
                        }];

                        skillet_pihole::apply(
                            system,
                            files,
                            skillet_pihole::PiholeUser {
                                uid: 40000,
                                gid: 40000,
                                name: "pihole".to_string(),
                                group_name: "pihole".to_string(),
                            },
                            secrets,
                        )
                        .map_err(|e: skillet_pihole::PiholeError| e.to_string())?;
                    }
                    _ => {
                        // Default fallback: just hardening
                        skillet_hardening::apply(system, files).map_err(|e| e.to_string())?;
                    }
                }
                Ok(())
            })
            .map_err(|e| anyhow!("Failed to apply configuration: {e}"))?;
        }
        Commands::Test { test_command } => handle_test(test_command)?,
    }
    Ok(())
}

fn handle_test(cmd: TestCommands) -> Result<()> {
    match cmd {
        TestCommands::Record { hostname, image } => {
            info!("Recording integration test for host: {}", hostname);
            run_container_test(&hostname, &image, true)?;
        }
        TestCommands::Run { hostname, image } => {
            info!(
                "Running integration test verification for host: {}",
                hostname
            );
            run_container_test(&hostname, &image, false)?;
        }
    }
    Ok(())
}

fn run_container_test(hostname: &str, image: &str, is_record: bool) -> Result<()> {
    build_workspace()?;

    let binary_path = locate_binary(hostname)?;
    let container_name = format!("skillet-test-{hostname}");

    setup_container(&container_name, image, &binary_path)?;

    let result = (|| -> Result<()> {
        prepare_and_run_skillet(&container_name)?;
        verify_or_record(hostname, &container_name, is_record)?;
        Ok(())
    })();

    info!("Stopping container...");
    let _ = Command::new("podman")
        .args(["rm", "-f", &container_name])
        .output();

    result
}

fn build_workspace() -> Result<()> {
    info!("Building skillet workspace...");
    let build_status = Command::new("cargo")
        .args(["build"])
        .status()
        .context("Failed to run cargo build")?;

    if !build_status.success() {
        return Err(anyhow!("Build failed"));
    }
    Ok(())
}

fn find_workspace_root() -> Result<PathBuf> {
    let mut current = std::env::current_exe()?
        .parent()
        .ok_or_else(|| anyhow!("Failed to get executable directory"))?
        .to_path_buf();

    loop {
        if current.join("Cargo.toml").exists() {
            return Ok(current);
        }
        if !current.pop() {
            break;
        }
    }

    // Fallback to CWD if not found relative to exe
    let cwd = std::env::current_dir()?;
    if cwd.join("Cargo.toml").exists() {
        return Ok(cwd);
    }

    Err(anyhow!(
        "Failed to locate workspace root (looking for Cargo.toml)"
    ))
}

fn locate_binary(hostname: &str) -> Result<PathBuf> {
    let host_binary_name = format!("skillet-{hostname}");
    let root = find_workspace_root()?;

    let binary_path = [
        root.join("target/release").join(&host_binary_name),
        root.join("target/debug").join(&host_binary_name),
        root.join("target/release").join("skillet"),
        root.join("target/debug").join("skillet"),
    ]
    .into_iter()
    .find(|p| p.exists())
    .ok_or_else(|| {
        anyhow!("No suitable skillet binary found in target/release or target/debug")
    })?;

    info!("Using binary: {}", binary_path.display());
    fs::canonicalize(&binary_path).context("Failed to canonicalize binary path")
}

fn setup_container(container_name: &str, image: &str, binary_path: &Path) -> Result<()> {
    info!("Starting container {container_name} from image {image}...");

    let _ = Command::new("podman")
        .args(["rm", "-f", container_name])
        .output();

    // Create a mock credentials directory
    let root = find_workspace_root()?;
    let mock_creds_dir = root.join("target/mock_creds");
    fs::create_dir_all(&mock_creds_dir)?;
    fs::write(mock_creds_dir.join("test_secret"), "supersecret_payload")?;

    let run_status = Command::new("podman")
        .args([
            "run",
            "-d",
            "--rm",
            "--name",
            container_name,
            "-v",
            &format!("{}:/usr/bin/skillet:ro", binary_path.display()),
            "-v",
            &format!("{}:/run/credentials:ro", mock_creds_dir.display()),
            "-e",
            "CREDENTIALS_DIRECTORY=/run/credentials",
            image,
            "sleep",
            "infinity",
        ])
        .status()
        .context("Failed to start podman container")?;

    if !run_status.success() {
        return Err(anyhow!("Failed to start container"));
    }
    Ok(())
}

fn prepare_and_run_skillet(container_name: &str) -> Result<()> {
    // Prepare entrypoint script
    let entrypoint_content = include_str!("test_entrypoint.sh");
    let mut temp_entrypoint = tempfile::Builder::new().suffix(".sh").tempfile()?;
    temp_entrypoint.write_all(entrypoint_content.as_bytes())?;
    let temp_entrypoint_path = temp_entrypoint
        .path()
        .to_str()
        .ok_or_else(|| anyhow!("Entrypoint path is not valid UTF-8"))?;

    // Copy entrypoint to container
    info!("Copying test entrypoint to container...");
    let cp_status = Command::new("podman")
        .args([
            "cp",
            temp_entrypoint_path,
            &format!("{container_name}:/tmp/test_entrypoint.sh"),
        ])
        .status()
        .context("Failed to copy entrypoint")?;

    if !cp_status.success() {
        return Err(anyhow!("Failed to copy entrypoint to container"));
    }

    // Make executable
    let chmod_status = Command::new("podman")
        .args([
            "exec",
            container_name,
            "chmod",
            "+x",
            "/tmp/test_entrypoint.sh",
        ])
        .status()
        .context("Failed to chmod entrypoint")?;

    if !chmod_status.success() {
        return Err(anyhow!("Failed to chmod entrypoint in container"));
    }

    info!("Executing skillet inside container...");
    let exec_status = Command::new("podman")
        .args([
            "exec",
            container_name,
            "/tmp/test_entrypoint.sh",
            "skillet",
            "apply",
            "--record",
            "/tmp/ops.yaml",
        ])
        .status()
        .context("Failed to exec skillet")?;

    if !exec_status.success() {
        return Err(anyhow!("skillet apply failed inside container"));
    }
    Ok(())
}

fn verify_or_record(hostname: &str, container_name: &str, is_record: bool) -> Result<()> {
    let root = find_workspace_root()?;
    let dest_dir = root.join("integration_tests/recordings");
    fs::create_dir_all(&dest_dir)?;
    let dest_file = dest_dir.join(format!("{hostname}.yaml"));

    if is_record {
        info!("Copying recording to {}", dest_file.display());
        let cp_status = Command::new("podman")
            .args([
                "cp",
                &format!("{container_name}:/tmp/ops.yaml"),
                dest_file
                    .to_str()
                    .ok_or_else(|| anyhow!("Destination path is not valid UTF-8"))?,
            ])
            .status()?;

        if !cp_status.success() {
            return Err(anyhow!("Failed to copy recording from container"));
        }
    } else {
        info!("Verifying recording...");
        let temp_dest = tempfile::Builder::new().suffix(".yaml").tempfile()?;
        let temp_path = temp_dest
            .path()
            .to_str()
            .ok_or_else(|| anyhow!("Temporary path is not valid UTF-8"))?;

        let cp_status = Command::new("podman")
            .args(["cp", &format!("{container_name}:/tmp/ops.yaml"), temp_path])
            .status()?;
        if !cp_status.success() {
            return Err(anyhow!("Failed to copy recording from container"));
        }

        let recorded_content = fs::read_to_string(&dest_file).context(format!(
            "Failed to read existing recording at {}",
            dest_file.display()
        ))?;
        let new_content = fs::read_to_string(temp_path)?;

        let recorded_ops: Vec<ResourceOp> = serde_yml::from_str(&recorded_content)?;
        let new_ops: Vec<ResourceOp> = serde_yml::from_str(&new_content)?;

        if recorded_ops == new_ops {
            info!("Integration test passed!");
        } else {
            error!("Recording mismatch!");
            error!("Expected: {:?}", recorded_ops);
            error!("Actual:   {:?}", new_ops);
            return Err(anyhow!(
                "Integration test failed: Actions do not match recording."
            ));
        }
    }
    Ok(())
}

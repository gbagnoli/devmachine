use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use skillet_core::files::LocalFileResource;
use skillet_core::recorder::Recorder;
use skillet_core::resource_op::ResourceOp;
use skillet_core::system::LinuxSystemResource;
use skillet_hardening::apply;
use std::fs;
use std::path::PathBuf;
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
        Commands::Apply { record } => handle_apply(record),
        Commands::Test { test_command } => handle_test(test_command),
    }
}

fn handle_apply(record_path: Option<PathBuf>) -> Result<()> {
    info!("Starting Skillet configuration (Agent Mode)...");

    let system = LinuxSystemResource::new();
    let files = LocalFileResource::new();

    if let Some(path) = record_path {
        let recorder_system = Recorder::new(system);
        let recorder_files = Recorder::with_ops(files, recorder_system.shared_ops());

        apply(&recorder_system, &recorder_files)?;

        let ops = recorder_system.get_ops();
        let yaml = serde_yaml::to_string(&ops)?;
        fs::write(&path, yaml).context("Failed to write recording")?;
        info!("Recording saved to {}", path.display());
    } else {
        apply(&system, &files)?;
    }

    info!("Configuration applied successfully.");
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
    // 1. Build binary
    info!("Building skillet workspace...");
    let build_status = Command::new("cargo")
        .args(["build"])
        .status()
        .context("Failed to run cargo build")?;

    if !build_status.success() {
        return Err(anyhow!("Build failed"));
    }

    // 2. Locate binary (with fallback)
    let host_binary_name = format!("skillet-{}", hostname);
    let target_debug = PathBuf::from("target/debug");

    let binary_path = if target_debug.join(&host_binary_name).exists() {
        info!("Found host-specific binary: {}", host_binary_name);
        target_debug.join(&host_binary_name)
    } else {
        info!(
            "Using generic skillet binary (host binary {} not found)",
            host_binary_name
        );
        target_debug.join("skillet")
    };

    if !binary_path.exists() {
        return Err(anyhow!(
            "Binary not found at {}. Make sure you run this from workspace root.",
            binary_path.display()
        ));
    }
    let abs_binary_path = fs::canonicalize(&binary_path)?;

    // 3. Start Container
    let container_name = format!("skillet-test-{}", hostname);
    info!(
        "Starting container {} from image {}...",
        container_name, image
    );

    let _ = Command::new("podman")
        .args(["rm", "-f", &container_name])
        .output();

    let run_status = Command::new("podman")
        .args([
            "run",
            "-d",
            "--rm",
            "--name",
            &container_name,
            "-v",
            &format!("{}:/usr/bin/skillet:ro", abs_binary_path.display()),
            image,
            "sleep",
            "infinity",
        ])
        .status()
        .context("Failed to start podman container")?;

    if !run_status.success() {
        return Err(anyhow!("Failed to start container"));
    }

    let result = (|| -> Result<()> {
        // Prepare entrypoint script
        let entrypoint_content = include_str!("test_entrypoint.sh");
        let mut temp_entrypoint = tempfile::Builder::new().suffix(".sh").tempfile()?;
        use std::io::Write;
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
                &format!("{}:/tmp/test_entrypoint.sh", container_name),
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
                &container_name,
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
                &container_name,
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

        let dest_dir = PathBuf::from("integration_tests/recordings");
        fs::create_dir_all(&dest_dir)?;
        let dest_file = dest_dir.join(format!("{}.yaml", hostname));

        if is_record {
            info!("Copying recording to {}", dest_file.display());
            let cp_status = Command::new("podman")
                .args([
                    "cp",
                    &format!("{}:/tmp/ops.yaml", container_name),
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
                .args([
                    "cp",
                    &format!("{}:/tmp/ops.yaml", container_name),
                    temp_path,
                ])
                .status()?;
            if !cp_status.success() {
                return Err(anyhow!("Failed to copy recording from container"));
            }

            let recorded_content = fs::read_to_string(&dest_file).context(format!(
                "Failed to read existing recording at {}",
                dest_file.display()
            ))?;
            let new_content = fs::read_to_string(temp_path)?;

            let recorded_ops: Vec<ResourceOp> = serde_yaml::from_str(&recorded_content)?;
            let new_ops: Vec<ResourceOp> = serde_yaml::from_str(&new_content)?;

            if recorded_ops != new_ops {
                error!("Recording mismatch!");
                error!("Expected: {:?}", recorded_ops);
                error!("Actual:   {:?}", new_ops);
                return Err(anyhow!(
                    "Integration test failed: Actions do not match recording."
                ));
            } else {
                info!("Integration test passed!");
            }
        }

        Ok(())
    })();

    info!("Stopping container...");
    let _ = Command::new("podman")
        .args(["kill", &container_name])
        .output();

    result
}

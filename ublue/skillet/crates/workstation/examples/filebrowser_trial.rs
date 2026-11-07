//! Seed harmless browser fixtures through the owned disposable-VM transport.
use skillet_vm::{
    backend::{VirshBackend, VmBackend},
    transport::{
        GuestCommand, GuestTransport, HostKeyPolicy, OwnershipCheckedTransport, SshTransport,
    },
    ManifestStore, RunIdentity,
};
use std::{error::Error, fs, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: filebrowser_trial BUTANE HOST INSTANCE".into());
    }
    let butane = fs::canonicalize(&args[1])?;
    let identity = RunIdentity::new(&args[2], &args[3])?;
    let profile = skillet_hosts::profile_for_name(identity.host()).ok_or("unknown host")?;
    let (files, owner) = profile
        .services
        .iter()
        .find_map(|s| match s.config {
            skillet_hosts::ServiceConfig::FileBrowser {
                files_path,
                data_owner,
                ..
            } => Some((files_path, data_owner)),
            _ => None,
        })
        .ok_or("host does not declare Quantum")?;
    let store = ManifestStore::new(&butane.join("runs"), skillet_vm::current_uid())?;
    let _lock = store.lock(&identity)?;
    let run = store.load(&identity)?;
    if run.environment != skillet_vm::Environment::Test {
        return Err("requires disposable test environment".into());
    }
    let backend = VirshBackend::for_run(&run, &butane.join("bin/virsh"))?;
    let ownership = || {
        backend
            .inspect(&run)?
            .ok_or_else(|| skillet_vm::Error::Invalid("owned VM absent".into()))?
            .validate_owned(&run)
    };
    ownership()?;
    let transport = SshTransport::new(run.ssh.clone(), HostKeyPolicy::Verify)?;
    let guest = OwnershipCheckedTransport::new(&transport, &ownership);
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../integration_tests/quantum-trial-guest.py");
    let script = fs::read_to_string(source)?;
    let pdf = include_bytes!("../../filebrowser/tests/sample-multipage.pdf");
    let output = guest.execute(
        &GuestCommand {
            program: "sudo",
            arguments: &["-n", "python3", "-c", &script, files, owner],
        },
        Some(pdf),
    )?;
    if !output.status.success() {
        return Err(format!(
            "guest trial fixture failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    print!("{}", String::from_utf8(output.stdout)?);
    Ok(())
}

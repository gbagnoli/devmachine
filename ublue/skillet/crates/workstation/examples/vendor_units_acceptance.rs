//! Owned disposable-VM checks for image unit precedence. Uses a temporary /usr
//! overlay, so it does not establish published-image or reboot persistence.
use skillet_core::{
    files::LocalFileResource,
    system::{LinuxSystemResource, ServiceResource as _},
};
use skillet_vm::{
    backend::{VirshBackend, VmBackend as _},
    transport::{
        GuestCommand, GuestTransport as _, HostKeyPolicy, OwnershipCheckedTransport, SshTransport,
    },
    ManifestStore, RunIdentity,
};
use std::{error::Error, fs, os::unix::fs::PermissionsExt as _, path::Path, process::Command};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn command(program: &str, args: &[&str]) -> Result<String> {
    let mut child = Command::new(program);
    if program == "systemd-analyze" {
        child.env(
            "SYSTEMD_UNIT_PATH",
            "/run/systemd/generator.early:/run/systemd/generator:/run/systemd/generator.late:",
        );
    }
    let output = child.args(args).output()?;
    if !output.status.success() {
        return Err(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().into())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|arg| arg == "--guest") {
        return guest(args.get(2).ok_or("expected disposable hostname")?);
    }
    if args.len() != 4 {
        return Err("usage: vendor_units_acceptance BUTANE HOST INSTANCE".into());
    }
    let butane = fs::canonicalize(&args[1])?;
    let identity = RunIdentity::new(&args[2], &args[3])?;
    let store = ManifestStore::new(&butane.join("runs"), skillet_vm::current_uid())?;
    let _lock = store.lock(&identity)?;
    let run = store.load(&identity)?;
    let backend = VirshBackend::for_run(&run, &butane.join("bin/virsh"))?;
    let ownership = || {
        backend
            .inspect(&run)?
            .ok_or_else(|| skillet_vm::Error::Invalid("owned VM absent".into()))?
            .validate_owned(&run)
    };
    ownership()?;
    let ssh = SshTransport::new(run.ssh.clone(), HostKeyPolicy::Verify)?;
    let checked = OwnershipCheckedTransport::new(&ssh, &ownership);
    let capacity = checked.execute(
        &GuestCommand {
            program: "df",
            arguments: &["-h", "/var/tmp"],
        },
        None,
    )?;
    print!("{}", String::from_utf8(capacity.stdout)?);
    let destination = "/var/tmp/skillet-vendor-units-acceptance";
    checked.upload(&std::env::current_exe()?, destination)?;
    let result = checked.execute(
        &GuestCommand {
            program: "sudo",
            arguments: &["-n", destination, "--guest", &run.guest_hostname],
        },
        None,
    )?;
    print!("{}", String::from_utf8(result.stdout)?);
    if !result.status.success() {
        return Err(format!(
            "guest acceptance failed: {}",
            String::from_utf8_lossy(&result.stderr)
        )
        .into());
    }
    Ok(())
}

fn guest(expected: &str) -> Result<()> {
    verify_guest(expected)?;
    run_guest()
}

fn verify_guest(expected: &str) -> Result<()> {
    if !nix::unistd::Uid::effective().is_root()
        || !expected.contains("-test-")
        || fs::read_to_string("/etc/hostname")?.trim() != expected
    {
        return Err("requires root in the ownership-checked disposable VM".into());
    }
    Ok(())
}

fn run_guest() -> Result<()> {
    let system = LinuxSystemResource::new();
    let files = LocalFileResource::new();
    let config = skillet_homeassistant::Config {
        bluetooth: false,
        backup_subvolume: "/var/lib/data",
        files_root: "/var/lib/data/vendor-unit-fixture",
        backup_files: "/var/lib/data/vendor-unit-fixture/backups",
        owner: "root",
        group: "root",
    };
    let ups = skillet_nut::Config {
        name: "fixture",
        vendor: 1,
        product: 1,
        description: "acceptance fixture",
    };
    let socket = Path::new(skillet_datadog::DOGSTATSD_SOCKET);
    let mut vendor = skillet_kexec::service::image_units()?;
    vendor.extend(skillet_homeassistant::backup::image_units());
    vendor.extend(skillet_homeassistant::metrics::image_units());
    vendor.extend(skillet_nut::monitoring::image_units());
    prepare_fixture(&system, &vendor)?;
    let apply = || -> Result<()> {
        skillet_kexec::service::apply(&system, &files, "/var/usrlocal/bin/skillet-fixture")?;
        skillet_homeassistant::backup::install(&system, &files, config)?;
        skillet_homeassistant::metrics::apply(&system, &files, socket)?;
        skillet_nut::apply_metrics(&system, &files, ups, socket)?;
        Ok(())
    };
    apply()?;
    println!("Legacy definitions installed with harmless action overrides");
    let candidate = Path::new("/usr/bin/.skillet-vendor-acceptance");
    fs::copy("/var/usrlocal/bin/skillet", candidate)?;
    fs::set_permissions(candidate, fs::Permissions::from_mode(0o755))?;
    fs::rename(candidate, "/usr/bin/skillet")?;
    for unit in &vendor {
        fs::write(
            Path::new("/usr/lib/systemd/system").join(unit.name),
            &unit.content,
        )?;
    }
    apply()?;
    let mut timestamps = Vec::new();
    for unit in &vendor {
        if Path::new("/etc/systemd/system").join(unit.name).exists() {
            return Err(format!("runtime override still exists: {}", unit.name).into());
        }
        if unit.is_timer() {
            if !system.service_is_active(unit.name)? {
                return Err("timer did not activate".into());
            }
            timestamps.push((
                unit.name,
                command(
                    "systemctl",
                    &[
                        "show",
                        "--property=ActiveEnterTimestampMonotonic",
                        "--value",
                        unit.name,
                    ],
                )?,
            ));
        }
    }
    apply()?;
    for (name, before) in timestamps {
        if command(
            "systemctl",
            &[
                "show",
                "--property=ActiveEnterTimestampMonotonic",
                "--value",
                name,
            ],
        )? != before
        {
            return Err("repeat apply restarted a timer".into());
        }
    }
    println!("Retained migration and repeat apply passed");
    let custom = Path::new("/etc/systemd/system/skillet-kexec-activate.service");
    let custom_bytes = b"[Service]\nExecStart=/usr/bin/false\n# administrator edit\n";
    fs::write(custom, custom_bytes)?;
    if apply().is_ok() || fs::read(custom)? != custom_bytes {
        return Err("administrator override was not preserved/refused".into());
    }
    fs::remove_file(custom)?;
    apply()?;
    verify_fragments(&system, &vendor)?;
    println!(
        "Custom-edit refusal, vendor precedence and real systemd syntax passed; timers stopped"
    );
    Ok(())
}

fn verify_fragments(
    system: &LinuxSystemResource,
    vendor: &[skillet_core::vendor_units::Unit],
) -> Result<()> {
    for unit in vendor.iter().filter(|unit| unit.is_timer()) {
        system.service_stop(unit.name)?;
    }
    for unit in vendor.iter().filter(|unit| unit.is_service()) {
        fs::remove_file(format!(
            "/run/systemd/system/{}.d/90-skillet-acceptance.conf",
            unit.name
        ))?;
    }
    system.daemon_reload()?;
    for unit in vendor {
        let path = format!("/usr/lib/systemd/system/{}", unit.name);
        if command(
            "systemctl",
            &["show", "--property=FragmentPath", "--value", unit.name],
        )? != path
        {
            return Err(format!("wrong effective fragment: {}", unit.name).into());
        }
        command("systemd-analyze", &["verify", "--man=no", &path])?;
    }
    for unit in vendor.iter().filter(|unit| unit.is_timer()) {
        command("systemctl", &["disable", unit.name])?;
    }
    Ok(())
}

fn prepare_fixture(
    system: &LinuxSystemResource,
    vendor: &[skillet_core::vendor_units::Unit],
) -> Result<()> {
    if command(
        "findmnt",
        &["--noheadings", "--output", "FSTYPE", "--target", "/usr"],
    )? != "overlay"
    {
        command("rpm-ostree", &["usroverlay"])?;
    }
    for unit in vendor {
        let path = Path::new("/usr/lib/systemd/system").join(unit.name);
        if path.exists() {
            if fs::read(&path)? != unit.content.as_bytes() {
                return Err(format!("unexpected fixture vendor definition: {}", unit.name).into());
            }
            fs::remove_file(path)?;
        }
        if unit.is_service() {
            command("systemctl", &["unmask", "--runtime", unit.name])?;
            let directory = format!("/run/systemd/system/{}.d", unit.name);
            fs::create_dir_all(&directory)?;
            fs::write(
                Path::new(&directory).join("90-skillet-acceptance.conf"),
                "[Service]\nExecStart=\nExecStart=/usr/bin/true\nExecStopPost=\nExecCondition=\n",
            )?;
        }
    }
    system.daemon_reload()?;
    Ok(())
}

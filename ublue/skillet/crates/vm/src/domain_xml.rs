//! Libvirt domain XML rendering for a recorded test run.
use crate::{Backend, Error, ManifestStore, Phase, Result, VmRun};
use quick_xml::{
    events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event},
    Writer,
};
use std::{
    fs::{self, File},
    io::Write as _,
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
};

/// Write the KVM definition beside its owned disk and Ignition files.
/// The output is atomically replaced and never follows a symlink.
pub fn write_domain_xml(store: &ManifestStore, run: &VmRun, emulator: &Path) -> Result<PathBuf> {
    store.validate(run, &run.identity)?;
    if !matches!(run.phase, Phase::Preparing | Phase::Defined)
        || !emulator.is_absolute()
        || (run.connection.backend == Backend::Native && !emulator.is_file())
    {
        return Err(Error::Invalid(
            "domain XML requires a preparing run and absolute emulator".into(),
        ));
    }
    let dir = run
        .disk
        .parent()
        .ok_or_else(|| Error::Invalid("VM disk has no parent directory".into()))?;
    for path in [&run.disk, &run.ignition] {
        crate::manifest::reject_symlinks(path)?;
        if !fs::metadata(path)?.is_file() {
            return Err(Error::Invalid(
                "VM disk and Ignition must be regular files".into(),
            ));
        }
    }
    crate::manifest::reject_symlinks(dir)?;
    let path = dir.join("domain.xml");
    crate::manifest::reject_symlinks(&path)?;
    if run.root_profile == crate::install::RootProfile::Tpm {
        let serial = dir.join("serial.log");
        crate::manifest::reject_symlinks(&serial)?;
        let log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(serial)?;
        log.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    let xml = render(run, emulator)?;
    let mut output = tempfile::NamedTempFile::new_in(dir)?;
    output
        .as_file()
        .set_permissions(fs::Permissions::from_mode(0o600))?;
    output.write_all(xml.as_bytes())?;
    output.write_all(b"\n")?;
    output.as_file().sync_all()?;
    output
        .persist(&path)
        .map_err(|error| Error::Io(error.error))?;
    File::open(dir)?.sync_all()?;
    Ok(path)
}

fn render(run: &VmRun, emulator: &Path) -> Result<String> {
    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    start(&mut writer, "domain", &[("type", "kvm".into())])?;
    text_element(&mut writer, "name", &run.identity.domain_name())?;
    text_element(&mut writer, "uuid", &run.uuid.to_string())?;
    text_element_with_attrs(&mut writer, "memory", "8192", &[("unit", "MiB".into())])?;
    text_element(&mut writer, "vcpu", "2")?;

    let encrypted = run.root_profile == crate::install::RootProfile::Tpm;
    let firmware_attrs = if encrypted {
        vec![("firmware", "efi".into())]
    } else {
        Vec::new()
    };
    start(&mut writer, "os", &firmware_attrs)?;
    if encrypted {
        start(&mut writer, "firmware", &[])?;
        for name in ["secure-boot", "enrolled-keys"] {
            empty(
                &mut writer,
                "feature",
                &[("enabled", "yes".into()), ("name", name.into())],
            )?;
        }
        end(&mut writer, "firmware")?;
    }
    text_element_with_attrs(
        &mut writer,
        "type",
        "hvm",
        &[("arch", "x86_64".into()), ("machine", "q35".into())],
    )?;
    if encrypted {
        text_element_with_attrs(
            &mut writer,
            "nvram",
            &crate::install::nvram_path(run)?.display().to_string(),
            &[("format", "raw".into())],
        )?;
    }
    empty(&mut writer, "boot", &[("dev", "hd".into())])?;
    end(&mut writer, "os")?;

    start(&mut writer, "features", &[])?;
    empty(&mut writer, "acpi", &[])?;
    empty(&mut writer, "apic", &[])?;
    if encrypted {
        empty(&mut writer, "smm", &[("state", "on".into())])?;
    }
    end(&mut writer, "features")?;
    empty(&mut writer, "cpu", &[("mode", "host-passthrough".into())])?;

    start(&mut writer, "sysinfo", &[("type", "fwcfg".into())])?;
    empty(
        &mut writer,
        "entry",
        &[
            ("name", "opt/com.coreos/config".into()),
            ("file", run.ignition.display().to_string()),
        ],
    )?;
    end(&mut writer, "sysinfo")?;

    start(&mut writer, "devices", &[])?;
    if run.connection.backend == Backend::Native {
        text_element(&mut writer, "emulator", &emulator.display().to_string())?;
    }
    if encrypted {
        render_tpm(&mut writer, run)?;
    }
    disk(&mut writer, &run.disk, "qcow2", "vda", false)?;
    disk(&mut writer, &run.ignition, "raw", "vdb", true)?;
    start(&mut writer, "interface", &[("type", "user".into())])?;
    empty(&mut writer, "backend", &[("type", "passt".into())])?;
    start(
        &mut writer,
        "portForward",
        &[("proto", "tcp".into()), ("address", "127.0.0.1".into())],
    )?;
    empty(
        &mut writer,
        "range",
        &[("start", run.ssh.port.to_string()), ("to", "22".into())],
    )?;
    end(&mut writer, "portForward")?;
    empty(&mut writer, "model", &[("type", "virtio".into())])?;
    end(&mut writer, "interface")?;
    render_console(&mut writer, run, encrypted)?;
    end(&mut writer, "devices")?;
    end(&mut writer, "domain")?;
    String::from_utf8(writer.into_inner())
        .map_err(|_| Error::Invalid("generated libvirt XML was not UTF-8".into()))
}

fn render_console(writer: &mut Writer<Vec<u8>>, run: &VmRun, encrypted: bool) -> Result<()> {
    start(writer, "serial", &[("type", "pty".into())])?;
    if encrypted {
        empty(
            writer,
            "log",
            &[
                (
                    "file",
                    run.disk
                        .parent()
                        .ok_or_else(|| Error::Invalid("disk has no parent".into()))?
                        .join("serial.log")
                        .display()
                        .to_string(),
                ),
                ("append", "on".into()),
            ],
        )?;
    }
    empty(writer, "target", &[("port", "0".into())])?;
    end(writer, "serial")?;
    start(writer, "console", &[("type", "pty".into())])?;
    empty(
        writer,
        "target",
        &[("type", "serial".into()), ("port", "0".into())],
    )?;
    end(writer, "console")?;
    Ok(())
}

fn render_tpm(writer: &mut Writer<Vec<u8>>, run: &VmRun) -> Result<()> {
    start(writer, "tpm", &[("model", "tpm-crb".into())])?;
    start(
        writer,
        "backend",
        &[("type", "emulator".into()), ("version", "2.0".into())],
    )?;
    empty(
        writer,
        "source",
        &[
            ("type", "dir".into()),
            ("path", crate::install::tpm_path(run)?.display().to_string()),
        ],
    )?;
    start(writer, "active_pcr_banks", &[])?;
    empty(writer, "sha256", &[])?;
    end(writer, "active_pcr_banks")?;
    end(writer, "backend")?;
    end(writer, "tpm")?;
    Ok(())
}

fn disk(
    writer: &mut Writer<Vec<u8>>,
    path: &Path,
    format: &str,
    target: &str,
    readonly: bool,
) -> Result<()> {
    start(
        writer,
        "disk",
        &[("type", "file".into()), ("device", "disk".into())],
    )?;
    empty(
        writer,
        "driver",
        &[("name", "qemu".into()), ("type", format.into())],
    )?;
    empty(writer, "source", &[("file", path.display().to_string())])?;
    empty(
        writer,
        "target",
        &[("dev", target.into()), ("bus", "virtio".into())],
    )?;
    if readonly {
        empty(writer, "readonly", &[])?;
    }
    end(writer, "disk")
}

fn start(writer: &mut Writer<Vec<u8>>, name: &str, attrs: &[(&str, String)]) -> Result<()> {
    let mut element = BytesStart::new(name);
    for (key, value) in attrs {
        element.push_attribute((*key, value.as_str()));
    }
    writer.write_event(Event::Start(element))?;
    Ok(())
}

fn empty(writer: &mut Writer<Vec<u8>>, name: &str, attrs: &[(&str, String)]) -> Result<()> {
    let mut element = BytesStart::new(name);
    for (key, value) in attrs {
        element.push_attribute((*key, value.as_str()));
    }
    writer.write_event(Event::Empty(element))?;
    Ok(())
}

fn text(writer: &mut Writer<Vec<u8>>, value: &str) -> Result<()> {
    writer.write_event(Event::Text(BytesText::new(value)))?;
    Ok(())
}

fn text_element(writer: &mut Writer<Vec<u8>>, name: &str, value: &str) -> Result<()> {
    start(writer, name, &[])?;
    text(writer, value)?;
    end(writer, name)
}

fn text_element_with_attrs(
    writer: &mut Writer<Vec<u8>>,
    name: &str,
    value: &str,
    attrs: &[(&str, String)],
) -> Result<()> {
    start(writer, name, attrs)?;
    text(writer, value)?;
    end(writer, name)
}

fn end(writer: &mut Writer<Vec<u8>>, name: &str) -> Result<()> {
    writer.write_event(Event::End(BytesEnd::new(name)))?;
    Ok(())
}

#[cfg(test)]
#[path = "domain_xml_tests.rs"]
mod tests;

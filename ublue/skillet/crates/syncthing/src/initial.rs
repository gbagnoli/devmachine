//! Missing-only application bootstrap. The generator runs with no network.
use crate::Instance;
use quick_xml::{
    events::{BytesEnd, BytesStart, BytesText, Event},
    Reader, Writer,
};
use serde::{Deserialize, Serialize};
use skillet_core::files::{FileError, FileMutationResource, FileReadResource, Ownership};
use std::path::Path;
use thiserror::Error;

pub const CREDENTIAL: &str = "syncthing_initial";
pub const UNIT: &str = "skillet-syncthing-initialize.service";
pub const IMAGE: &str = "docker.io/syncthing/syncthing:latest";

pub fn install<
    S: skillet_core::system::ServiceResource + ?Sized,
    F: FileMutationResource + ?Sized,
>(
    system: &S,
    files: &F,
) -> Result<(), crate::SyncthingError> {
    install_instance(system, files, Instance::Shared)
}

pub fn install_instance<
    S: skillet_core::system::ServiceResource + ?Sized,
    F: FileMutationResource + ?Sized,
>(
    system: &S,
    files: &F,
    instance: Instance,
) -> Result<(), crate::SyncthingError> {
    let mut unit = include_str!("initial.service")
        .replace("syncthing_initial", instance.credential())
        .replace(
            "Before=syncthing.service",
            &format!("Before={}", instance.service()),
        );
    if instance == Instance::Personal {
        unit = unit.replace(
            "--host-file /etc/skillet/host",
            "--host-file /etc/skillet/host --instance personal",
        );
    }
    files.ensure_directory(
        Path::new("/etc/systemd/system"),
        Some(0o755),
        &Ownership::default(),
    )?;
    if files.ensure_file(
        &Path::new("/etc/systemd/system").join(instance.initial_unit()),
        unit.as_bytes(),
        Some(0o644),
        &Ownership::default(),
    )? {
        system.daemon_reload()?;
    }
    Ok(())
}

/// Private resolved input; deliberately does not implement Debug.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    #[serde(default, skip_serializing_if = "Instance::is_shared")]
    pub instance: Instance,
    pub host: String,
    pub environment: String,
    pub runtime_hostname: String,
    pub device_name: String,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid Syncthing bootstrap identity or configuration")]
    Invalid,
    #[error("Syncthing bootstrap is bound to different settings")]
    Binding,
    #[error("incomplete or conflicting Syncthing device identity; keys were preserved")]
    Identity,
    #[error("Syncthing offline generator failed")]
    Generator,
    #[error(transparent)]
    File(#[from] FileError),
}

impl Settings {
    pub fn validate(&self) -> Result<(), Error> {
        if !matches!(self.environment.as_str(), "prod" | "test")
            || !label(&self.host)
            || !label(&self.runtime_hostname)
            || self.device_name.len() > 253
            || !self.device_name.starts_with(&format!(
                "{}.",
                self.instance.device_label(&self.runtime_hostname)
            ))
            || !self.device_name.split('.').all(label)
            || (self.environment == "prod" && self.host != self.runtime_hostname)
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
    pub fn payload(&self) -> Result<Vec<u8>, Error> {
        self.validate()?;
        serde_json::to_vec(self).map_err(|_| Error::Invalid)
    }
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let value: Self = serde_json::from_slice(bytes).map_err(|_| Error::Invalid)?;
        value.validate()?;
        Ok(value)
    }
}

fn label(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 63
        && !s.starts_with('-')
        && !s.ends_with('-')
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// Generate upstream artifacts in this owned staging directory only.
pub trait Generator {
    /// Reap a previously journaled owned generator before inspecting its staging.
    fn recover(&self, _staging: &Path) -> Result<(), Error> {
        Ok(())
    }
    fn generate(&self, staging: &Path) -> Result<(), Error>;
}

/// Caller serializes initialization against other bootstraps and stops the app
/// before resetting XML. Config publication is the final commit point.
pub fn initialize<F: FileReadResource + FileMutationResource + ?Sized>(
    files: &F,
    generator: &impl Generator,
    root: &Path,
    settings: &Settings,
    acknowledge_proxy: bool,
    ownership: &Ownership,
) -> Result<bool, Error> {
    settings.validate()?;
    let target = root.join("config.xml");
    if files.read_file(&target)?.is_some() {
        cleanup_completed(files, root, settings)?;
        return Ok(false);
    }
    let stage = root.join(".skillet-initial");
    files.ensure_directory(&stage, Some(0o700), ownership)?;
    let binding = stage.join("settings.json");
    let payload = settings.payload()?;
    files.create_file_if_absent(&binding, &payload, Some(0o600), ownership)?;
    if files.read_file(&binding)?.as_deref() != Some(payload.as_slice()) {
        return Err(Error::Binding);
    }
    generator.recover(&stage)?;
    let retained = [
        files.read_file(&root.join("cert.pem"))?,
        files.read_file(&root.join("key.pem"))?,
    ];
    let pending = [
        files.read_file(&stage.join("cert.pem"))?,
        files.read_file(&stage.join("key.pem"))?,
    ];
    // A partial publication is recoverable only from the complete owned pair.
    if retained.iter().filter(|v| v.is_some()).count() == 1 && pending.iter().any(Option::is_none) {
        return Err(Error::Identity);
    }
    if retained.iter().all(Option::is_none) && pending.iter().any(Option::is_none) {
        // No live identity exists yet: discard only this bound partial generation.
        for name in ["cert.pem", "key.pem", "config.xml"] {
            files.delete_file(&stage.join(name))?;
        }
    }
    for (i, name) in ["cert.pem", "key.pem"].iter().enumerate() {
        if let Some(bytes) = &retained[i] {
            files.create_file_if_absent(&stage.join(name), bytes, Some(0o600), ownership)?;
            if files.read_file(&stage.join(name))?.as_ref() != Some(bytes) {
                return Err(Error::Identity);
            }
        }
    }
    generator.generate(&stage)?;
    let xml = files
        .read_file(&stage.join("config.xml"))?
        .ok_or(Error::Invalid)?;
    let completed = configure(&xml, &settings.device_name, acknowledge_proxy)?;
    // Validate both artifacts before publishing either. Never replace a key.
    let keys = [
        files
            .read_file(&stage.join("cert.pem"))?
            .ok_or(Error::Identity)?,
        files
            .read_file(&stage.join("key.pem"))?
            .ok_or(Error::Identity)?,
    ];
    if keys.iter().any(Vec::is_empty) {
        return Err(Error::Identity);
    }
    for (name, bytes) in ["cert.pem", "key.pem"].iter().zip(&keys) {
        let path = root.join(name);
        files.create_file_if_absent(&path, bytes, Some(0o600), ownership)?;
        if files.read_file(&path)?.as_ref() != Some(bytes) {
            return Err(Error::Identity);
        }
    }
    let created = files.create_file_if_absent(&target, &completed, Some(0o600), ownership)?;
    // Keep owned pending artifacts through every error; remove after commit.
    for name in ["config.xml", "cert.pem", "key.pem", "settings.json"] {
        files.delete_file(&stage.join(name))?;
    }
    Ok(created)
}

/// Finish owned staging cleanup after a committed XML; never touch application files.
pub fn cleanup_completed<F: FileReadResource + FileMutationResource + ?Sized>(
    files: &F,
    root: &Path,
    settings: &Settings,
) -> Result<(), Error> {
    let stage = root.join(".skillet-initial");
    if files.read_file(&stage.join("settings.json"))?.as_deref()
        != Some(settings.payload()?.as_slice())
    {
        return Ok(());
    }
    for name in ["cert.pem", "key.pem"] {
        if let Some(bytes) = files.read_file(&stage.join(name))? {
            if files.read_file(&root.join(name))?.as_ref() != Some(&bytes) {
                return Err(Error::Identity);
            }
        }
    }
    for name in ["config.xml", "cert.pem", "key.pem", "settings.json"] {
        files.delete_file(&stage.join(name))?;
    }
    Ok(())
}

/// Change only bootstrap fields in a complete upstream-generated document.
pub fn configure(xml: &[u8], name: &str, acknowledge_proxy: bool) -> Result<Vec<u8>, Error> {
    let mut reader = Reader::from_reader(xml);
    let mut writer = Writer::new(Vec::new());
    let mut stack = Vec::new();
    let mut device_count = 0;
    let mut gui_flag = false;
    let mut api_key = false;
    let mut version = false;
    loop {
        let event = reader.read_event().map_err(|_| Error::Invalid)?;
        match &event {
            Event::Start(start) => {
                let tag = start.name().as_ref().to_vec();
                if stack.is_empty() && tag == b"configuration" {
                    version = start
                        .attributes()
                        .any(|a| a.is_ok_and(|a| a.key.as_ref() == b"version"));
                }
                if stack.len() == 1 && tag == b"device" {
                    device_count += 1;
                    let mut replacement = start.clone().into_owned();
                    replacement.clear_attributes();
                    for attribute in start.attributes() {
                        let attribute = attribute.map_err(|_| Error::Invalid)?;
                        if attribute.key.as_ref() != b"name" {
                            replacement.push_attribute(attribute);
                        }
                    }
                    replacement.push_attribute(("name", name));
                    writer
                        .write_event(Event::Start(replacement))
                        .map_err(|_| Error::Invalid)?;
                    stack.push(tag);
                    continue;
                }
                if stack.len() == 2 && stack[1] == b"gui" && tag == b"insecureAdminAccess" {
                    reader
                        .read_to_end(start.name())
                        .map_err(|_| Error::Invalid)?;
                    write_gui_flag(&mut writer, acknowledge_proxy)?;
                    gui_flag = true;
                    continue;
                }
                stack.push(tag);
            }
            Event::Empty(start)
                if stack.len() == 2
                    && stack[1] == b"gui"
                    && start.name().as_ref() == b"insecureAdminAccess" =>
            {
                write_gui_flag(&mut writer, acknowledge_proxy)?;
                gui_flag = true;
                continue;
            }
            Event::Text(text) => {
                if stack.last().is_some_and(|tag| tag == b"apikey") && !text.as_ref().is_empty() {
                    api_key = true;
                }
            }
            Event::End(_) => {
                // Upstream omits false/default fields from fresh generated XML.
                if stack.len() == 2 && stack[1] == b"gui" && !gui_flag {
                    write_gui_flag(&mut writer, acknowledge_proxy)?;
                    gui_flag = true;
                }
                stack.pop().ok_or(Error::Invalid)?;
            }
            Event::Eof => break,
            _ => {}
        }
        writer.write_event(event).map_err(|_| Error::Invalid)?;
    }
    if !version || device_count != 1 || !api_key || !gui_flag || !stack.is_empty() {
        return Err(Error::Invalid);
    }
    Ok(writer.into_inner())
}

fn write_gui_flag(writer: &mut Writer<Vec<u8>>, value: bool) -> Result<(), Error> {
    for event in [
        Event::Start(BytesStart::new("insecureAdminAccess")),
        Event::Text(BytesText::new(if value { "true" } else { "false" })),
        Event::End(BytesEnd::new("insecureAdminAccess")),
    ] {
        writer.write_event(event).map_err(|_| Error::Invalid)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "initial/tests.rs"]
mod tests;

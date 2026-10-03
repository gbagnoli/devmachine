//! Focused libvirt adapter. A failed connection is never an absent domain.
use crate::{Backend, Connection, Error, Result, VmRun};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DomainSnapshot {
    pub name: String,
    pub uuid: Uuid,
    pub state: String,
    pub disks: Vec<PathBuf>,
}

impl DomainSnapshot {
    pub fn validate_owned(&self, run: &VmRun) -> Result<()> {
        let mut actual = self.disks.clone();
        actual.sort();
        let mut expected = vec![run.disk.clone(), run.ignition.clone()];
        expected.sort();
        if self.uuid != run.uuid || self.name != run.identity.domain_name() || actual != expected {
            return Err(Error::Invalid(
                "domain UUID, name or disks do not match owned run".into(),
            ));
        }
        Ok(())
    }
}

pub trait VmBackend {
    fn inspect(&self, run: &VmRun) -> Result<Option<DomainSnapshot>>;
    fn define(&self, run: &VmRun, xml: &Path) -> Result<()>;
    fn start(&self, run: &VmRun) -> Result<()>;
    fn reboot(&self, run: &VmRun) -> Result<()>;
    fn stop(&self, run: &VmRun) -> Result<()>;
    fn undefine(&self, run: &VmRun) -> Result<()>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirshInvocation {
    pub program: PathBuf,
    pub arguments: Vec<OsString>,
    pub environment: BTreeMap<OsString, OsString>,
}

#[derive(Debug)]
pub struct VirshOutput {
    pub success: bool,
    pub code: Option<i32>,
    pub stdout: String,
}

/// Execution boundary belongs only to the libvirt adapter, not host recipes.
pub trait VirshExecutor {
    fn execute(&self, invocation: &VirshInvocation) -> Result<VirshOutput>;
}

pub struct ProcessExecutor {
    pub timeout: Duration,
}

impl Default for ProcessExecutor {
    fn default() -> Self {
        Self {
            timeout: Duration::from_mins(1),
        }
    }
}

impl VirshExecutor for ProcessExecutor {
    fn execute(&self, invocation: &VirshInvocation) -> Result<VirshOutput> {
        let mut command = Command::new(&invocation.program);
        command
            .args(&invocation.arguments)
            .envs(&invocation.environment);
        let output = crate::process::capture(command, self.timeout)?;
        Ok(VirshOutput {
            success: output.status.success(),
            code: output.status.code(),
            stdout: String::from_utf8(output.stdout)
                .map_err(|_| Error::Invalid("libvirt output is not UTF8".into()))?,
        })
    }
}

pub struct VirshBackend<E = ProcessExecutor> {
    connection: Connection,
    program: PathBuf,
    executor: E,
}

impl VirshBackend {
    pub fn for_run(run: &VmRun, flatpak_wrapper: &Path) -> Result<Self> {
        Self::new(
            run.connection.clone(),
            run.owner_uid,
            flatpak_wrapper,
            ProcessExecutor::default(),
        )
    }
}

impl<E: VirshExecutor> VirshBackend<E> {
    pub fn new(
        connection: Connection,
        owner_uid: u32,
        flatpak_wrapper: &Path,
        executor: E,
    ) -> Result<Self> {
        connection.validate(owner_uid)?;
        let program = match connection.backend {
            Backend::Native => "/usr/bin/virsh".into(),
            Backend::Flatpak => flatpak_wrapper.into(),
        };
        Ok(Self {
            connection,
            program,
            executor,
        })
    }

    fn invoke(&self, arguments: &[&str]) -> Result<String> {
        let runtime_key = match self.connection.backend {
            Backend::Native => "XDG_RUNTIME_DIR",
            Backend::Flatpak => "TEST_VM_LIBVIRT_RUNTIME_DIR",
        };
        let invocation = VirshInvocation {
            program: self.program.clone(),
            arguments: ["-c", self.connection.uri.as_str()]
                .into_iter()
                .chain(arguments.iter().copied())
                .map(OsString::from)
                .collect(),
            environment: BTreeMap::from([(
                runtime_key.into(),
                self.connection.runtime_dir.as_os_str().into(),
            )]),
        };
        let output = self.executor.execute(&invocation)?;
        if !output.success {
            return Err(Error::Command {
                operation: arguments.first().copied().unwrap_or("inspect").into(),
                code: output.code,
            });
        }
        Ok(output.stdout)
    }

    fn action(&self, action: &str, run: &VmRun) -> Result<()> {
        self.validate_connection(run)?;
        self.invoke(&[action, &run.uuid.to_string()])?;
        Ok(())
    }

    fn validate_connection(&self, run: &VmRun) -> Result<()> {
        if run.connection != self.connection {
            return Err(Error::Invalid(
                "adapter connection differs from the run manifest".into(),
            ));
        }
        Ok(())
    }

    /// Resolve the emulator advertised for the architecture used by native
    /// test guests. Keep capability parsing beside the selected libvirt
    /// adapter so alternate runtimes use their own connection and environment.
    pub fn x86_64_emulator(&self, run: &VmRun) -> Result<PathBuf> {
        self.validate_connection(run)?;
        let capabilities = self.invoke(&["capabilities"])?;
        let capabilities: CapabilitiesXml = quick_xml::de::from_str(&capabilities)?;
        let emulator = capabilities
            .guests
            .into_iter()
            .filter(|guest| guest.os_type == "hvm")
            .flat_map(|guest| guest.arches)
            .find(|arch| arch.name == "x86_64")
            .and_then(|arch| arch.emulator)
            .ok_or_else(|| Error::Invalid("libvirt has no x86_64 HVM emulator".into()))?;
        let metadata = fs::metadata(&emulator)?;
        if !emulator.is_absolute()
            || !metadata.is_file()
            || metadata.permissions().mode() & 0o111 == 0
        {
            return Err(Error::Invalid(
                "libvirt x86_64 emulator is not an executable file".into(),
            ));
        }
        Ok(emulator)
    }

    pub fn version(&self, run: &VmRun) -> Result<String> {
        self.validate_connection(run)?;
        self.invoke(&["--version"])
    }
}

impl<E: VirshExecutor> VmBackend for VirshBackend<E> {
    fn inspect(&self, run: &VmRun) -> Result<Option<DomainSnapshot>> {
        self.validate_connection(run)?;
        let uuids = self.invoke(&["list", "--all", "--uuid"])?;
        let uuid = run.uuid.to_string();
        let name = run.identity.domain_name();
        let target = if uuids.lines().any(|line| line.trim() == uuid) {
            uuid.as_str()
        } else {
            let names = self.invoke(&["list", "--all", "--name"])?;
            if !names.lines().any(|line| line.trim() == name) {
                return Ok(None);
            }
            name.as_str()
        };
        let xml = self.invoke(&["dumpxml", target])?;
        let state = self.invoke(&["domstate", target])?.trim().to_string();
        Ok(Some(parse_domain(&xml, state)?))
    }

    fn define(&self, run: &VmRun, xml: &Path) -> Result<()> {
        self.validate_connection(run)?;
        crate::manifest::reject_symlinks(xml)?;
        let definition = parse_domain(&fs::read_to_string(xml)?, "defined".into())?;
        definition.validate_owned(run)?;
        let xml = xml
            .to_str()
            .ok_or_else(|| Error::Invalid("non-UTF8 domain XML path".into()))?;
        self.invoke(&["define", xml])?;
        Ok(())
    }
    fn start(&self, run: &VmRun) -> Result<()> {
        self.action("start", run)
    }
    fn reboot(&self, run: &VmRun) -> Result<()> {
        self.action("reboot", run)
    }
    fn stop(&self, run: &VmRun) -> Result<()> {
        self.action("destroy", run)
    }
    fn undefine(&self, run: &VmRun) -> Result<()> {
        self.action("undefine", run)
    }
}

#[derive(Deserialize)]
struct DomainXml {
    name: String,
    uuid: Uuid,
    devices: DevicesXml,
}

#[derive(Deserialize)]
struct CapabilitiesXml {
    #[serde(rename = "guest", default)]
    guests: Vec<GuestXml>,
}

#[derive(Deserialize)]
struct GuestXml {
    os_type: String,
    #[serde(rename = "arch", default)]
    arches: Vec<ArchitectureXml>,
}

#[derive(Deserialize)]
struct ArchitectureXml {
    #[serde(rename = "@name")]
    name: String,
    emulator: Option<PathBuf>,
}
#[derive(Deserialize)]
struct DevicesXml {
    #[serde(rename = "disk", default)]
    disks: Vec<DiskXml>,
}
#[derive(Deserialize)]
struct DiskXml {
    #[serde(rename = "@device")]
    kind: String,
    source: Option<DiskSourceXml>,
}
#[derive(Deserialize)]
struct DiskSourceXml {
    #[serde(rename = "@file")]
    file: Option<PathBuf>,
}

pub(crate) fn parse_domain(xml: &str, state: String) -> Result<DomainSnapshot> {
    let domain: DomainXml = quick_xml::de::from_str(xml)?;
    let mut disks = Vec::new();
    for disk in domain.devices.disks {
        if disk.kind == "disk" {
            let source = disk
                .source
                .and_then(|source| source.file)
                .ok_or_else(|| Error::Invalid("VM has a non-file disk source".into()))?;
            disks.push(source);
        }
    }
    Ok(DomainSnapshot {
        name: domain.name,
        uuid: domain.uuid,
        state,
        disks,
    })
}

#[cfg(test)]
#[path = "backend/tests.rs"]
mod tests;

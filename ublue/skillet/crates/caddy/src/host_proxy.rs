//! A bounded Unix-to-loopback relay for host-network UI listeners.
use crate::{CaddyError, CaddySites, UpstreamTransport};
use skillet_core::{
    activation::{activate, ActivationRequest, ConsumerKind},
    files::{FileMutationResource, FileReadResource, Ownership},
    system::ServiceResource,
};
use skillet_podman::Volume;
use std::{
    io,
    net::{Ipv4Addr, Shutdown, SocketAddr, TcpStream},
    os::unix::{
        fs::{FileTypeExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

const POLICY: &str = include_str!("host-proxy.cil");

pub fn apply<S, F>(
    system: &S,
    files: &F,
    sites: &CaddySites,
    volumes: &mut Vec<Volume>,
) -> Result<(), CaddyError>
where
    S: ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    let services: Vec<_> = sites
        .services
        .iter()
        .filter(|s| s.transport == UpstreamTransport::HostLoopback)
        .collect();
    if services.is_empty() {
        return Ok(());
    }
    let root = Ownership::named(Some("root"), Some("root"));
    files.ensure_directory(Path::new("/etc/skillet/selinux"), Some(0o755), &root)?;
    files.ensure_directory(Path::new("/var/lib/skillet/ui-host"), Some(0o700), &root)?;
    let changed = files.ensure_file(
        Path::new("/etc/skillet/selinux/skillet-ui-host.cil"),
        POLICY.as_bytes(),
        Some(0o644),
        &root,
    )?;
    let policy_unit = "[Unit]\nDescription=Load Skillet host UI relay policy\nConditionSecurity=selinux\n[Service]\nType=oneshot\nRemainAfterExit=yes\nExecStart=/usr/bin/semodule -i /etc/skillet/selinux/skillet-ui-host.cil\nTimeoutStartSec=120\n";
    let unit_changed = files.ensure_file(
        Path::new("/etc/systemd/system/skillet-ui-host-policy.service"),
        policy_unit.as_bytes(),
        Some(0o644),
        &root,
    )?;
    activate(
        system,
        files,
        &ActivationRequest {
            service: "skillet-ui-host-policy.service".into(),
            state_path: "/var/lib/skillet/ui-host/policy.applied".into(),
            revision: [POLICY.as_bytes(), policy_unit.as_bytes()].concat(),
            definition_changed: changed || unit_changed,
            reload_daemon: true,
            consumer_kind: ConsumerKind::OneShot,
        },
    )?;
    for site in services {
        let name = &site.name;
        let unit = format!("[Unit]\nDescription=Private host UI relay for {name}\nRequires=skillet-ui-host-policy.service\nAfter=skillet-ui-host-policy.service\n[Service]\nExecStart=/var/usrlocal/bin/skillet ui-proxy serve --name {name} --port {}\nRuntimeDirectory=skillet-ui-host\nRuntimeDirectoryMode=0700\nRuntimeDirectoryPreserve=yes\nRestart=on-failure\nUMask=0077\nSELinuxContext=system_u:system_r:skillet_ui_host_t:s0\nNoNewPrivileges=yes\nProtectSystem=strict\nProtectHome=yes\nPrivateTmp=yes\nReadWritePaths=/run/skillet-ui-host\nRestrictAddressFamilies=AF_UNIX AF_INET\nIPAddressDeny=any\nIPAddressAllow=localhost\nTasksMax=300\n[Install]\nWantedBy=multi-user.target\n", site.port);
        let service = format!("skillet-ui-host-{name}.service");
        let changed = files.ensure_file(
            &Path::new("/etc/systemd/system").join(&service),
            unit.as_bytes(),
            Some(0o644),
            &root,
        )?;
        system.service_enable(&service)?;
        activate(
            system,
            files,
            &ActivationRequest {
                service,
                state_path: format!("/var/lib/skillet/ui-host/{name}.applied").into(),
                revision: unit.into_bytes(),
                definition_changed: changed,
                reload_daemon: true,
                consumer_kind: ConsumerKind::Persistent,
            },
        )?;
    }
    volumes.push(Volume {
        host_path: "/run/skillet-ui-host".into(),
        container_path: "/run/skillet-ui-host".into(),
        options: Some("ro,z".into()),
        host_mode: None,
        host_ownership: None,
    });
    Ok(())
}

pub fn serve(name: &str, port: u16) -> Result<(), CaddyError> {
    super::validate_label(name, "host relay name")?;
    if port == 0 {
        return Err(CaddyError::InvalidConfiguration(
            "invalid host relay port".into(),
        ));
    }
    serve_at(
        &Path::new("/run/skillet-ui-host").join(format!("{name}.sock")),
        port,
    )
}

fn serve_at(path: &Path, port: u16) -> Result<(), CaddyError> {
    if let Ok(metadata) = std::fs::symlink_metadata(path) {
        if !metadata.file_type().is_socket() || UnixStream::connect(path).is_ok() {
            return Err(CaddyError::InvalidConfiguration(
                "host relay socket is already in use or unsafe".into(),
            ));
        }
        std::fs::remove_file(path)?;
    }
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    let active = Arc::new(AtomicUsize::new(0));
    for connection in listener.incoming() {
        let stream = connection?;
        if active
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                (n < 128).then_some(n + 1)
            })
            .is_err()
        {
            continue;
        }
        let count = active.clone();
        std::thread::spawn(move || {
            if let Err(error) = relay(stream, port) {
                tracing::debug!(kind = ?error.kind(), "host UI relay connection ended");
            }
            count.fetch_sub(1, Ordering::SeqCst);
        });
    }
    Ok(())
}

fn relay(mut unix: UnixStream, port: u16) -> io::Result<()> {
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let mut tcp = TcpStream::connect_timeout(&address, Duration::from_secs(5))?;
    unix.set_read_timeout(Some(Duration::from_mins(15)))?;
    unix.set_write_timeout(Some(Duration::from_secs(30)))?;
    tcp.set_read_timeout(Some(Duration::from_mins(15)))?;
    tcp.set_write_timeout(Some(Duration::from_secs(30)))?;
    let mut tcp_write = tcp.try_clone()?;
    let mut unix_read = unix.try_clone()?;
    std::thread::scope(|scope| {
        scope.spawn(move || {
            let _ = io::copy(&mut unix_read, &mut tcp_write);
            let _ = tcp_write.shutdown(Shutdown::Write);
        });
        let result = io::copy(&mut tcp, &mut unix);
        let _ = unix.shutdown(Shutdown::Both);
        let _ = tcp.shutdown(Shutdown::Both);
        result.map(|_| ())
    })
}

#[cfg(test)]
#[path = "host_proxy_tests.rs"]
mod tests;

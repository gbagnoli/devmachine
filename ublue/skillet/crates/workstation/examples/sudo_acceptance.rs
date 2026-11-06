//! Disposable acceptance for encrypted SSH keys plus password-required sudo.
use skillet_vm::{
    artifacts,
    backend::{VirshBackend, VmBackend},
    transport::{
        GuestCommand, GuestTransport, HostKeyPolicy, OwnershipCheckedTransport, SshConnection,
        SshTransport,
    },
    ManifestStore, RunIdentity,
};
use skillet_workstation::{
    credential_cache::{CredentialCache, KernelCache},
    sudo::{PasswordPrompt, SudoTransport},
};
use std::{
    error::Error,
    fs,
    io::{Read, Write},
    os::unix::{fs::PermissionsExt, net::UnixListener},
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use zeroize::Zeroizing;
type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn guest(
    transport: &impl GuestTransport,
    program: &str,
    args: &[&str],
    input: Option<&[u8]>,
) -> Result<Output> {
    let output = transport.execute(
        &GuestCommand {
            program,
            arguments: args,
        },
        input,
    )?;
    if !output.status.success() {
        return Err(format!(
            "acceptance guest command failed: {program}, exit {:?}",
            output.status.code()
        )
        .into());
    }
    Ok(output)
}
// Keep the private test agent scoped; never alter the user's agent or environment.
struct Agent(Child);
impl Drop for Agent {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct Account<'a, T: GuestTransport> {
    guest: &'a T,
    name: String,
    sudoers: String,
}
impl<T: GuestTransport> Account<'_, T> {
    fn cleanup(&self) -> Result<()> {
        guest(self.guest, "sudo", &["-n", "rm", "-f", &self.sudoers], None)?;
        // sshd/PAM can keep this generated account's session alive briefly after
        // the client exits. Terminate only its sessions before deleting it.
        let _ = self.guest.execute(
            &GuestCommand {
                program: "sudo",
                arguments: &["-n", "loginctl", "terminate-user", &self.name],
            },
            None,
        )?;
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let output = self.guest.execute(
                &GuestCommand {
                    program: "sudo",
                    arguments: &["-n", "userdel", "--remove", &self.name],
                },
                None,
            )?;
            if output.status.success() || output.status.code() == Some(6) {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "test account {} cleanup failed: {}",
                    self.name,
                    String::from_utf8_lossy(&output.stderr)
                )
                .into());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
}
impl<T: GuestTransport> Drop for Account<'_, T> {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup() {
            eprintln!("{error}");
        }
    }
}

fn key_prompt(listener: UnixListener) -> std::thread::JoinHandle<std::io::Result<()>> {
    std::thread::spawn(move || -> std::io::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut request = [0; 24];
                    stream.read_exact(&mut request)?;
                    if &request != b"skillet-sudo-askpass-v1\n" {
                        return Err(std::io::Error::other("bad test askpass request"));
                    }
                    stream.write_all(b"test-only-key-passphrase\n")?;
                    return Ok(());
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err(std::io::Error::other("key prompt timed out"));
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(e) => return Err(e),
            }
        }
    })
}

fn encrypted_agent(helper: &Path, directory: &Path) -> Result<(Agent, PathBuf, PathBuf)> {
    let key = directory.join("identity");
    // This fixed passphrase protects only a newly generated disposable test key.
    let mut generate = Command::new("ssh-keygen");
    generate
        .args([
            "-q",
            "-t",
            "ed25519",
            "-N",
            "test-only-key-passphrase",
            "-f",
        ])
        .arg(&key);
    if !skillet_vm::capture_command(generate, Duration::from_secs(20))?
        .status
        .success()
    {
        return Err("test key generation failed".into());
    }
    let agent_socket = directory.join("agent.sock");
    let mut agent = Command::new("ssh-agent");
    agent
        .args(["-D", "-a"])
        .arg(&agent_socket)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let agent_guard = Agent(agent.spawn()?);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !agent_socket.exists() {
        if Instant::now() >= deadline {
            return Err("test agent did not start".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let local_helper = directory.join("helper");
    fs::copy(helper, &local_helper)?;
    fs::set_permissions(&local_helper, fs::Permissions::from_mode(0o700))?;
    let listener = UnixListener::bind(directory.join("askpass.sock"))?;
    listener.set_nonblocking(true)?;
    let key_prompt = key_prompt(listener);
    let mut add = Command::new("ssh-add");
    add.arg(&key)
        .env("SSH_AUTH_SOCK", &agent_socket)
        .env("SSH_ASKPASS", &local_helper)
        .env("SSH_ASKPASS_REQUIRE", "force")
        .env("DISPLAY", "skillet-test");
    if !skillet_vm::capture_command(add, Duration::from_secs(25))?
        .status
        .success()
    {
        return Err("encrypted test key did not unlock".into());
    }
    key_prompt
        .join()
        .map_err(|_| "key prompt thread failed")??;

    Ok((agent_guard, key, agent_socket))
}

fn create_account<'a, T: GuestTransport>(
    trusted: &'a T,
    key: &Path,
) -> Result<(Account<'a, T>, Zeroizing<String>)> {
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let user = format!("sk-sudo-{}", &suffix[..10]);
    guest(
        trusted,
        "sudo",
        &[
            "-n",
            "useradd",
            "--create-home",
            "--shell",
            "/bin/bash",
            &user,
        ],
        None,
    )?;
    let account = Account {
        guest: trusted,
        name: user.clone(),
        sudoers: format!("/etc/sudoers.d/{user}"),
    };
    let password = Zeroizing::new(uuid::Uuid::new_v4().simple().to_string());
    let chpasswd = Zeroizing::new(format!("{user}:{}\n", password.as_str()));
    guest(
        trusted,
        "sudo",
        &["-n", "chpasswd"],
        Some(chpasswd.as_bytes()),
    )?;
    let policy = format!("{user} ALL=(ALL) ALL\n");
    guest(
        trusted,
        "sudo",
        &["-n", "tee", &account.sudoers],
        Some(policy.as_bytes()),
    )?;
    guest(
        trusted,
        "sudo",
        &["-n", "chmod", "0440", &account.sudoers],
        None,
    )?;
    guest(
        trusted,
        "sudo",
        &["-n", "visudo", "-cf", &account.sudoers],
        None,
    )?;
    let home = format!("/home/{user}");
    guest(
        trusted,
        "sudo",
        &[
            "-n",
            "install",
            "-d",
            "-m",
            "0700",
            "-o",
            &user,
            "-g",
            &user,
            &format!("{home}/.ssh"),
        ],
        None,
    )?;
    let public = fs::read(key.with_extension("pub"))?;
    guest(
        trusted,
        "sudo",
        &["-n", "tee", &format!("{home}/.ssh/authorized_keys")],
        Some(&public),
    )?;
    guest(
        trusted,
        "sudo",
        &[
            "-n",
            "chown",
            &format!("{user}:{user}"),
            &format!("{home}/.ssh/authorized_keys"),
        ],
        None,
    )?;
    guest(
        trusted,
        "sudo",
        &[
            "-n",
            "chmod",
            "0600",
            &format!("{home}/.ssh/authorized_keys"),
        ],
        None,
    )?;
    guest(trusted, "sudo", &["-n", "restorecon", "-RF", &home], None)?;
    Ok((account, password))
}

struct CacheCleanup(Arc<dyn CredentialCache>, [u8; 32]);
impl Drop for CacheCleanup {
    fn drop(&mut self) {
        let _ = self.0.forget(&self.1);
    }
}

fn authentication_checks<A, T>(
    actor: A,
    trusted: T,
    workspace: &Path,
    password: Zeroizing<String>,
) -> Result<()>
where
    A: GuestTransport
        + skillet_vm::transport::ReverseUnixForward
        + skillet_vm::transport::CredentialIdentity
        + Clone,
    T: GuestTransport
        + skillet_vm::transport::ReverseUnixForward
        + skillet_vm::transport::CredentialIdentity
        + Clone,
{
    assert!(!actor
        .execute(
            &GuestCommand {
                program: "sudo",
                arguments: &["-n", "-k", "true"]
            },
            None
        )?
        .status
        .success());
    let prompts = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&prompts);
    let prompt: PasswordPrompt = Arc::new(move || {
        if counter.fetch_add(1, Ordering::SeqCst) == 0 {
            return Ok(Zeroizing::new("test-only-incorrect-password".into()));
        }
        Ok(Zeroizing::new(password.to_string()))
    });
    let cache: Arc<dyn CredentialCache> = Arc::new(KernelCache::new("sudo-acceptance"));
    let scope = actor
        .credential_scope()?
        .ok_or("test SSH trust binding unavailable")?;
    let _cleanup = CacheCleanup(Arc::clone(&cache), scope);
    cache.forget(&scope)?;
    let authenticated = SudoTransport::with_cache(
        actor.clone(),
        workspace.to_owned(),
        Some(prompt),
        Arc::clone(&cache),
    );
    let payload = b"binary stdin stays unchanged\x00\x01\xff\r\n";
    println!("Acceptance: password-required sudo with rejected first password");
    let first = guest(&authenticated, "sudo", &["-n", "-k", "cat"], Some(payload))?;
    assert_eq!(first.stdout, payload);
    println!("Acceptance: repeat sudo with cached corrected password");
    let uid = guest(&authenticated, "sudo", &["-n", "-k", "id", "-u"], None)?;
    assert_eq!(uid.stdout, b"0\n");
    assert_eq!(prompts.load(Ordering::SeqCst), 2);
    println!("Acceptance: close password-required sudo channel");
    drop(authenticated);
    println!("Acceptance: new transport reuses kernel cache without a terminal");
    assert!(cache.read(&scope)?.is_some());
    let noninteractive =
        SudoTransport::with_cache(actor, workspace.to_owned(), None, Arc::clone(&cache));
    assert_eq!(
        guest(&noninteractive, "sudo", &["-n", "-k", "cat"], Some(payload))?.stdout,
        payload
    );
    println!("Acceptance: explicit cache eviction prevents unattended authentication");
    cache.forget(&scope)?;
    assert!(noninteractive
        .execute(
            &GuestCommand {
                program: "sudo",
                arguments: &["-n", "-k", "true"]
            },
            None
        )
        .is_err());
    println!("Acceptance: no-password sudo with binary stdin");
    let trusted_scope = trusted
        .credential_scope()?
        .ok_or("trusted SSH binding unavailable")?;
    let nopasswd = SudoTransport::with_cache(
        trusted,
        workspace.to_owned(),
        Some(Arc::new(|| {
            Err(std::io::Error::other("NOPASSWD must not prompt"))
        })),
        Arc::clone(&cache),
    );
    assert_eq!(
        guest(&nopasswd, "sudo", &["-n", "-k", "cat"], Some(payload))?.stdout,
        payload
    );
    println!("Acceptance: close no-password sudo channel");
    assert!(cache.read(&trusted_scope)?.is_none());
    drop(nopasswd);
    drop(noninteractive);
    Ok(())
}

fn helper_inventory(guest: &impl GuestTransport) -> Result<std::collections::BTreeSet<String>> {
    let output = crate::guest(
        guest,
        "find",
        &[
            "/var/tmp",
            "-maxdepth",
            "1",
            "-type",
            "d",
            "-name",
            "skillet-sudo-*",
            "-printf",
            "%f\n",
        ],
        None,
    )?;
    Ok(String::from_utf8(output.stdout)?
        .lines()
        .map(str::to_owned)
        .collect::<std::collections::BTreeSet<_>>())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: sudo_acceptance BUTANE HOST INSTANCE".into());
    }
    let butane = PathBuf::from(&args[0]);
    let identity = RunIdentity::new(&args[1], &args[2])?;
    let store = ManifestStore::new(&butane.join("runs"), skillet_vm::current_uid())?;
    let _lock = store.lock(&identity)?;
    let run = store.load(&identity)?;
    let backend = VirshBackend::for_run(&run, &butane.join("bin/virsh"))?;
    let guard = || {
        backend
            .inspect(&run)?
            .ok_or("owned VM absent")
            .map_err(|_| skillet_vm::Error::Invalid("owned VM absent".into()))?
            .validate_owned(&run)
    };
    guard()?;
    let ssh = SshTransport::new(run.ssh.clone(), HostKeyPolicy::Verify)?;
    let trusted = OwnershipCheckedTransport::new(&ssh, &guard);
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("workspace absent")?;
    let built = artifacts::build(&artifacts::BuildRequest {
        workspace,
        packages: &["skillet-sudo-askpass"],
        target: "x86_64-unknown-linux-musl",
        profile: "release",
    })?;
    let helper = built.get("skillet-sudo-askpass").ok_or("helper absent")?;
    let temporary = tempfile::Builder::new()
        .prefix("skillet-sudo-acceptance-")
        .tempdir()?;
    println!("Acceptance: unlock encrypted test key in isolated agent");
    let (_agent, key, agent_socket) = encrypted_agent(helper, temporary.path())?;
    println!("Acceptance: create password-required disposable account");
    let (account, password) = create_account(&trusted, &key)?;
    println!("Acceptance: disposable account {}", account.name);
    let user = account.name.clone();
    let actor = SshTransport::workstation(SshConnection {
        user: Some(user),
        address: run.ssh.address.clone(),
        port: Some(run.ssh.port),
        identity: Some(key),
        known_hosts: Some(run.ssh.known_hosts.clone()),
    })?
    .with_agent_socket(agent_socket)?;
    let actor = OwnershipCheckedTransport::new(&actor, &guard);
    let original_helpers = helper_inventory(&trusted)?;
    authentication_checks(actor, trusted.clone(), workspace, password)?;
    assert_eq!(helper_inventory(&trusted)?, original_helpers);
    println!("Acceptance: verify helper and account cleanup");
    let account_name = account.name.clone();
    let sudoers = account.sudoers.clone();
    account.cleanup()?;
    drop(account);
    assert!(!trusted
        .execute(
            &GuestCommand {
                program: "id",
                arguments: &["-u", &account_name]
            },
            None
        )?
        .status
        .success());
    assert!(!trusted
        .execute(
            &GuestCommand {
                program: "sudo",
                arguments: &["-n", "test", "-e", &sudoers]
            },
            None
        )?
        .status
        .success());
    guard()?;
    println!("Passed: encrypted SSH key via isolated agent, independent sudo password with rejection/re-prompt, cached repeat operations, binary stdin preservation, NOPASSWD and noninteractive refusal; test account/helpers removed");
    Ok(())
}

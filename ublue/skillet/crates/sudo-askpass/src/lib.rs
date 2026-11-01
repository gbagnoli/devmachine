//! Minimal guest askpass client. Passwords travel over a private SSH-forwarded socket.
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::Path,
    time::Duration,
};
use zeroize::Zeroizing;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("sudo askpass channel failed")]
    Io(#[from] std::io::Error),
    #[error("invalid sudo askpass response")]
    Invalid,
}

pub fn run(executable: &Path, output: &mut impl Write) -> Result<(), Error> {
    let directory = executable.parent().ok_or(Error::Invalid)?;
    let mut stream = UnixStream::connect(directory.join("askpass.sock"))?;
    stream.set_read_timeout(Some(Duration::from_mins(5)))?;
    // The request is nonsecret; the server never puts a password on command stdin.
    stream.write_all(b"skillet-sudo-askpass-v1\n")?;
    let mut password = Zeroizing::new(Vec::new());
    stream.take(4097).read_to_end(&mut password)?;
    if password.is_empty()
        || password.len() > 4096
        || password.contains(&0)
        || password[..password.len() - 1].contains(&b'\n')
        || password.last() != Some(&b'\n')
    {
        return Err(Error::Invalid);
    }
    output.write_all(&password)?;
    Ok(())
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

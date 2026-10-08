//! Apply the production `UniFi` module alone on an already prepared disposable guest.
//! This test driver does not import backups or configure other host services.
use skillet_core::{files::LocalFileResource, system::LinuxSystemResource};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    skillet_unifi::apply(&LinuxSystemResource::new(), &LocalFileResource::new())?;
    Ok(())
}

//! USB observations and rule refresh owned by the concrete host adapter.
use crate::system::{LinuxSystemResource, SystemError};
use std::{fs, io, path::Path, process::Command};

pub trait UsbResource {
    fn usb_device_present(&self, vendor: u16, product: u16) -> Result<bool, SystemError>;
    fn refresh_usb_rules(&self, vendor: u16, product: u16) -> Result<(), SystemError>;
}

impl UsbResource for LinuxSystemResource {
    fn usb_device_present(&self, vendor: u16, product: u16) -> Result<bool, SystemError> {
        device_present(Path::new("/sys/bus/usb/devices"), vendor, product)
    }
    fn refresh_usb_rules(&self, vendor: u16, product: u16) -> Result<(), SystemError> {
        for args in [
            vec![
                "control".into(),
                "--timeout=10".into(),
                "--reload-rules".into(),
            ],
            vec![
                "trigger".into(),
                "--action=add".into(),
                "--subsystem-match=usb".into(),
                format!("--attr-match=idVendor={vendor:04x}"),
                format!("--attr-match=idProduct={product:04x}"),
            ],
            vec!["settle".into(), "--timeout=10".into()],
        ] {
            if !Command::new("udevadm").args(args).status()?.success() {
                return Err(SystemError::Command("USB rule refresh failed".into()));
            }
        }
        Ok(())
    }
}

fn device_present(root: &Path, vendor: u16, product: u16) -> Result<bool, SystemError> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    for entry in entries {
        let entry = entry?;
        let read_id = |name: &str| -> Result<Option<u16>, SystemError> {
            match fs::read_to_string(entry.path().join(name)) {
                Ok(value) => u16::from_str_radix(value.trim(), 16)
                    .map(Some)
                    .map_err(|_| SystemError::Command("invalid USB identifier in sysfs".into())),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(error.into()),
            }
        };
        if read_id("idVendor")? == Some(vendor) && read_id("idProduct")? == Some(product) {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
#[path = "usb/tests.rs"]
mod tests;

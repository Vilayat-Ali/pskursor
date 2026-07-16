use hidapi::HidApi;
use regex::Regex;
use std::{collections::HashSet, error::Error, ffi::CString};

/// Represents details about a connected Human Interface Device (HID).
#[derive(Debug, Hash, PartialEq, Eq)]
pub struct DeviceInfo {
    /// The Vendor ID (VID) assigned by the USB-IF or relevant standard body.
    pub vendor_id: u16,

    /// The Product ID (PID) assigned by the manufacturer.
    pub product_id: u16,

    /// The user-friendly product string of the device, if exposed.
    pub device_name: Option<String>,

    /// The serial number or MAC address of the device, if available.
    pub mac_addr: Option<String>,

    /// The OS-specific system path used to open the raw device connection.
    pub path: CString,
}

impl DeviceInfo {
    pub fn get_mac_address(&self) -> String {
        match &self.mac_addr {
            Some(addr) => {
                if addr.is_empty() {
                    String::from(" - ")
                } else {
                    addr.clone()
                }
            }
            None => String::from(" - "),
        }
    }

    pub fn get_device_name(&self) -> String {
        match &self.device_name {
            Some(device_name) => {
                if device_name.is_empty() {
                    String::from(" UNNAMED ")
                } else {
                    device_name.clone()
                }
            }
            None => String::from(" UNNAMED "),
        }
    }
}

fn get_device_names_to_ignore() -> Vec<Regex> {
    vec![
            // Ignore ELAN touchpads
           Regex::new(
            r"^(?P<acpi_id>[A-Z0-9]+):(?P<acpi_idx>\d{2})\s+(?P<vendor_id>[0-9A-F]{4}):(?P<product_id>[0-9A-F]{4})$"
           ).unwrap()
        ]
}

pub fn should_ignore_device_by_name(device_name: &str) -> bool {
    for regex in get_device_names_to_ignore() {
        if !regex.is_match(device_name) {
            return false;
        }
    }

    true
}

/// Finds unique HID devices connected to the system. Filters out duplicate devices based on their product string.
///
/// Returns an error if the `HidApi` instance fails to initialize.
pub fn get_devices() -> Result<Vec<DeviceInfo>, Box<dyn Error>> {
    let api = HidApi::new()?;
    let mut devices = HashSet::new();

    for device in api.device_list() {
        let name = device.product_string().unwrap_or(" UNNAMED ");
        if !name.is_empty() && !should_ignore_device_by_name(name) {
            devices.insert(DeviceInfo {
                vendor_id: device.vendor_id(),
                product_id: device.product_id(),
                device_name: Some(name.to_owned()),
                mac_addr: device.serial_number().map(|s| s.to_owned()),
                path: device.path().to_owned(),
            });
        }
    }

    Ok(devices.into_iter().collect::<Vec<DeviceInfo>>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanity_check_port_detection() {
        let devices = get_devices().unwrap();

        assert!(devices.len() > 0)
    }
}

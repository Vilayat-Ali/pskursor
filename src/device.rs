use hidapi::HidApi;
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    ffi::CString,
};

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

/// Finds unique HID devices connected to the system. Filters out duplicate devices based on their product string.
///
/// Returns an error if the `HidApi` instance fails to initialize.
pub fn get_devices() -> Result<HashMap<u16, DeviceInfo>, Box<dyn Error>> {
    let api = HidApi::new()?;
    let mut device_names = HashSet::new();
    let mut devices_map = HashMap::new();

    for device in api.device_list() {
        let name = device.product_string().unwrap_or_default();
        if !name.is_empty() && device_names.insert(name.to_owned()) {
            devices_map.insert(
                device.product_id(),
                DeviceInfo {
                    vendor_id: device.vendor_id(),
                    product_id: device.product_id(),
                    device_name: Some(name.to_owned()),
                    mac_addr: device.serial_number().map(|s| s.to_owned()),
                    path: device.path().to_owned(),
                },
            );
        }
    }

    Ok(devices_map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanity_check_port_detection() {
        let devices = get_devices().unwrap();

        assert!(devices.len() > 0)
    }

    #[test]
    fn test_ps5_controller_detection() {
        let ps5_controller_product_id: u16 = 3302;
        let devices = get_devices().unwrap();

        assert!(devices.get(&ps5_controller_product_id).is_some());
    }
}

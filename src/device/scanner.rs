use anyhow::{Context, Result};
use evdev::Device;
use std::path::PathBuf;

/// Information about a discovered input device
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub path: PathBuf,
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub is_mouse: bool,
    /// Human readable capabilities summary
    pub capabilities: String,
}

/// Scan /dev/input for available input devices, filtering for mice
pub fn scan_devices() -> Result<Vec<DeviceInfo>> {
    let mut devices = Vec::new();

    for entry in std::fs::read_dir("/dev/input")
        .context("Failed to read /dev/input (are you running as root?)")?
    {
        let entry = entry?;
        let path = entry.path();

        // Only look at eventN files
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !file_name.starts_with("event") {
            continue;
        }

        match open_device_info(&path) {
            Ok(info) => devices.push(info),
            Err(e) => {
                log::debug!("Skipping {}: {}", path.display(), e);
            }
        }
    }

    // Sort by path for consistent ordering
    devices.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(devices)
}

/// Scan and return only mouse devices
pub fn scan_mice() -> Result<Vec<DeviceInfo>> {
    Ok(scan_devices()?.into_iter().filter(|d| d.is_mouse).collect())
}

fn open_device_info(path: &PathBuf) -> Result<DeviceInfo> {
    let device =
        Device::open(path).with_context(|| format!("Failed to open {}", path.display()))?;

    let name = device.name().unwrap_or("Unknown").to_string();
    let input_id = device.input_id();
    let vendor_id = input_id.vendor();
    let product_id = input_id.product();

    // Detect if this is a mouse: must have relative axes (REL_X, REL_Y) and mouse buttons
    let has_rel = device.supported_relative_axes().is_some_and(|rel| {
        rel.contains(evdev::RelativeAxisCode::REL_X) && rel.contains(evdev::RelativeAxisCode::REL_Y)
    });

    let has_mouse_btn = device.supported_keys().is_some_and(|keys| {
        keys.contains(evdev::KeyCode::BTN_LEFT) && keys.contains(evdev::KeyCode::BTN_RIGHT)
    });

    let is_mouse = has_rel && has_mouse_btn;

    // Build capabilities summary
    let mut caps = Vec::new();
    if has_rel {
        caps.push("relative-axes");
    }
    if has_mouse_btn {
        caps.push("mouse-buttons");
    }
    if device
        .supported_keys()
        .is_some_and(|keys| keys.contains(evdev::KeyCode::KEY_A))
    {
        caps.push("keyboard");
    }
    if device.supported_absolute_axes().is_some() {
        caps.push("absolute-axes");
    }

    Ok(DeviceInfo {
        path: path.clone(),
        name,
        vendor_id,
        product_id,
        is_mouse,
        capabilities: caps.join(", "),
    })
}

/// Find a device matching the given config criteria
pub fn find_device(
    name: Option<&str>,
    path: Option<&str>,
    vendor_id: Option<u16>,
    product_id: Option<u16>,
) -> Result<Option<DeviceInfo>> {
    let devices = scan_devices()?;

    for device in &devices {
        // If path is specified, match exactly
        if let Some(p) = path {
            if device.path.to_str() == Some(p) {
                return Ok(Some(device.clone()));
            }
        }

        // If vendor/product specified, match those
        if let (Some(vid), Some(pid)) = (vendor_id, product_id) {
            if device.vendor_id == vid && device.product_id == pid {
                return Ok(Some(device.clone()));
            }
        }

        // Match by name substring
        if let Some(n) = name {
            if device.name.to_lowercase().contains(&n.to_lowercase()) && device.is_mouse {
                return Ok(Some(device.clone()));
            }
        }
    }

    Ok(None)
}

/// List all button/key codes supported by a device at the given path
pub fn get_device_buttons(path: &PathBuf) -> Result<Vec<evdev::KeyCode>> {
    let device =
        Device::open(path).with_context(|| format!("Failed to open {}", path.display()))?;

    let mut buttons = Vec::new();
    if let Some(keys) = device.supported_keys() {
        for key in keys.iter() {
            buttons.push(key);
        }
    }

    Ok(buttons)
}

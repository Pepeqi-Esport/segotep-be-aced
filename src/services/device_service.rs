use anyhow::{bail, Context, Result};
use hidapi::{HidApi, HidDevice};

use crate::services::protocol_service::DeviceMode;
use crate::utils::constants::{LEGACY_PID, LEGACY_VID, SCREEN_PID, SCREEN_VID};

#[derive(Debug, Clone)]
pub struct HidDeviceInfo {
    pub vendor_id: u16,
    pub product_id: u16,
    pub product_string: Option<String>,
    pub path: String,
    pub mode: Option<DeviceMode>,
}

pub fn enumerate_devices(api: &HidApi) -> Vec<HidDeviceInfo> {
    api.device_list()
        .map(|d| {
            let mode = match (d.vendor_id(), d.product_id()) {
                (SCREEN_VID, SCREEN_PID) => Some(DeviceMode::Screen),
                (LEGACY_VID, LEGACY_PID) => Some(DeviceMode::Legacy),
                _ => None,
            };
            HidDeviceInfo {
                vendor_id: d.vendor_id(),
                product_id: d.product_id(),
                product_string: d.product_string().map(str::to_string),
                path: d.path().to_string_lossy().into_owned(),
                mode,
            }
        })
        .collect()
}

pub fn open_be_aced(
    api: &HidApi,
    mode: DeviceMode,
    vendor_id: Option<u16>,
    product_id: Option<u16>,
) -> Result<HidDevice> {
    let (default_vid, default_pid) = mode.vid_pid();
    let vid = vendor_id.unwrap_or(default_vid);
    let pid = product_id.unwrap_or(default_pid);

    let candidates: Vec<_> = api
        .device_list()
        .filter(|d| d.vendor_id() == vid && d.product_id() == pid)
        .collect();

    if candidates.is_empty() {
        bail!(
            "BeAced HID not found (VID={vid:#06X} PID={pid:#06X}, mode={mode:?}). \
             Close the official Segotep Digital app if it has exclusive access, \
             then unplug/replug the USB header cable."
        );
    }

    // Prefer the first openable interface (device may expose multiple).
    let mut last_err = None;
    for info in &candidates {
        match info.open_device(api) {
            Ok(dev) => {
                tracing::info!(
                    vid = format_args!("{vid:#06X}"),
                    pid = format_args!("{pid:#06X}"),
                    path = %info.path().to_string_lossy(),
                    "opened HID device"
                );
                return Ok(dev);
            }
            Err(err) => {
                tracing::warn!(
                    path = %info.path().to_string_lossy(),
                    error = %err,
                    "failed to open HID interface"
                );
                last_err = Some(err);
            }
        }
    }

    Err(last_err
        .map(|e| anyhow::anyhow!(e))
        .unwrap_or_else(|| anyhow::anyhow!("unable to open HID device")))
    .context("open BeAced HID")
}

pub fn write_report(device: &HidDevice, report: &[u8]) -> Result<()> {
    let written = device.write(report).context("HID write")?;
    if written == 0 {
        bail!("HID write returned 0 bytes (device may be busy or report rejected)");
    }
    tracing::debug!(bytes = %crate::services::protocol_service::format_hex(report), written, "HID report sent");
    Ok(())
}

pub fn detect_default_mode(api: &HidApi) -> DeviceMode {
    let list = enumerate_devices(api);
    if list.iter().any(|d| d.mode == Some(DeviceMode::Screen)) {
        DeviceMode::Screen
    } else if list.iter().any(|d| d.mode == Some(DeviceMode::Legacy)) {
        DeviceMode::Legacy
    } else {
        // BeAced ships with empty `official/screen` marker → screen protocol.
        DeviceMode::Screen
    }
}

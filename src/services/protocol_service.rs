//! HID report builders reconstructed from official `LEDDisplay.Form1`
//! (`SendScreenValues` / `SendLatestValues`).

use crate::config::FlashProfile;
use crate::services::sensor_service::SensorSnapshot;
use crate::utils::constants::{LEGACY_PID, LEGACY_VID, SCREEN_PID, SCREEN_VID};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceMode {
    /// `SendScreenValues` — VID 0x0416 / PID 0x6001
    Screen,
    /// `SendLatestValues` — VID 0x1A86 / PID 0xA001
    Legacy,
}

impl DeviceMode {
    pub fn vid_pid(self) -> (u16, u16) {
        match self {
            Self::Screen => (SCREEN_VID, SCREEN_PID),
            Self::Legacy => (LEGACY_VID, LEGACY_PID),
        }
    }
}

/// Build an outbound HID report matching the official app.
pub fn build_report(
    mode: DeviceMode,
    screen_on: bool,
    flash: &FlashProfile,
    sensors: &SensorSnapshot,
    fahrenheit: bool,
) -> Vec<u8> {
    match mode {
        DeviceMode::Screen => build_screen_values(screen_on, sensors),
        DeviceMode::Legacy => build_latest_values(screen_on, flash, sensors, fahrenheit),
    }
}

/// Official `SendScreenValues` packet (11 bytes including report id 0).
///
/// Layout:
/// `[0x00, 0xDC, 0xD6, 0xD8, 0xDC|0xDD, load, temp, power_le16, clock_le16]`
fn build_screen_values(screen_on: bool, sensors: &SensorSnapshot) -> Vec<u8> {
    let mut pkt = vec![0u8, 220, 214, 216, 220];
    if !screen_on {
        pkt[4] = 221;
    }
    let temp = sensors.cpu_temperature.clamp(0.0, 255.0) as u8;
    let load = sensors.cpu_load.clamp(0.0, 255.0) as u8;
    let clock = sensors.cpu_clock.round().clamp(0.0, u16::MAX as f32) as u16;
    let power = sensors.cpu_power.round().clamp(0.0, u16::MAX as f32) as u16;
    pkt.extend_from_slice(&[
        load,
        temp,
        (power & 0xFF) as u8,
        (power >> 8) as u8,
        (clock & 0xFF) as u8,
        (clock >> 8) as u8,
    ]);
    pkt
}

/// Official `SendLatestValues` packet (34 bytes including report id 0).
fn build_latest_values(
    screen_on: bool,
    flash: &FlashProfile,
    sensors: &SensorSnapshot,
    fahrenheit: bool,
) -> Vec<u8> {
    let mut header = vec![
        0u8, 220, 221, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 12, 0, 0, 0,
    ];
    header[4] = flash.index;

    let cpu_temp = sensors.cpu_temperature.clamp(0.0, 255.0) as u8;
    let cpu_load = sensors.cpu_load.clamp(0.0, 255.0) as u8;
    let cpu_clock = sensors.cpu_clock.round().clamp(0.0, u16::MAX as f32) as u16;
    let cpu_power = sensors.cpu_power.round().clamp(0.0, u16::MAX as f32) as u16;
    let gpu_temp = sensors.gpu_temperature.clamp(0.0, 255.0) as u8;
    let gpu_load = sensors.gpu_load.clamp(0.0, 255.0) as u8;
    let gpu_clock = sensors.gpu_clock.round().clamp(0.0, u16::MAX as f32) as u16;
    let gpu_power = sensors.gpu_power.round().clamp(0.0, u16::MAX as f32) as u16;

    let mut payload = vec![
        cpu_temp,
        cpu_load,
        (cpu_power & 0xFF) as u8,
        (cpu_power >> 8) as u8,
        (cpu_clock & 0xFF) as u8,
        (cpu_clock >> 8) as u8,
        gpu_temp,
        gpu_load,
        (gpu_power & 0xFF) as u8,
        (gpu_power >> 8) as u8,
        (gpu_clock & 0xFF) as u8,
        (gpu_clock >> 8) as u8,
        u8::from(fahrenheit),
    ];

    header.append(&mut payload);
    if !screen_on {
        // Official sets bytes 3..4 to 0x0E, 0x0F when screen is off.
        header[3] = 14;
        header[4] = 15;
    }
    header
}

/// Hex dump helper for capture validation / logging.
pub fn format_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FlashProfile;
    use crate::services::sensor_service::SensorSnapshot;

    #[test]
    fn screen_packet_matches_official_layout() {
        let sensors = SensorSnapshot {
            cpu_temperature: 45.0,
            cpu_load: 12.0,
            cpu_clock: 3600.0,
            cpu_power: 65.0,
            ..SensorSnapshot::default()
        };
        let pkt = build_screen_values(true, &sensors);
        assert_eq!(pkt.len(), 11);
        assert_eq!(&pkt[..5], &[0, 220, 214, 216, 220]);
        assert_eq!(pkt[5], 12); // load
        assert_eq!(pkt[6], 45); // temp
        assert_eq!(u16::from_le_bytes([pkt[7], pkt[8]]), 65); // power
        assert_eq!(u16::from_le_bytes([pkt[9], pkt[10]]), 3600); // clock
    }

    #[test]
    fn screen_off_sets_trailer() {
        let pkt = build_screen_values(false, &SensorSnapshot::default());
        assert_eq!(pkt[4], 221);
    }

    #[test]
    fn legacy_packet_includes_flash_index() {
        let flash = FlashProfile {
            index: 3,
            ..FlashProfile::default()
        };
        let pkt = build_latest_values(true, &flash, &SensorSnapshot::default(), false);
        assert_eq!(pkt.len(), 34);
        assert_eq!(pkt[4], 3);
        assert_eq!(&pkt[..3], &[0, 220, 221]);
    }
}

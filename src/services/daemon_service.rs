use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use hidapi::HidApi;

use crate::config::AppConfig;
use crate::services::device_service::{open_be_aced, write_report};
use crate::services::protocol_service::{build_report, format_hex, DeviceMode};
use crate::services::sensor_service::SensorReader;

pub struct DaemonOptions {
    pub mode: DeviceMode,
    pub screen_on: bool,
    pub dry_run: bool,
    pub once: bool,
    pub quiet: bool,
}

pub fn run_daemon(cfg: &AppConfig, opts: DaemonOptions) -> Result<()> {
    let api = HidApi::new().context("init hidapi")?;
    let mut sensors = SensorReader::new();

    let device = if opts.dry_run {
        None
    } else {
        Some(open_be_aced(
            &api,
            opts.mode,
            cfg.vendor_id,
            cfg.product_id,
        )?)
    };

    tracing::info!(
        mode = ?opts.mode,
        interval_ms = cfg.interval_ms,
        screen_on = opts.screen_on,
        dry_run = opts.dry_run,
        "daemon starting"
    );

    loop {
        let snap = sensors.poll();
        let report = build_report(
            opts.mode,
            opts.screen_on,
            &cfg.flash,
            &snap,
            false,
        );

        if !opts.quiet {
            println!("{} | {}", snap.summary_line(), format_hex(&report));
        }

        if let Some(dev) = &device {
            if let Err(err) = write_report(dev, &report) {
                tracing::error!(error = %err, "failed to write HID report");
            }
        }

        if opts.once {
            break;
        }
        thread::sleep(Duration::from_millis(cfg.interval_ms.max(100)));
    }

    Ok(())
}

pub fn send_screen_state(cfg: &AppConfig, mode: DeviceMode, screen_on: bool) -> Result<()> {
    let api = HidApi::new().context("init hidapi")?;
    let mut sensors = SensorReader::new();
    let snap = sensors.poll();
    let report = build_report(mode, screen_on, &cfg.flash, &snap, false);
    let device = open_be_aced(&api, mode, cfg.vendor_id, cfg.product_id)?;
    write_report(&device, &report)?;
    println!(
        "screen {} → {}",
        if screen_on { "on" } else { "off" },
        format_hex(&report)
    );
    Ok(())
}

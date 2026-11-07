//! HID / tray / quit actions for [`HomePage`].

use hidapi::HidApi;

use crate::helpers::tray_helper;
use crate::services::background_service;
use crate::services::device_service::{open_be_aced, write_report};
use crate::services::protocol_service::build_report;
use crate::views::components::Metric;
use crate::views::pages::HomePage;

impl HomePage {
    pub(crate) fn reopen_hid(&mut self) {
        self.sensors.ensure_bridge();
        match HidApi::new() {
            Ok(api) => match open_be_aced(
                &api,
                self.mode,
                self.cfg.vendor_id,
                self.cfg.product_id,
            ) {
                Ok(d) => {
                    self.device = Some(d);
                    self.device_error = None;
                }
                Err(e) => {
                    self.device = None;
                    self.device_error = Some(e.to_string());
                }
            },
            Err(e) => {
                self.device = None;
                self.device_error = Some(e.to_string());
            }
        }
    }

    pub(crate) fn handoff_to_tray(&mut self) {
        self.device = None;
        self.sensors.stop_bridge();
        if let Err(err) = background_service::start(self.screen_on) {
            self.status = format!("Tray handoff failed: {err}");
            self.sensors.ensure_bridge();
            self.reopen_hid();
            self.tray_hidden = false;
            tray_helper::show_window();
            return;
        }
        self.tray_hidden = true;
        tray_helper::hide_window();
        self.status = "Running in tray".into();
    }

    pub(crate) fn resume_from_tray(&mut self) {
        if !self.tray_hidden && self.device.is_some() {
            tray_helper::show_window();
            return;
        }
        background_service::stop();
        self.tray_hidden = false;
        self.reopen_hid();
        tray_helper::show_window();
        self.status = "Opened from tray".into();
    }

    pub(crate) fn visible_metrics(&self) -> Vec<Metric> {
        Metric::all()
            .into_iter()
            .filter(|m| self.flash_flags & m.bit() != 0)
            .collect()
    }

    pub(crate) fn push_hid(&mut self) {
        let report = build_report(
            self.mode,
            self.screen_on,
            &self.cfg.flash,
            &self.snap,
            false,
        );
        if let Some(dev) = &self.device {
            match write_report(dev, &report) {
                Ok(()) => {
                    self.device_error = None;
                    self.status = format!(
                        "Synced · {}",
                        crate::services::protocol_service::format_hex(&report)
                    );
                }
                Err(e) => {
                    self.status = format!("Write failed: {e}");
                    self.device_error = Some(e.to_string());
                }
            }
        } else if let Some(err) = &self.device_error {
            self.status = err.clone();
        }
    }

    pub(crate) fn request_quit(&mut self) {
        self.quit_fully = true;
        background_service::stop();
        self.screen_on = false;
        self.push_hid();
        self.device = None;
        self.sensors.stop_bridge_and_sweep();
        crate::services::sensor_service::kill_all_sensor_bridges();
        if let Some(tray_icon) = self.tray.take() {
            tray_helper::dispose(tray_icon);
        }
        tray_helper::kill_all_app_processes();
        std::process::exit(0);
    }

    pub(crate) fn tick(&mut self) {
        if self.quit_fully || self.tray_hidden {
            return;
        }
        if self.last_tick.elapsed() < self.interval {
            return;
        }
        self.last_tick = std::time::Instant::now();
        self.snap = self.sensors.poll();
        self.push_hid();
    }

    pub(crate) fn metric_value(&self, m: Metric) -> (f32, &'static str) {
        match m {
            Metric::CpuTemp => (self.snap.cpu_temperature, "°C"),
            Metric::CpuLoad => (self.snap.cpu_load, "%"),
            Metric::CpuClock => (self.snap.cpu_clock, "MHz"),
            Metric::CpuPower => (self.snap.cpu_power, "W"),
            Metric::GpuTemp => (self.snap.gpu_temperature, "°C"),
            Metric::GpuLoad => (self.snap.gpu_load, "%"),
            Metric::GpuClock => (self.snap.gpu_clock, "MHz"),
            Metric::GpuPower => (self.snap.gpu_power, "W"),
        }
    }
}

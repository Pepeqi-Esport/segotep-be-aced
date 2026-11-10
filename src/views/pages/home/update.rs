//! GitHub Releases check / apply UI for [`HomePage`].

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use eframe::egui::{self, Align2, Color32, RichText, Sense, Vec2};

use crate::helpers::update_helper::{self, UpdateInfo, UpdatePrefs};
use crate::views::pages::HomePage;

pub(crate) enum UpdateMsg {
    Checked {
        info: UpdateInfo,
        /// When true, always show a modal (update or up-to-date).
        force_prompt: bool,
    },
    CheckFailed(String),
    ApplyFailed(String),
}

#[derive(Debug, Clone)]
pub(crate) enum UpdateDialog {
    Available(UpdateInfo),
    UpToDate { current: String },
}

impl HomePage {
    pub(crate) fn ensure_startup_update_check(&mut self, ctx: &egui::Context) {
        if self.update_startup_started || self.tray_hidden || self.start_in_tray {
            return;
        }
        self.update_startup_started = true;
        self.spawn_update_check(false, ctx);
    }

    pub(crate) fn request_manual_update_check(&mut self, ctx: &egui::Context) {
        if self.update_busy {
            return;
        }
        self.status = "Checking for updates…".into();
        self.spawn_update_check(true, ctx);
    }

    fn spawn_update_check(&mut self, force_prompt: bool, ctx: &egui::Context) {
        self.update_busy = true;
        let tx = self.update_tx.clone();
        let ctx = ctx.clone();
        thread::spawn(move || {
            let msg = match update_helper::check_latest() {
                Ok(info) => UpdateMsg::Checked { info, force_prompt },
                Err(err) => UpdateMsg::CheckFailed(err.to_string()),
            };
            let _ = tx.send(msg);
            ctx.request_repaint();
        });
    }

    pub(crate) fn poll_update_messages(&mut self) {
        while let Ok(msg) = self.update_rx.try_recv() {
            self.update_busy = false;
            match msg {
                UpdateMsg::Checked { info, force_prompt } => {
                    if info.newer {
                        let show = if force_prompt {
                            true
                        } else {
                            update_helper::should_prompt_on_startup(&info, &self.update_prefs)
                        };
                        if show {
                            self.status = format!(
                                "Update available: v{} → v{}",
                                info.current, info.remote
                            );
                            self.update_dialog = Some(UpdateDialog::Available(info));
                        } else {
                            self.status = format!("Update v{} dismissed", info.remote);
                        }
                    } else if force_prompt {
                        self.status = format!("Up to date (v{})", info.current);
                        self.update_dialog = Some(UpdateDialog::UpToDate {
                            current: info.current,
                        });
                    }
                }
                UpdateMsg::CheckFailed(err) => {
                    self.status = format!("Update check failed: {err}");
                    tracing::warn!(error = %err, "update check failed");
                }
                UpdateMsg::ApplyFailed(err) => {
                    self.status = format!("Update failed: {err}");
                    tracing::error!(error = %err, "update apply failed");
                    self.reopen_hid();
                }
            }
        }
    }

    pub(crate) fn dismiss_update_available(&mut self) {
        if let Some(UpdateDialog::Available(info)) = self.update_dialog.take() {
            if let Err(err) = self.update_prefs.dismiss(&info.remote) {
                self.status = format!("Could not save Later preference: {err}");
            } else {
                self.status = format!("Remind me later for v{}", info.remote);
            }
        } else {
            self.update_dialog = None;
        }
    }

    pub(crate) fn start_apply_update(&mut self, ctx: &egui::Context) {
        if self.update_busy {
            return;
        }
        self.update_dialog = None;
        self.update_busy = true;
        self.status = "Downloading update…".into();

        // Release HID / sensors before the background thread kills bridges + swaps the exe.
        self.device = None;
        self.sensors.stop_bridge();

        let tx = self.update_tx.clone();
        let ctx = ctx.clone();
        thread::spawn(move || {
            match update_helper::apply_update() {
                Ok(()) => {
                    // apply_update relaunches and exits on success.
                }
                Err(err) => {
                    let _ = tx.send(UpdateMsg::ApplyFailed(err.to_string()));
                    ctx.request_repaint();
                }
            }
        });
    }

    pub(crate) fn show_update_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.update_dialog.clone() else {
            return;
        };
        let p = self.palette();

        match dialog {
            UpdateDialog::Available(info) => {
                let mut open = true;
                let mut update_clicked = false;
                let mut later_clicked = false;

                egui::Window::new("Update available")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                    .open(&mut open)
                    .show(ctx, |ui| {
                        ui.set_min_width(300.0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new(format!("v{} → v{}", info.current, info.remote))
                                    .size(16.0)
                                    .strong()
                                    .color(p.text),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new("A newer BeAced release is available on GitHub.")
                                    .size(13.0)
                                    .color(p.muted),
                            );
                            ui.add_space(16.0);
                            // Later (left) · Update (right), centered as a pair.
                            ui.horizontal(|ui| {
                                let total = ui.available_width();
                                let pair_w = 168.0_f32;
                                ui.add_space(((total - pair_w) * 0.5).max(0.0));
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("Later").size(13.0).color(p.text),
                                        )
                                        .min_size(Vec2::new(76.0, 0.0))
                                        .fill(Color32::TRANSPARENT)
                                        .stroke(egui::Stroke::new(1.0_f32, p.border)),
                                    )
                                    .clicked()
                                {
                                    later_clicked = true;
                                }
                                ui.add_space(8.0);
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("Update")
                                                .size(13.0)
                                                .color(Color32::WHITE),
                                        )
                                        .min_size(Vec2::new(76.0, 0.0))
                                        .fill(p.accent)
                                        .sense(Sense::click()),
                                    )
                                    .clicked()
                                {
                                    update_clicked = true;
                                }
                            });
                        });
                    });

                if !open || later_clicked {
                    self.dismiss_update_available();
                } else if update_clicked {
                    self.start_apply_update(ctx);
                }
            }
            UpdateDialog::UpToDate { current } => {
                let mut open = true;
                let mut ok_clicked = false;

                egui::Window::new("You're up to date")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                    .open(&mut open)
                    .show(ctx, |ui| {
                        ui.set_min_width(280.0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "BeAced v{current} is the latest version."
                                ))
                                .size(14.0)
                                .color(p.text),
                            );
                            ui.add_space(16.0);
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("OK").size(13.0).color(Color32::WHITE),
                                    )
                                    .fill(p.accent),
                                )
                                .clicked()
                            {
                                ok_clicked = true;
                            }
                        });
                    });

                if !open || ok_clicked {
                    self.update_dialog = None;
                }
            }
        }
    }
}

pub(crate) fn update_channel() -> (Sender<UpdateMsg>, Receiver<UpdateMsg>) {
    mpsc::channel()
}

pub(crate) fn load_prefs() -> UpdatePrefs {
    UpdatePrefs::load()
}

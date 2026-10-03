//! Native update notification and explicit verified-download/install sheet.
use super::{
    Action,
    helpers::{ButtonKind, Group, SheetPlacement, button, caption, padded, sheet, sheet_header},
};
use crate::{
    runtime::updates::{UpdateStatus, Updates},
    theme::{Palette, metrics},
};
use eframe::egui::{self, Align2, Frame, Id, Margin, Stroke, vec2};

fn status(updates: &Updates) -> String {
    match &updates.status {
        UpdateStatus::Idle => format!("Neptune {}", env!("CARGO_PKG_VERSION")),
        UpdateStatus::Checking => "Checking for updates…".into(),
        UpdateStatus::Current => "You're up to date.".into(),
        UpdateStatus::Available => updates
            .release
            .as_ref()
            .map(|release| format!("Neptune {} is available.", release.version))
            .unwrap_or_default(),
        UpdateStatus::Downloading => "Downloading and verifying…".into(),
        UpdateStatus::Ready => "Download verified. Ready to open.".into(),
        UpdateStatus::Opening => "Opening your download…".into(),
        UpdateStatus::Opened => {
            "Download opened. Finish installation, then relaunch Neptune.".into()
        }
        UpdateStatus::Error(message) => message.clone(),
    }
}

/// The installed version with a manual check, the release on offer and the
/// state of the last check, as rows of one Preferences card.
pub fn preferences_status(
    ui: &mut egui::Ui,
    p: Palette,
    updates: &Updates,
    rows: &mut Group,
    actions: &mut Vec<Action>,
) {
    rows.row(
        ui,
        &format!("Neptune {}", env!("CARGO_PKG_VERSION")),
        |ui| {
            ui.add_enabled_ui(!updates.busy(), |ui| {
                if button(ui, p, "Check for updates", ButtonKind::Secondary).clicked() {
                    actions.push(Action::CheckUpdates);
                }
            });
        },
    );
    if let Some(release) = &updates.release {
        rows.row(ui, &format!("Version {}", release.version), |ui| {
            if button(ui, p, "Review update", ButtonKind::Secondary).clicked() {
                actions.push(Action::ReviewUpdate);
            }
        });
    }
    // An idle updater has nothing to add to the installed version.
    if !matches!(updates.status, UpdateStatus::Idle) {
        rows.note(ui, &status(updates));
    }
}

pub fn notification(ctx: &egui::Context, p: Palette, updates: &Updates, actions: &mut Vec<Action>) {
    let Some(release) = updates.notification() else {
        return;
    };
    egui::Area::new(Id::new("neptune-update-notification"))
        .order(egui::Order::Foreground)
        .anchor(
            Align2::RIGHT_TOP,
            vec2(-14.0, metrics::TOOLBAR_HEIGHT + 8.0),
        )
        .show(ctx, |ui| {
            Frame::new()
                .fill(p.elevated)
                .corner_radius(12)
                .stroke(Stroke::new(1.0, p.border))
                .shadow(p.popup_shadow())
                .inner_margin(Margin::same(12))
                .show(ui, |ui| {
                    ui.set_max_width((ctx.content_rect().width() - 52.0).clamp(160.0, 340.0));
                    ui.label(
                        egui::RichText::new(format!("Neptune {} is available", release.version))
                            .color(p.fg)
                            .size(13.0),
                    );
                    ui.horizontal_wrapped(|ui| {
                        if button(ui, p, "What's New", ButtonKind::Secondary).clicked() {
                            actions.push(Action::ReviewUpdate);
                        }
                        if button(ui, p, "Later", ButtonKind::Quiet).clicked() {
                            actions.push(Action::DismissUpdate);
                        }
                    });
                });
        });
}

pub fn show(ctx: &egui::Context, p: Palette, updates: &Updates, actions: &mut Vec<Action>) {
    let mut close = false;
    let output = sheet(
        ctx,
        p,
        "Neptune update",
        500.0,
        SheetPlacement::Center,
        |ui| {
            close |= sheet_header(ui, p, "Neptune update", Some("Close update"));
            egui::ScrollArea::vertical()
                .id_salt("update-body")
                .max_height((ctx.content_rect().height() - 190.0).clamp(120.0, 440.0))
                .show(ui, |ui| {
                    padded(ui, 20.0, |ui| {
                        if let Some(release) = &updates.release {
                            ui.label(
                                egui::RichText::new(format!("Neptune {}", release.version))
                                    .size(20.0)
                                    .color(p.fg),
                            );
                            caption(ui, p, &format!("Installed: {}", env!("CARGO_PKG_VERSION")));
                            ui.add_space(10.0);
                            // Plain text styling keeps release notes independent
                            // of external images or executable rich content.
                            for line in release.notes.lines() {
                                if line.trim().is_empty() {
                                    ui.add_space(6.0);
                                } else if let Some(heading) = line.strip_prefix("### ") {
                                    ui.label(egui::RichText::new(heading).strong().color(p.fg));
                                } else {
                                    let text = line.strip_prefix("- ")
                                        .map(|text| format!("• {text}"))
                                        .unwrap_or_else(|| line.to_owned());
                                    ui.add(egui::Label::new(text).wrap());
                                }
                            }
                            ui.add_space(12.0);
                            if button(ui, p, "View GitHub release", ButtonKind::Quiet).clicked()
                                && let Some(link) = crate::platform::links::WebLink::new(&release.release_url())
                            {
                                actions.push(Action::OpenLink(link));
                            }
                            let instructions = if cfg!(target_os = "macos") {
                                "Open the verified disk image, quit Neptune when you're ready, then drag Neptune into Applications."
                            } else if cfg!(windows) {
                                "Open the verified installer and follow its steps. Save your shell work before closing Neptune. Windows signing is not available yet."
                            } else {
                                "Show the verified download in your file manager. For AppImage, make it executable and replace your old file after quitting. For DEB, install with your package manager."
                            };
                            caption(ui, p, instructions);
                        }
                    });
                });
            padded(ui, 20.0, |ui| {
                // Keep failures and progress visible even when notes scroll.
                caption(ui, p, &status(updates));
                ui.horizontal_wrapped(|ui| {
                    if let Some(release) = &updates.release {
                        ui.add_enabled_ui(!updates.busy(), |ui| {
                            if matches!(updates.status, UpdateStatus::Ready | UpdateStatus::Opened)
                            {
                                if button(
                                    ui,
                                    p,
                                    if cfg!(target_os = "linux") {
                                        "Show download"
                                    } else {
                                        "Open installer"
                                    },
                                    ButtonKind::Primary,
                                )
                                .clicked()
                                {
                                    actions.push(Action::OpenUpdate(release.version.clone()));
                                }
                            } else if button(ui, p, "Download update", ButtonKind::Primary)
                                .clicked()
                            {
                                actions.push(Action::DownloadUpdate(release.version.clone()));
                            }
                        });
                    }
                    if updates.busy() && button(ui, p, "Cancel", ButtonKind::Secondary).clicked() {
                        actions.push(Action::CancelUpdate);
                    }
                    close |= button(ui, p, "Later", ButtonKind::Quiet).clicked();
                });
            });
        },
    );
    if close || output.backdrop_clicked {
        actions.push(Action::DismissUpdate);
        actions.push(Action::CloseOverlay);
    }
}

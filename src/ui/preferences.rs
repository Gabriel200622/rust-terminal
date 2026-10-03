//! Preferences: grouped settings that apply as they change.
use super::Action;
use super::helpers::{
    ButtonKind, Group, SheetPlacement, animate, button, caption, group, menu_item, menu_layout,
    menu_separator, padded, place, section_label, segmented, select, sheet, sheet_footer,
    sheet_header, slider, stepper, text_field, toggle,
};
use super::theme_browser;
use crate::{
    config::{Config, Cursor},
    icons::Icon,
    platform::shells::{Detection, Shell},
    theme::{self, Palette},
};
use eframe::egui::{self, Align, Align2, Id, Layout, WidgetInfo, WidgetType, vec2};

pub fn show(
    ctx: &egui::Context,
    current: &Config,
    updates: &crate::runtime::updates::Updates,
    state: &mut theme_browser::State,
    shells: &mut ShellPicker,
    actions: &mut Vec<Action>,
) {
    let mut config = current.clone();
    let p = Palette::for_config(&config);
    let screen = ctx.content_rect();
    let mut close = false;
    // The catalog needs room for its grid: the sheet widens as it appears.
    let themes = animate(ctx, Id::new("preferences-themes"), state.open, 0.16);
    let width = egui::lerp(500.0..=theme_browser::WIDTH, themes);
    let output = sheet(ctx, p, "Preferences", width, SheetPlacement::Center, |ui| {
        // Leave the window visible around the sheet when there is room; a
        // short window gives most of that margin back to the settings.
        let chrome = 52.0 + 60.0;
        let body_height = (screen.height() - chrome - 96.0)
            .max((screen.height() - chrome - 24.0).min(260.0))
            .clamp(120.0, 560.0);
        if state.open {
            close |= theme_browser::show(ui, p, &mut config, state, body_height);
            return;
        }
        close |= sheet_header(ui, p, "Preferences", Some("Close preferences"));
        egui::ScrollArea::vertical()
            .id_salt("preferences-body")
            .max_height(body_height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                padded(ui, 20.0, |ui| {
                    section_label(ui, p, "Appearance");
                    group(ui, p, |ui, rows| {
                        if theme_browser::summary(ui, p, &config, rows) {
                            state.browse();
                        }
                        rows.row(ui, "Window zoom", |ui| {
                            let text = format!("{:.0}%", config.window_zoom * 100.0);
                            if stepper(
                                ui,
                                p,
                                "Window zoom",
                                &mut config.window_zoom,
                                Config::WINDOW_ZOOM_RANGE,
                                0.1,
                                &text,
                            ) {
                                config.window_zoom = (config.window_zoom * 100.0).round() / 100.0;
                            }
                        });
                    });
                    caption(
                        ui,
                        p,
                        if cfg!(target_os = "macos") {
                            "Window zoom: Command+Plus / Minus. Reset: Command+0. Saved for next launch."
                        } else {
                            "Window zoom: Ctrl+Plus / Minus. Reset: Ctrl+0. Saved for next launch."
                        },
                    );

                    ui.add_space(14.0);
                    section_label(ui, p, "Text");
                    group(ui, p, |ui, rows| {
                        rows.row(ui, "Font size", |ui| {
                            let text = format!("{} pt", config.font_size.round() as i32);
                            stepper(
                                ui,
                                p,
                                "Font size",
                                &mut config.font_size,
                                9.0..=32.0,
                                1.0,
                                &text,
                            );
                        });
                        rows.row(ui, "Line spacing", |ui| {
                            ui.spacing_mut().item_spacing.x = 12.0;
                            let (_, value) = ui.allocate_space(vec2(34.0, 20.0));
                            ui.painter().text(
                                value.right_center(),
                                Align2::RIGHT_CENTER,
                                format!("{:.2}", config.line_height),
                                theme::medium(12.5),
                                p.secondary,
                            );
                            slider(
                                ui,
                                p,
                                "Line spacing",
                                &mut config.line_height,
                                1.0..=2.0,
                                0.05,
                                150.0,
                            );
                        });
                    });
                    caption(
                        ui,
                        p,
                        if cfg!(target_os = "macos") {
                            "Font size: Command+Shift+Plus / Minus. Reset: Command+Shift+0."
                        } else {
                            "Font size: Ctrl+Shift+Plus / Minus. Reset: Ctrl+Shift+0."
                        },
                    );

                    ui.add_space(14.0);
                    section_label(ui, p, "Cursor");
                    group(ui, p, |ui, rows| {
                        rows.row(ui, "Style", |ui| {
                            segmented(
                                ui,
                                p,
                                "cursor-style",
                                &mut config.cursor,
                                &[
                                    (Cursor::Block, "Block"),
                                    (Cursor::Beam, "Beam"),
                                    (Cursor::Underline, "Underline"),
                                ],
                                228.0,
                            );
                        });
                        rows.row(ui, "Blink", |ui| {
                            toggle(ui, p, &mut config.cursor_blink, "Blink cursor");
                        });
                    });

                    ui.add_space(14.0);
                    section_label(ui, p, "Shell");
                    group(ui, p, |ui, rows| {
                        shell_rows(ui, p, &mut config, shells, rows);
                        rows.row(ui, "Scrollback", |ui| {
                            ui.add(
                                egui::DragValue::new(&mut config.scrollback)
                                    .range(0..=1_000_000)
                                    .speed(100)
                                    .suffix(" lines"),
                            )
                            .widget_info(|| {
                                WidgetInfo::labeled(WidgetType::DragValue, true, "Scrollback lines")
                            });
                        });
                    });
                    caption(
                        ui,
                        p,
                        "Shell and scrollback changes apply to new terminals.",
                    );

                    ui.add_space(14.0);
                    section_label(ui, p, "Sessions");
                    group(ui, p, |ui, rows| {
                        rows.row(ui, "Restore workspaces on launch", |ui| {
                            toggle(
                                ui,
                                p,
                                &mut config.restore_workspaces,
                                "Restore workspaces on launch",
                            );
                        });
                        rows.row(ui, "Confirm before closing terminals", |ui| {
                            toggle(
                                ui,
                                p,
                                &mut config.confirm_close,
                                "Confirm before closing terminals",
                            );
                        });
                        rows.row(ui, "Warn about running processes", |ui| {
                            toggle(
                                ui,
                                p,
                                &mut config.warn_running_processes,
                                "Warn about running processes",
                            );
                        });
                    });
                    caption(
                        ui,
                        p,
                        "Restoring reopens folders and layouts with fresh shells. Process warnings also apply when quitting, even with close confirmation off.",
                    );
                    ui.add_space(14.0);
                    section_label(ui, p, "Notifications");
                    group(ui, p, |ui, rows| {
                        rows.row(ui, "Desktop banners", |ui| {
                            toggle(
                                ui,
                                p,
                                &mut config.desktop_notifications,
                                "Desktop banners",
                            );
                        });
                    });
                    caption(
                        ui,
                        p,
                        "Show a system banner when a terminal asks for attention. Pane rings, unread counts and history stay on.",
                    );
                    ui.add_space(14.0);
                    section_label(ui, p, "Updates");
                    group(ui, p, |ui, rows| {
                        rows.row(ui, "Check automatically", |ui| {
                            toggle(ui, p, &mut config.check_updates, "Check for updates automatically");
                        });
                        rows.row(ui, "Release channel", |ui| {
                            use crate::runtime::updates::ReleaseChannel;
                            segmented(ui, p, "release-channel", &mut config.release_channel, &[(ReleaseChannel::Stable, "Stable"), (ReleaseChannel::Beta, "Beta")], 150.0);
                        });
                    });
                    caption(ui, p, "Beta includes previews and newer stable releases. Updates are installed only when you choose.");
                    crate::ui::updates::preferences_status(ui, p, updates, actions);
                    ui.add_space(18.0);
                });
            });

        let bar = sheet_footer(ui, p);
        place(
            ui,
            bar,
            Layout::left_to_right(Align::Center),
            "preferences-reset",
            |ui| {
                if button(ui, p, "Reset to defaults", ButtonKind::Quiet).clicked() {
                    config = Config {
                        custom_themes: config.custom_themes.clone(),
                        favorite_themes: config.favorite_themes.clone(),
                        ..Config::default()
                    };
                }
            },
        );
        place(
            ui,
            bar,
            Layout::right_to_left(Align::Center),
            "preferences-done",
            |ui| {
                close |= button(ui, p, "Done", ButtonKind::Primary).clicked();
            },
        );
    });
    if (close || output.backdrop_clicked) && state.may_close() {
        actions.push(Action::CloseOverlay);
    }
    if &config != current {
        actions.push(Action::Preferences(config));
    }
}

/// The shells found on this computer, and whether the user chose to enter a
/// program. Reset each time Preferences opens, so new installs appear.
#[derive(Default)]
pub struct ShellPicker {
    detection: Detection,
    custom: bool,
}

/// A menu of detected shells, as Windows Terminal offers profiles, with a
/// program field for anything else. Only the program and its arguments are
/// saved, so a shell that is later uninstalled shows as a custom program.
fn shell_rows(
    ui: &mut egui::Ui,
    p: Palette,
    config: &mut Config,
    picker: &mut ShellPicker,
    rows: &mut Group,
) {
    let width = (ui.available_width() - 110.0).clamp(120.0, 250.0);
    let detected = picker.detection.poll(ui.ctx());
    let shells = detected.unwrap_or_default();
    let default = match shells.iter().find(|shell| shell.default) {
        Some(shell) => format!("System default ({})", shell.name),
        None => "System default".into(),
    };
    let chosen: Option<&Shell> = config.shell.as_ref().and_then(|program| {
        shells
            .iter()
            .find(|shell| shell.runs(program, &config.shell_args))
    });
    let custom =
        picker.custom || (detected.is_some() && config.shell.is_some() && chosen.is_none());
    let value = match (&config.shell, chosen) {
        _ if custom => "Custom".to_owned(),
        (None, _) => default.clone(),
        (Some(_), Some(shell)) => shell.name.clone(),
        // Still detecting: name the program rather than guess its profile.
        (Some(program), None) => std::path::Path::new(program).file_stem().map_or_else(
            || program.clone(),
            |name| name.to_string_lossy().into_owned(),
        ),
    };
    // Choosing Custom… brings its field into view, ready for typing.
    let mut reveal = false;
    rows.row(ui, "Program", |ui| {
        let response = select(
            ui,
            p,
            Id::new("preferences-shell-menu"),
            &value,
            "Shell",
            width,
        );
        let menu = egui::Popup::menu(&response).show(|ui| {
            menu_layout(ui, 300.0);
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .show(ui, |ui| {
                    let mark = |selected: bool| {
                        if selected {
                            Icon::Check
                        } else {
                            Icon::Terminal
                        }
                    };
                    if menu_item(
                        ui,
                        p,
                        mark(!custom && config.shell.is_none()),
                        &default,
                        "",
                        false,
                    ) {
                        config.shell = None;
                        config.shell_args.clear();
                        picker.custom = false;
                    }
                    for shell in shells.iter().filter(|shell| !shell.default) {
                        let selected = !custom && chosen == Some(shell);
                        if menu_item(ui, p, mark(selected), &shell.name, "", false) {
                            config.shell = Some(shell.program.clone());
                            config.shell_args = shell.args.clone();
                            picker.custom = false;
                        }
                    }
                    if detected.is_none() {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new("   Finding shells…")
                                .font(theme::regular(12.5))
                                .color(p.muted),
                        );
                        ui.add_space(4.0);
                    }
                    menu_separator(ui, p);
                    let icon = if custom { Icon::Check } else { Icon::Pencil };
                    if menu_item(ui, p, icon, "Custom…", "", false) {
                        // The program stays for editing; a profile's arguments do not.
                        config.shell_args.clear();
                        picker.custom = true;
                        reveal = true;
                    }
                });
        });
        // Both the sheet and the menu are foreground layers; a reopened sheet
        // can otherwise stay above a menu that was shown before.
        if let Some(menu) = menu {
            ui.ctx().move_to_top(menu.response.layer_id);
        }
    });
    if custom || reveal {
        rows.row(ui, "Path", |ui| {
            let mut shell = config.shell.clone().unwrap_or_default();
            let response = text_field(
                ui,
                p,
                Id::new("preferences-shell"),
                &mut shell,
                "Program path",
                "Shell program",
                width,
            );
            if reveal {
                response.request_focus();
            }
            if reveal || response.gained_focus() {
                // Leave room for the focus ring and the row below the field.
                ui.scroll_to_rect(response.rect.expand(14.0), None);
            }
            if response.changed() {
                config.shell = (!shell.trim().is_empty()).then_some(shell);
                if config.shell.is_none() {
                    config.shell_args.clear();
                }
            }
        });
    }
}

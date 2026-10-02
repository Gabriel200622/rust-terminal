//! Preferences: grouped settings that apply as they change.
use super::Action;
use super::helpers::{
    ButtonKind, SheetPlacement, button, caption, group, padded, place, section_label, segmented,
    sheet, sheet_header, slider, stepper, text_field, toggle,
};
use crate::{
    config::{Accent, Config, Cursor, Theme},
    theme::{self, Palette},
};
use eframe::egui::{
    self, Align, Align2, Color32, CursorIcon, Id, Layout, Pos2, Rect, Sense, Stroke, StrokeKind,
    Ui, Vec2, WidgetInfo, WidgetType, vec2,
};

/// A miniature of the window in a theme's own materials.
fn theme_tile(
    ui: &mut Ui,
    p: Palette,
    theme: Theme,
    accent: Accent,
    name: &str,
    selected: bool,
    width: f32,
) -> bool {
    let (_, rect) = ui.allocate_space(vec2(width, 100.0));
    let response = ui.interact(rect, ui.id().with(("theme", name)), Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, true, selected, name));
    let preview = Rect::from_min_size(rect.min + vec2(3.0, 3.0), vec2(width - 6.0, 70.0));
    let look = Palette::with_accent(theme, accent);
    let painter = ui.painter();
    painter.rect_filled(preview, 8, look.chrome);
    let side = preview.width() * 0.27;
    for (index, light) in [0xff5f57, 0xfebc2e, 0x28c840].into_iter().enumerate() {
        painter.circle_filled(
            preview.min + vec2(8.0 + index as f32 * 6.0, 8.0),
            2.0,
            theme::color(light),
        );
    }
    for (index, tone) in [look.pressed, look.hover, look.hover]
        .into_iter()
        .enumerate()
    {
        painter.rect_filled(
            Rect::from_min_size(
                preview.min + vec2(6.0, 20.0 + index as f32 * 11.0),
                vec2(side - 10.0, 7.0),
            ),
            3,
            tone,
        );
    }
    let stage = Rect::from_min_max(preview.min + vec2(side, 6.0), preview.max - vec2(5.0, 5.0));
    painter.rect_filled(stage, 5, look.bg);
    for (index, (length, tone)) in [
        (0.42, look.accent),
        (0.74, look.fg),
        (0.56, look.ansi[2]),
        (0.66, look.secondary),
        (0.3, look.ansi[5]),
    ]
    .into_iter()
    .enumerate()
    {
        painter.rect_filled(
            Rect::from_min_size(
                stage.min + vec2(7.0, 8.0 + index as f32 * 9.0),
                vec2((stage.width() - 14.0) * length, 3.5),
            ),
            2,
            theme::tint(tone, 0.85),
        );
    }
    if selected {
        painter.rect_stroke(
            preview.expand(3.0),
            11,
            Stroke::new(2.0, p.accent),
            StrokeKind::Inside,
        );
    } else {
        painter.rect_stroke(
            preview,
            8,
            Stroke::new(
                1.0,
                if response.hovered() {
                    p.muted
                } else {
                    p.border
                },
            ),
            StrokeKind::Inside,
        );
    }
    if response.has_focus() {
        painter.rect_stroke(
            preview.expand(5.0),
            13,
            Stroke::new(1.5, theme::tint(p.accent, 0.5)),
            StrokeKind::Inside,
        );
    }
    painter.text(
        Pos2::new(rect.center().x, rect.bottom() - 11.0),
        Align2::CENTER_CENTER,
        name,
        theme::medium(12.0),
        if selected { p.fg } else { p.secondary },
    );
    response.on_hover_cursor(CursorIcon::PointingHand).clicked() && !selected
}

fn accent_dots(ui: &mut Ui, p: Palette, current: &mut Accent) {
    ui.spacing_mut().item_spacing.x = 7.0;
    // The row lays out from its trailing edge; reverse to read Blue first.
    for (accent, name) in Accent::ALL.into_iter().rev() {
        let (_, rect) = ui.allocate_space(Vec2::splat(20.0));
        let label = format!("{name} accent");
        let response = ui.interact(rect, ui.id().with(("accent", name)), Sense::click());
        let selected = accent == *current;
        response
            .widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, true, selected, &label));
        let color = theme::accent_color(accent, p.dark);
        let painter = ui.painter();
        painter.circle_filled(
            rect.center(),
            if response.hovered() { 8.5 } else { 8.0 },
            color,
        );
        painter.circle_stroke(
            rect.center(),
            8.0,
            Stroke::new(0.5, Color32::from_black_alpha(40)),
        );
        if selected {
            painter.circle_filled(rect.center(), 3.0, Color32::WHITE);
        }
        if response.has_focus() {
            painter.circle_stroke(
                rect.center(),
                10.5,
                Stroke::new(1.5, theme::tint(color, 0.7)),
            );
        }
        if response
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(name)
            .clicked()
        {
            *current = accent;
        }
    }
}

pub fn show(
    ctx: &egui::Context,
    current: &Config,
    updates: &crate::runtime::updates::Updates,
    actions: &mut Vec<Action>,
) {
    let mut config = current.clone();
    let p = Palette::for_config(&config);
    let before = toml::to_string(&config).unwrap_or_default();
    let screen = ctx.content_rect();
    let mut close = false;
    let output = sheet(ctx, p, "Preferences", 500.0, SheetPlacement::Center, |ui| {
        close |= sheet_header(ui, p, "Preferences", Some("Close preferences"));
        // Leave the window visible around the sheet when there is room.
        let body_height = (screen.height() - 52.0 - 60.0 - 96.0).clamp(120.0, 560.0);
        egui::ScrollArea::vertical()
            .id_salt("preferences-body")
            .max_height(body_height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                padded(ui, 20.0, |ui| {
                    section_label(ui, p, "Appearance");
                    ui.add_space(2.0);
                    let gap = 10.0;
                    let tile = ((ui.available_width() - gap * 2.0) / 3.0).floor();
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;
                        for (theme, name) in [
                            (Theme::Graphite, "Graphite"),
                            (Theme::Dusk, "Dusk"),
                            (Theme::Light, "Light"),
                        ] {
                            if theme_tile(
                                ui,
                                p,
                                theme,
                                config.accent,
                                name,
                                config.theme == theme,
                                tile,
                            ) {
                                config.theme = theme;
                            }
                        }
                    });
                    ui.add_space(8.0);
                    group(ui, p, |ui, rows| {
                        rows.row(ui, "Accent", |ui| accent_dots(ui, p, &mut config.accent));
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
                        rows.row(ui, "Program", |ui| {
                            let mut shell = config.shell.clone().unwrap_or_default();
                            let width = (ui.available_width() - 110.0).clamp(120.0, 250.0);
                            if text_field(
                                ui,
                                p,
                                Id::new("preferences-shell"),
                                &mut shell,
                                "System default",
                                "Shell program",
                                width,
                            )
                            .changed()
                            {
                                config.shell = (!shell.trim().is_empty()).then_some(shell);
                            }
                        });
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

        let (_, footer) = ui.allocate_space(vec2(ui.available_width(), 60.0));
        ui.painter()
            .line_segment([footer.left_top(), footer.right_top()], p.hairline());
        let bar = footer.shrink2(vec2(16.0, 0.0));
        place(
            ui,
            bar,
            Layout::left_to_right(Align::Center),
            "preferences-reset",
            |ui| {
                if button(ui, p, "Reset to defaults", ButtonKind::Quiet).clicked() {
                    config = Config::default();
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
    if close || output.backdrop_clicked {
        actions.push(Action::CloseOverlay);
    }
    if toml::to_string(&config).unwrap_or_default() != before {
        actions.push(Action::Preferences(config));
    }
}

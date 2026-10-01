//! Small, consistent vector icons. Coordinates use a 24-point design grid.

use eframe::egui::{
    Color32, CursorIcon, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui,
    WidgetInfo, WidgetType, vec2,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Terminal,
    Plus,
    Close,
    ChevronRight,
    ChevronDown,
    Settings,
    Search,
    Sidebar,
    SplitVertical,
    SplitHorizontal,
    Folder,
    Grid,
    Keyboard,
    Check,
    ArrowUpRight,
    ArrowUp,
    ArrowDown,
    Minus,
    Maximize,
    Command,
    Sun,
    Moon,
    Copy,
}

/// Paint an icon into its visual bounds. The caller controls the hit area.
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let side = rect.width().min(rect.height());
    let origin = rect.center() - vec2(side, side) * 0.5;
    let scale = side / 24.0;
    let point = |x: f32, y: f32| origin + vec2(x, y) * scale;
    let stroke = Stroke::new(1.5, color);
    let line = |points: &[[f32; 2]]| {
        let points: Vec<Pos2> = points.iter().map(|p| point(p[0], p[1])).collect();
        let first = points.first().copied();
        let last = points.last().copied();
        painter.add(Shape::line(points, stroke));
        // egui's line primitives have square caps. These tiny circles give the
        // outline set the same quiet, rounded ends as the desktop reference.
        if let Some(first) = first {
            painter.circle_filled(first, stroke.width * 0.5, color);
        }
        if let Some(last) = last {
            painter.circle_filled(last, stroke.width * 0.5, color);
        }
    };
    let rectangle = |x: f32, y: f32, width: f32, height: f32, radius: f32| {
        painter.rect_stroke(
            Rect::from_min_max(point(x, y), point(x + width, y + height)),
            (radius * scale).round() as u8,
            stroke,
            StrokeKind::Middle,
        );
    };
    let circle = |x: f32, y: f32, radius: f32| {
        painter.circle_stroke(point(x, y), radius * scale, stroke);
    };

    match icon {
        Icon::Terminal => {
            rectangle(3.0, 5.0, 18.0, 14.0, 3.0);
            line(&[[7.0, 9.0], [10.0, 12.0], [7.0, 15.0]]);
            line(&[[13.0, 15.0], [17.0, 15.0]]);
        }
        Icon::Plus => {
            line(&[[12.0, 5.0], [12.0, 19.0]]);
            line(&[[5.0, 12.0], [19.0, 12.0]]);
        }
        Icon::Close => {
            line(&[[6.0, 6.0], [18.0, 18.0]]);
            line(&[[18.0, 6.0], [6.0, 18.0]]);
        }
        Icon::ChevronRight => line(&[[9.0, 5.0], [16.0, 12.0], [9.0, 19.0]]),
        Icon::ChevronDown => line(&[[5.0, 9.0], [12.0, 16.0], [19.0, 9.0]]),
        Icon::Settings => {
            let mut outline = Vec::with_capacity(33);
            for tooth in 0..8 {
                for (offset, radius) in [(0.0, 7.5), (0.2, 9.5), (0.55, 9.5), (0.75, 7.5)] {
                    let angle = (tooth as f32 + offset) * std::f32::consts::TAU / 8.0;
                    outline.push([12.0 + radius * angle.cos(), 12.0 + radius * angle.sin()]);
                }
            }
            outline.push(outline[0]);
            line(&outline);
            circle(12.0, 12.0, 3.0);
        }
        Icon::Search => {
            circle(10.5, 10.5, 6.5);
            line(&[[15.3, 15.3], [20.0, 20.0]]);
        }
        Icon::Sidebar => {
            rectangle(3.0, 4.0, 18.0, 16.0, 3.0);
            line(&[[8.5, 4.5], [8.5, 19.5]]);
        }
        Icon::SplitVertical => {
            rectangle(3.0, 4.0, 18.0, 16.0, 3.0);
            line(&[[12.0, 4.5], [12.0, 19.5]]);
        }
        Icon::SplitHorizontal => {
            rectangle(3.0, 4.0, 18.0, 16.0, 3.0);
            line(&[[3.5, 12.0], [20.5, 12.0]]);
        }
        Icon::Folder => {
            line(&[
                [3.0, 7.0],
                [3.0, 18.0],
                [4.0, 19.0],
                [20.0, 19.0],
                [21.0, 18.0],
                [21.0, 9.0],
                [20.0, 8.0],
                [12.0, 8.0],
                [10.0, 5.0],
                [4.0, 5.0],
                [3.0, 6.0],
                [3.0, 7.0],
            ]);
        }
        Icon::Grid => {
            for y in [4.0, 14.0] {
                for x in [4.0, 14.0] {
                    rectangle(x, y, 6.0, 6.0, 1.5);
                }
            }
        }
        Icon::Keyboard => {
            rectangle(2.0, 6.0, 20.0, 13.0, 3.0);
            for y in [10.0, 13.0] {
                for x in [6.0, 10.0, 14.0, 18.0] {
                    painter.circle_filled(point(x, y), 0.8 * scale, color);
                }
            }
            line(&[[8.0, 16.0], [16.0, 16.0]]);
        }
        Icon::Check => line(&[[5.0, 12.0], [10.0, 17.0], [20.0, 7.0]]),
        Icon::ArrowUpRight => {
            line(&[[6.0, 18.0], [18.0, 6.0]]);
            line(&[[7.0, 6.0], [18.0, 6.0], [18.0, 17.0]]);
        }
        Icon::ArrowUp => {
            line(&[[12.0, 19.0], [12.0, 5.0]]);
            line(&[[6.0, 11.0], [12.0, 5.0], [18.0, 11.0]]);
        }
        Icon::ArrowDown => {
            line(&[[12.0, 5.0], [12.0, 19.0]]);
            line(&[[6.0, 13.0], [12.0, 19.0], [18.0, 13.0]]);
        }
        Icon::Minus => line(&[[5.0, 12.0], [19.0, 12.0]]),
        Icon::Maximize => {
            line(&[[4.0, 9.0], [4.0, 4.0], [9.0, 4.0]]);
            line(&[[15.0, 4.0], [20.0, 4.0], [20.0, 9.0]]);
            line(&[[20.0, 15.0], [20.0, 20.0], [15.0, 20.0]]);
            line(&[[9.0, 20.0], [4.0, 20.0], [4.0, 15.0]]);
        }
        Icon::Command => {
            rectangle(8.0, 8.0, 8.0, 8.0, 0.0);
            for (x, y) in [(3.0, 3.0), (16.0, 3.0), (3.0, 16.0), (16.0, 16.0)] {
                rectangle(x, y, 5.0, 5.0, 2.5);
            }
        }
        Icon::Sun => {
            circle(12.0, 12.0, 4.0);
            for index in 0..8 {
                let angle = index as f32 * std::f32::consts::TAU / 8.0;
                let direction = [angle.cos(), angle.sin()];
                line(&[
                    [12.0 + direction[0] * 7.0, 12.0 + direction[1] * 7.0],
                    [12.0 + direction[0] * 9.5, 12.0 + direction[1] * 9.5],
                ]);
            }
        }
        Icon::Moon => {
            // A continuous crescent keeps the background visible, including
            // when the icon is drawn on a selected control.
            let mut crescent = Vec::with_capacity(33);
            for step in 0..=20 {
                let angle = (-93.0 - step as f32 * 13.2).to_radians();
                crescent.push([12.0 + 9.0 * angle.cos(), 12.0 + 9.0 * angle.sin()]);
            }
            for step in 0..=12 {
                let angle = (65.2 + step as f32 * (139.6 / 12.0)).to_radians();
                crescent.push([18.0 + 7.125 * angle.cos(), 6.0 + 7.125 * angle.sin()]);
            }
            crescent.push(crescent[0]);
            line(&crescent);
        }
        Icon::Copy => {
            rectangle(8.0, 8.0, 12.0, 13.0, 2.5);
            line(&[
                [15.0, 5.0],
                [15.0, 3.0],
                [4.0, 3.0],
                [3.0, 4.0],
                [3.0, 15.0],
                [5.0, 15.0],
            ]);
        }
    }
}

/// A compact, keyboard-focusable icon button with a circular hover surface.
pub fn button(ui: &mut Ui, icon: Icon, tooltip: &str) -> Response {
    let (_, rect) = ui.allocate_space(vec2(28.0, 28.0));
    let response = ui.interact(rect, ui.id().with(("icon-button", tooltip)), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), tooltip));
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        if response.hovered() || response.is_pointer_button_down_on() {
            ui.painter()
                .circle_filled(rect.center(), 13.0, visuals.weak_bg_fill);
        }
        if response.has_focus() {
            ui.painter()
                .circle_stroke(rect.center(), 13.0, ui.visuals().selection.stroke);
        }
        paint(
            ui.painter(),
            rect.shrink(6.0),
            icon,
            visuals.fg_stroke.color,
        );
    }
    response
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(tooltip)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{self, Align, Event, Layout, Modifiers, PointerButton, UiBuilder};

    fn title_controls_frame(ctx: &egui::Context, events: Vec<Event>) -> Response {
        let mut close = None;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1180.0, 760.0))),
                events,
                ..Default::default()
            },
            |ui| {
                let title = Rect::from_min_size(Pos2::ZERO, vec2(1180.0, 46.0));
                ui.interact(
                    title.shrink2(vec2(180.0, 0.0)),
                    ui.id().with("title-drag"),
                    Sense::click_and_drag(),
                );
                ui.scope_builder(
                    UiBuilder::new()
                        .max_rect(Rect::from_min_size(Pos2::new(112.0, 9.0), vec2(64.0, 28.0)))
                        .layout(Layout::left_to_right(Align::Center)),
                    |ui| {
                        button(ui, Icon::Sidebar, "Toggle sidebar");
                    },
                );
                ui.scope_builder(
                    UiBuilder::new()
                        .max_rect(Rect::from_min_max(
                            Pos2::new(title.right() - 164.0, 9.0),
                            title.max - vec2(9.0, 9.0),
                        ))
                        .layout(Layout::right_to_left(Align::Center)),
                    |ui| {
                        close = Some(button(ui, Icon::Close, "Close window"));
                        button(ui, Icon::Maximize, "Maximize");
                        button(ui, Icon::Minus, "Minimize");
                        ui.add_space(8.0);
                        button(ui, Icon::Command, "Commands");
                    },
                );
            },
        );
        output.textures_delta.clear();
        close.expect("titlebar close control")
    }

    #[test]
    fn rtl_titlebar_icon_retains_identity_and_clicks_on_release() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx, crate::config::Theme::Graphite);
        let initial = title_controls_frame(&ctx, vec![]);
        let pos = initial.rect.center();
        assert_eq!(initial.rect.size(), vec2(28.0, 28.0));
        assert!(!initial.clicked());

        let hover = title_controls_frame(&ctx, vec![Event::PointerMoved(pos)]);
        assert_eq!(hover.id, initial.id);
        assert!(hover.hovered());
        assert!(!hover.clicked());

        let press = title_controls_frame(
            &ctx,
            vec![Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            }],
        );
        assert_eq!(press.id, initial.id);
        assert!(press.is_pointer_button_down_on());
        assert!(!press.clicked());

        let release = title_controls_frame(
            &ctx,
            vec![Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert_eq!(release.id, initial.id);
        assert!(release.clicked());
        assert!(!release.is_pointer_button_down_on());
        assert!(!title_controls_frame(&ctx, vec![]).clicked());
    }

    fn action_controls_frame(
        ctx: &egui::Context,
        show_sidebar_control: bool,
        focus_find: bool,
        events: Vec<Event>,
    ) -> (Response, Response) {
        let mut actions = None;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(640.0, 480.0))),
                events,
                ..Default::default()
            },
            |ui| {
                ui.horizontal(|ui| {
                    if show_sidebar_control {
                        button(ui, Icon::Sidebar, "Toggle sidebar");
                    }
                    let find = button(ui, Icon::Search, "Find in terminal");
                    let close = button(ui, Icon::Close, "Close terminal");
                    if focus_find {
                        find.request_focus();
                    }
                    actions = Some((find, close));
                });
            },
        );
        output.textures_delta.clear();
        actions.expect("action controls")
    }

    #[test]
    fn hiding_neighboring_controls_preserves_focus_and_keyboard_activation() {
        let ctx = egui::Context::default();
        let (initial_find, initial_close) = action_controls_frame(&ctx, true, true, vec![]);
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(initial_find.id));

        // Responsive chrome can remove an earlier control. Positional auto IDs
        // would now assign Find's focused ID to Close and activate that action.
        let (find, close) = action_controls_frame(
            &ctx,
            false,
            false,
            vec![Event::Key {
                key: egui::Key::Enter,
                physical_key: Some(egui::Key::Enter),
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );

        assert_eq!(find.id, initial_find.id);
        assert_eq!(close.id, initial_close.id);
        assert!(find.has_focus());
        assert!(
            find.clicked(),
            "Enter did not activate the focused Find control"
        );
        assert!(!close.has_focus());
        assert!(
            !close.clicked(),
            "Enter activated a different control after layout changed"
        );
    }
}

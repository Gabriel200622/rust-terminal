//! Shared controls and surfaces. Each paints from palette tokens, keeps a stable
//! semantic identity and reports a native accessible role, so the custom chrome
//! stays keyboard-reachable and inspectable.
use crate::{
    icons::{self, Icon},
    theme::{self, Palette, metrics},
};
use eframe::egui::{
    self, Align, Align2, Color32, CursorIcon, FontId, Frame, Galley, Id, Layout, Margin, Painter,
    Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2, WidgetInfo, WidgetType,
    vec2,
};
use std::{ops::RangeInclusive, sync::Arc};

/// Ease-out progress of a boolean state. Repaints only while it is moving, so
/// a settled interface stays idle.
pub fn animate(ctx: &egui::Context, id: Id, on: bool, seconds: f32) -> f32 {
    ctx.animate_bool_with_time_and_easing(id, on, seconds, egui::emath::easing::cubic_out)
}

/// Lays out controls inside an already allocated rectangle. Unlike a scope, it
/// leaves the parent's cursor alone, so a row keeps its declared height.
pub fn place<R>(
    ui: &mut Ui,
    rect: Rect,
    layout: Layout,
    salt: impl std::hash::Hash + std::fmt::Debug,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    let mut child = ui.new_child(
        UiBuilder::new()
            .id_salt(Id::new(salt))
            .max_rect(rect)
            .layout(layout),
    );
    add_contents(&mut child)
}

/// One row of text, truncated with an ellipsis at `max_width`.
pub fn elided(
    painter: &Painter,
    text: &str,
    font: FontId,
    color: Color32,
    max_width: f32,
) -> Arc<Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_width.max(0.0));
    painter.layout_job(job)
}

/// Paints a galley with its vertical centre on `left_center`.
pub fn galley_at(painter: &Painter, left_center: Pos2, galley: Arc<Galley>) -> Rect {
    let pos = Pos2::new(left_center.x, left_center.y - galley.size().y * 0.5);
    let rect = Rect::from_min_size(pos, galley.size());
    painter.galley(pos, galley, Color32::PLACEHOLDER);
    rect
}

/// The platform's primary command chord for a key, e.g. `Ctrl+Shift+D` or `⌘D`.
pub fn shortcut(key: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("⌘{key}")
    } else {
        format!("Ctrl+Shift+{key}")
    }
}

/// The platform's editing chord, used for preferences and app zoom.
pub fn edit_shortcut(key: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("⌘{key}")
    } else {
        format!("Ctrl+{key}")
    }
}

/// Splits a chord into its keys. A trailing `+` is the plus key itself.
fn chord_keys(shortcut: &str) -> Vec<&str> {
    let (modifiers, plus) = match shortcut.strip_suffix('+') {
        Some(rest) => (rest.strip_suffix('+').unwrap_or(rest), true),
        None => (shortcut, false),
    };
    modifiers
        .split('+')
        .filter(|key| !key.is_empty())
        .chain(plus.then_some("+"))
        .collect()
}

/// Paints right-aligned keycaps for a shortcut and returns their total width.
pub fn keycaps(painter: &Painter, right_center: Pos2, shortcut: &str, text: Color32) -> f32 {
    if shortcut.is_empty() {
        return 0.0;
    }
    let font = theme::medium(10.5);
    let mut right = right_center.x;
    for key in chord_keys(shortcut).into_iter().rev() {
        let galley = painter.layout_no_wrap(key.to_owned(), font.clone(), text);
        let width = (galley.size().x + 10.0).max(20.0);
        let cap = Rect::from_min_size(
            Pos2::new(right - width, right_center.y - 9.0),
            vec2(width, 18.0),
        );
        painter.rect_filled(cap, 5, theme::tint(text, 0.12));
        painter.galley(
            cap.center() - galley.size() * 0.5,
            galley,
            Color32::PLACEHOLDER,
        );
        right -= width + 3.0;
    }
    right_center.x - right - 3.0
}

fn focus_ring(painter: &Painter, rect: Rect, radius: u8, p: Palette) {
    painter.rect_stroke(
        rect.expand(2.0),
        radius.saturating_add(2),
        Stroke::new(2.0, theme::tint(p.accent, 0.55)),
        StrokeKind::Inside,
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    /// The one recommended action of a surface.
    Primary,
    Secondary,
    /// Ends processes or discards work.
    Destructive,
    /// Text-only, for tertiary actions.
    Quiet,
}

pub fn button(ui: &mut Ui, p: Palette, label: &str, kind: ButtonKind) -> Response {
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_owned(), theme::medium(13.0), Color32::PLACEHOLDER);
    let padding = if kind == ButtonKind::Quiet {
        10.0
    } else {
        16.0
    };
    let size = vec2(
        (galley.size().x + padding * 2.0).max(72.0),
        metrics::CONTROL_HEIGHT,
    );
    let (_, rect) = ui.allocate_space(size);
    let response = ui.interact(rect, ui.id().with(("button", label)), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), label));
    if ui.is_rect_visible(rect) {
        let down = response.is_pointer_button_down_on();
        let (fill, text) = match kind {
            ButtonKind::Primary => (p.accent, p.on_accent),
            ButtonKind::Destructive => (p.red, Color32::WHITE),
            ButtonKind::Secondary => (p.control, p.fg),
            ButtonKind::Quiet => (Color32::TRANSPARENT, p.secondary),
        };
        let radius = metrics::CONTROL_RADIUS;
        let painter = ui.painter();
        painter.rect_filled(rect, radius, fill);
        let solid = matches!(kind, ButtonKind::Primary | ButtonKind::Destructive);
        if down {
            painter.rect_filled(
                rect,
                radius,
                if solid {
                    Color32::from_black_alpha(46)
                } else {
                    p.pressed
                },
            );
        } else if response.hovered() {
            painter.rect_filled(
                rect,
                radius,
                if solid {
                    Color32::from_white_alpha(26)
                } else {
                    p.hover
                },
            );
        }
        if response.has_focus() {
            focus_ring(painter, rect, radius, p);
        }
        painter.galley(rect.center() - galley.size() * 0.5, galley, text);
    }
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// A switch for a setting that applies immediately.
pub fn toggle(ui: &mut Ui, p: Palette, on: &mut bool, label: &str) -> Response {
    let (_, rect) = ui.allocate_space(vec2(38.0, 22.0));
    let mut response = ui.interact(rect, ui.id().with(("toggle", label)), Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let state = *on;
    response
        .widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, ui.is_enabled(), state, label));
    if ui.is_rect_visible(rect) {
        let progress = animate(ui.ctx(), response.id.with("progress"), state, 0.16);
        let painter = ui.painter();
        painter.rect_filled(rect, 11, p.pressed);
        painter.rect_filled(rect, 11, theme::tint(p.accent, progress));
        if response.has_focus() {
            focus_ring(painter, rect, 11, p);
        }
        let travel = rect.width() - 22.0;
        let knob = Pos2::new(rect.left() + 11.0 + travel * progress, rect.center().y);
        painter.circle_filled(knob + vec2(0.0, 1.0), 9.0, Color32::from_black_alpha(40));
        painter.circle_filled(knob, 9.0, Color32::WHITE);
    }
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// Mutually exclusive choices with a thumb that slides to the selection.
pub fn segmented<T: PartialEq + Copy>(
    ui: &mut Ui,
    p: Palette,
    name: &str,
    value: &mut T,
    options: &[(T, &str)],
    width: f32,
) -> bool {
    let (_, rect) = ui.allocate_space(vec2(width, 28.0));
    let id = ui.id().with(("segmented", name));
    let segment = (rect.width() - 4.0) / options.len().max(1) as f32;
    let selected = options
        .iter()
        .position(|(option, _)| option == value)
        .unwrap_or(0);
    let painter = ui.painter().clone();
    painter.rect_filled(rect, 8, p.control);
    let thumb_x = ui
        .ctx()
        .animate_value_with_time(id.with("thumb"), selected as f32, 0.14);
    let thumb = Rect::from_min_size(
        Pos2::new(rect.left() + 2.0 + thumb_x * segment, rect.top() + 2.0),
        vec2(segment, rect.height() - 4.0),
    );
    painter.rect_filled(
        thumb.translate(vec2(0.0, 1.0)),
        6,
        Color32::from_black_alpha(if p.dark { 50 } else { 22 }),
    );
    painter.rect_filled(
        thumb,
        6,
        if p.dark {
            theme::mix(p.elevated, Color32::WHITE, 0.16)
        } else {
            Color32::WHITE
        },
    );
    let mut changed = false;
    for (index, (option, label)) in options.iter().enumerate() {
        let cell = Rect::from_min_size(
            Pos2::new(rect.left() + 2.0 + index as f32 * segment, rect.top()),
            vec2(segment, rect.height()),
        );
        let response = ui.interact(cell, id.with(*label), Sense::click());
        let active = index == selected;
        response.widget_info(|| {
            WidgetInfo::selected(WidgetType::RadioButton, ui.is_enabled(), active, *label)
        });
        if response.clicked() && !active {
            *value = *option;
            changed = true;
        }
        if response.has_focus() {
            focus_ring(&painter, cell.shrink(2.0), 6, p);
        }
        painter.text(
            cell.center(),
            Align2::CENTER_CENTER,
            *label,
            theme::medium(12.0),
            if active || response.hovered() {
                p.fg
            } else {
                p.secondary
            },
        );
        response.on_hover_cursor(CursorIcon::PointingHand);
    }
    changed
}

/// A continuous value with a filled track. Arrow keys nudge by one step.
pub fn slider(
    ui: &mut Ui,
    p: Palette,
    label: &str,
    value: &mut f32,
    range: RangeInclusive<f32>,
    step: f32,
    width: f32,
) -> Response {
    let (_, rect) = ui.allocate_space(vec2(width, 22.0));
    let mut response = ui.interact(
        rect,
        ui.id().with(("slider", label)),
        Sense::click_and_drag(),
    );
    let (low, high) = (*range.start(), *range.end());
    let knob_radius = 8.0;
    let left = rect.left() + knob_radius;
    let travel = (rect.width() - knob_radius * 2.0).max(1.0);
    let snap = |raw: f32| ((raw / step).round() * step).clamp(low, high);
    let mut next = *value;
    if let Some(pos) = response.interact_pointer_pos() {
        next = snap(low + (high - low) * ((pos.x - left) / travel).clamp(0.0, 1.0));
    }
    if response.has_focus() {
        let nudge = ui.input(|input| {
            input.num_presses(egui::Key::ArrowRight) as f32
                - input.num_presses(egui::Key::ArrowLeft) as f32
        });
        if nudge != 0.0 {
            next = snap(*value + nudge * step);
        }
    }
    if next != *value {
        *value = next;
        response.mark_changed();
    }
    let current = *value;
    response.widget_info(|| WidgetInfo::slider(ui.is_enabled(), current as f64, label));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let fraction = ((current - low) / (high - low).max(f32::EPSILON)).clamp(0.0, 1.0);
        let knob = Pos2::new(left + travel * fraction, rect.center().y);
        let track = Rect::from_min_max(
            Pos2::new(rect.left(), rect.center().y - 2.0),
            Pos2::new(rect.right(), rect.center().y + 2.0),
        );
        painter.rect_filled(track, 2, p.pressed);
        painter.rect_filled(
            Rect::from_min_max(track.min, Pos2::new(knob.x, track.bottom())),
            2,
            p.accent,
        );
        if response.has_focus() {
            painter.circle_stroke(
                knob,
                knob_radius + 2.0,
                Stroke::new(2.0, theme::tint(p.accent, 0.55)),
            );
        }
        painter.circle_filled(
            knob + vec2(0.0, 1.0),
            knob_radius,
            Color32::from_black_alpha(50),
        );
        painter.circle_filled(knob, knob_radius, Color32::WHITE);
    }
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// A bounded number adjusted in fixed steps. Returns whether it changed.
pub fn stepper(
    ui: &mut Ui,
    p: Palette,
    label: &str,
    value: &mut f32,
    range: RangeInclusive<f32>,
    step: f32,
    text: &str,
) -> bool {
    let (_, rect) = ui.allocate_space(vec2(112.0, 28.0));
    let id = ui.id().with(("stepper", label));
    let painter = ui.painter().clone();
    painter.rect_filled(rect, 8, p.control);
    let mut changed = false;
    for (direction, icon, name, x) in [
        (-1.0, Icon::Minus, "Decrease", rect.left()),
        (1.0, Icon::Plus, "Increase", rect.right() - 30.0),
    ] {
        let cell = Rect::from_min_size(Pos2::new(x, rect.top()), vec2(30.0, rect.height()));
        let next = (*value + direction * step).clamp(*range.start(), *range.end());
        let enabled = next != *value;
        let response = ui.interact(cell, id.with(name), Sense::click());
        response.widget_info(|| {
            WidgetInfo::labeled(
                WidgetType::Button,
                enabled,
                format!("{name} {}", label.to_lowercase()),
            )
        });
        if enabled && response.is_pointer_button_down_on() {
            painter.rect_filled(cell.shrink(2.0), 6, p.pressed);
        } else if enabled && response.hovered() {
            painter.rect_filled(cell.shrink(2.0), 6, p.hover);
        }
        if response.has_focus() {
            focus_ring(&painter, cell.shrink(2.0), 6, p);
        }
        icons::paint(
            &painter,
            Rect::from_center_size(cell.center(), Vec2::splat(12.0)),
            icon,
            if !enabled {
                p.muted
            } else if response.hovered() {
                p.fg
            } else {
                p.secondary
            },
        );
        if response.clicked() && enabled {
            *value = next;
            changed = true;
        }
        if enabled {
            response.on_hover_cursor(CursorIcon::PointingHand);
        }
    }
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        theme::medium(12.5),
        p.fg,
    );
    changed
}

/// A filled single-line field with an accent focus ring. `label` is its
/// accessible name; the caller owns `id` so focus can be requested by identity.
pub fn text_field(
    ui: &mut Ui,
    p: Palette,
    id: Id,
    text: &mut String,
    hint: &str,
    label: &str,
    width: f32,
) -> Response {
    let (_, rect) = ui.allocate_space(vec2(width, metrics::CONTROL_HEIGHT));
    let focused = ui.memory(|memory| memory.has_focus(id));
    let radius = metrics::CONTROL_RADIUS;
    ui.painter().rect_filled(rect, radius, p.control);
    if focused {
        ui.painter().rect_stroke(
            rect.expand(2.5),
            radius.saturating_add(2),
            Stroke::new(3.0, theme::tint(p.accent, 0.28)),
            StrokeKind::Inside,
        );
        ui.painter()
            .rect_stroke(rect, radius, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    } else {
        ui.painter()
            .rect_stroke(rect, radius, p.hairline(), StrokeKind::Inside);
    }
    let inner = rect.shrink2(vec2(10.0, 0.0));
    let response = place(
        ui,
        inner,
        Layout::left_to_right(Align::Center),
        ("text-field", id),
        |ui| bare_text_edit(ui, id, text, hint, 13.0, inner.width()),
    );
    response.widget_info(|| WidgetInfo::labeled(WidgetType::TextEdit, true, label));
    response
}

/// A frameless single-line editor for surfaces that draw their own field.
pub fn bare_text_edit(
    ui: &mut Ui,
    id: Id,
    text: &mut String,
    hint: &str,
    size: f32,
    width: f32,
) -> Response {
    ui.add(
        egui::TextEdit::singleline(text)
            .id(id)
            .hint_text(hint)
            .font(theme::regular(size))
            .frame(Frame::NONE)
            .background_color(Color32::TRANSPARENT)
            .margin(Margin::ZERO)
            .desired_width(width),
    )
}

/// Dims the window behind a sheet. Follows the window's rounded shape so the
/// transparent corners stay clear.
pub fn scrim(painter: &Painter, bounds: Rect, radius: u8, p: Palette, opacity: f32) {
    if opacity > 0.0 {
        painter.rect_filled(bounds, radius, p.scrim.gamma_multiply(opacity));
    }
}

#[derive(Clone, Copy)]
pub enum SheetPlacement {
    Center,
    /// Hangs from the top of the window, for command-style surfaces.
    Top(f32),
}

pub struct SheetOutput<R> {
    pub inner: R,
    /// The dimmed area outside the sheet was clicked.
    pub backdrop_clicked: bool,
}

/// A modal surface. Keyboard focus stays inside it, and it is exposed to
/// assistive tools as a window named `title`.
pub fn sheet<R>(
    ctx: &egui::Context,
    p: Palette,
    title: &str,
    width: f32,
    placement: SheetPlacement,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> SheetOutput<R> {
    let id = Id::new(("sheet", title));
    let screen = ctx.content_rect();
    let width = width.min(screen.width() - 24.0).max(120.0);
    let area = egui::Modal::default_area(id);
    let area = match placement {
        SheetPlacement::Center => area,
        SheetPlacement::Top(offset) => area
            .anchor(Align2::CENTER_TOP, vec2(0.0, offset))
            .fade_in(false),
    };
    let frame = Frame::new()
        .fill(p.elevated)
        .corner_radius(metrics::SHEET_RADIUS)
        .stroke(Stroke::new(1.0, p.border))
        .shadow(p.sheet_shadow());
    let modal = egui::Modal::new(id)
        .area(area)
        .frame(frame)
        // The window paints its own rounded scrim.
        .backdrop_color(Color32::TRANSPARENT)
        .show(ctx, |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            add_contents(ui)
        });
    modal
        .response
        .widget_info(|| WidgetInfo::labeled(WidgetType::Window, true, title));
    SheetOutput {
        inner: modal.inner,
        backdrop_clicked: modal.backdrop_response.clicked(),
    }
}

/// The title row of a sheet. Returns true when its close control is activated.
pub fn sheet_header(ui: &mut Ui, p: Palette, title: &str, close_label: Option<&str>) -> bool {
    let (_, rect) = ui.allocate_space(vec2(ui.available_width(), 52.0));
    ui.painter().text(
        Pos2::new(rect.left() + 20.0, rect.center().y),
        Align2::LEFT_CENTER,
        title,
        theme::semibold(15.0),
        p.fg,
    );
    let Some(close_label) = close_label else {
        return false;
    };
    place(
        ui,
        Rect::from_center_size(
            Pos2::new(rect.right() - 26.0, rect.center().y),
            Vec2::splat(28.0),
        ),
        Layout::left_to_right(Align::Center),
        ("sheet-close", title),
        |ui| icons::button(ui, Icon::Close, close_label).clicked(),
    )
}

/// Left padding, content, right padding: the standard sheet gutter.
pub fn padded<R>(ui: &mut Ui, horizontal: f32, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    let width = ui.available_width() - horizontal * 2.0;
    ui.horizontal(|ui| {
        ui.add_space(horizontal);
        ui.vertical(|ui| {
            ui.set_width(width);
            add_contents(ui)
        })
        .inner
    })
    .inner
}

/// A small label above a group of related settings.
pub fn section_label(ui: &mut Ui, p: Palette, text: &str) {
    let (_, rect) = ui.allocate_space(vec2(ui.available_width(), 24.0));
    ui.painter().text(
        Pos2::new(rect.left() + 4.0, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        theme::medium(11.5),
        p.secondary,
    );
}

/// Supporting text beneath a group.
pub fn caption(ui: &mut Ui, p: Palette, text: &str) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        ui.add(
            egui::Label::new(egui::RichText::new(text).size(11.5).color(p.muted))
                .wrap()
                .selectable(false),
        );
    });
}

/// An inset card of setting rows separated by hairlines.
pub struct Group {
    p: Palette,
    rows: usize,
}

pub fn group(ui: &mut Ui, p: Palette, add_rows: impl FnOnce(&mut Ui, &mut Group)) {
    let background = ui.painter().add(egui::Shape::Noop);
    let top = ui.cursor().top();
    let width = ui.available_width();
    let left = ui.cursor().left();
    let mut group = Group { p, rows: 0 };
    ui.vertical(|ui| {
        ui.set_width(width);
        add_rows(ui, &mut group);
    });
    let rect = Rect::from_min_max(
        Pos2::new(left, top),
        Pos2::new(left + width, ui.cursor().top()),
    );
    ui.painter()
        .set(background, egui::Shape::rect_filled(rect, 10, p.control));
}

impl Group {
    /// A labelled row whose control is aligned to the trailing edge.
    pub fn row<R>(
        &mut self,
        ui: &mut Ui,
        label: &str,
        add_control: impl FnOnce(&mut Ui) -> R,
    ) -> R {
        self.row_with_height(ui, label, 42.0, add_control)
    }

    pub fn row_with_height<R>(
        &mut self,
        ui: &mut Ui,
        label: &str,
        height: f32,
        add_control: impl FnOnce(&mut Ui) -> R,
    ) -> R {
        let (_, rect) = ui.allocate_space(vec2(ui.available_width(), height));
        if self.rows > 0 {
            ui.painter().line_segment(
                [
                    Pos2::new(rect.left() + 14.0, rect.top()),
                    Pos2::new(rect.right(), rect.top()),
                ],
                self.p.hairline(),
            );
        }
        self.rows += 1;
        ui.painter().text(
            Pos2::new(rect.left() + 14.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            theme::regular(13.0),
            self.p.fg,
        );
        place(
            ui,
            rect.shrink2(vec2(12.0, 0.0)),
            Layout::right_to_left(Align::Center),
            ("group-row", label),
            add_control,
        )
    }
}

/// A row in a pop-up menu. The hovered row takes the accent, as native menus do.
pub fn menu_item(
    ui: &mut Ui,
    p: Palette,
    icon: Icon,
    label: &str,
    shortcut: &str,
    destructive: bool,
) -> bool {
    let (_, rect) = ui.allocate_space(vec2(ui.available_width(), 28.0));
    let response = ui.interact(rect, ui.id().with(("menu-item", label)), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), label));
    let highlighted = response.hovered() || response.has_focus();
    let painter = ui.painter();
    let (text, hint) = if highlighted {
        painter.rect_filled(rect, 6, if destructive { p.red } else { p.accent });
        let ink = if destructive {
            Color32::WHITE
        } else {
            p.on_accent
        };
        (ink, ink)
    } else if destructive {
        (p.red, p.muted)
    } else {
        (p.fg, p.muted)
    };
    icons::paint(
        painter,
        Rect::from_center_size(
            Pos2::new(rect.left() + 15.0, rect.center().y),
            Vec2::splat(14.0),
        ),
        icon,
        text,
    );
    painter.text(
        Pos2::new(rect.left() + 32.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        theme::regular(13.0),
        text,
    );
    if !shortcut.is_empty() {
        painter.text(
            Pos2::new(rect.right() - 10.0, rect.center().y),
            Align2::RIGHT_CENTER,
            shortcut,
            theme::regular(11.5),
            hint,
        );
    }
    response.clicked()
}

pub fn menu_separator(ui: &mut Ui, p: Palette) {
    let (_, rect) = ui.allocate_space(vec2(ui.available_width(), 9.0));
    ui.painter().line_segment(
        [
            Pos2::new(rect.left() + 8.0, rect.center().y),
            Pos2::new(rect.right() - 8.0, rect.center().y),
        ],
        p.hairline(),
    );
}

/// Shared geometry for pop-up menus built from [`menu_item`]. The width is
/// fixed: a justified menu would otherwise grow to the widest space offered.
pub fn menu_layout(ui: &mut Ui, width: f32) {
    ui.set_width(width);
    ui.spacing_mut().item_spacing = Vec2::ZERO;
}

/// A floating capsule, used for status inside a pane.
pub fn capsule(painter: &Painter, rect: Rect, p: Palette) {
    let radius = (rect.height() * 0.5).round() as u8;
    painter.add(p.popup_shadow().as_shape(rect, radius));
    painter.rect_filled(rect, radius, p.elevated);
    painter.rect_stroke(rect, radius, Stroke::new(1.0, p.border), StrokeKind::Inside);
}

/// A non-blocking message in the window's top trailing corner, clear of pane
/// status and the active prompt. Returns true when dismissed.
pub fn toast(ctx: &egui::Context, p: Palette, message: &str) -> bool {
    let screen = ctx.content_rect();
    let mut dismissed = false;
    egui::Area::new(Id::new("pace-toast"))
        // Above sheets, so a failure reported during a dialog stays reachable.
        .order(egui::Order::Tooltip)
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
                .inner_margin(Margin {
                    left: 14,
                    right: 8,
                    top: 8,
                    bottom: 8,
                })
                .show(ui, |ui| {
                    ui.set_max_width((screen.width() - 96.0).clamp(160.0, 440.0));
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        let (_, icon) = ui.allocate_space(Vec2::splat(16.0));
                        icons::paint(ui.painter(), icon, Icon::Warning, p.red);
                        let close = ui.available_width() - 38.0;
                        ui.scope(|ui| {
                            ui.set_max_width(close.max(80.0));
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(message).size(12.5).color(p.fg),
                                )
                                .wrap(),
                            );
                        });
                        dismissed = icons::button(ui, Icon::Close, "Dismiss").clicked();
                    });
                });
        });
    dismissed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use eframe::egui::{Event, Modifiers, PointerButton};

    fn context() -> egui::Context {
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::platform::fonts::bundled_definitions());
        theme::apply(&ctx, &Config::default());
        ctx
    }

    fn frame(ctx: &egui::Context, events: Vec<Event>, mut add: impl FnMut(&mut Ui)) {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(480.0, 320.0))),
                events,
                ..Default::default()
            },
            |ui| add(ui),
        );
        output.textures_delta.clear();
    }

    fn click(ctx: &egui::Context, pos: Pos2, mut add: impl FnMut(&mut Ui)) {
        frame(ctx, vec![Event::PointerMoved(pos)], &mut add);
        for pressed in [true, false] {
            frame(
                ctx,
                vec![Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                }],
                &mut add,
            );
        }
    }

    #[test]
    fn toggle_flips_once_per_click_and_keeps_its_identity() {
        let ctx = context();
        let p = Palette::new(Config::default().theme);
        let mut on = false;
        let mut target = None;
        let mut id = None;
        frame(&ctx, vec![], |ui| {
            let response = toggle(ui, p, &mut on, "Blink cursor");
            target = Some(response.rect.center());
            id = Some(response.id);
        });
        let mut after = None;
        click(&ctx, target.unwrap(), |ui| {
            after = Some(toggle(ui, p, &mut on, "Blink cursor").id);
        });
        assert!(on, "one click turns the switch on");
        assert_eq!(id, after);
        frame(&ctx, vec![], |ui| {
            toggle(ui, p, &mut on, "Blink cursor");
        });
        assert!(on, "an idle frame does not toggle again");
    }

    #[test]
    fn segmented_selects_the_clicked_option_only() {
        let ctx = context();
        let p = Palette::new(Config::default().theme);
        let options = [(0u8, "Block"), (1, "Beam"), (2, "Underline")];
        let mut value = 0u8;
        let mut origin = Pos2::ZERO;
        frame(&ctx, vec![], |ui| {
            origin = ui.cursor().min;
            segmented(ui, p, "cursor", &mut value, &options, 240.0);
        });
        // The third of three equal segments.
        click(&ctx, origin + vec2(200.0, 14.0), |ui| {
            segmented(ui, p, "cursor", &mut value, &options, 240.0);
        });
        assert_eq!(value, 2);
    }

    #[test]
    fn slider_snaps_to_steps_and_clamps_to_its_range() {
        let ctx = context();
        let p = Palette::new(Config::default().theme);
        let mut value = 1.4f32;
        let mut rect = Rect::NOTHING;
        frame(&ctx, vec![], |ui| {
            rect = slider(ui, p, "Line spacing", &mut value, 1.0..=2.0, 0.05, 200.0).rect;
        });
        click(
            &ctx,
            Pos2::new(rect.right() + 40.0, rect.center().y),
            |ui| {
                slider(ui, p, "Line spacing", &mut value, 1.0..=2.0, 0.05, 200.0);
            },
        );
        // A press outside the track is not this slider's interaction.
        assert_eq!(value, 1.4);
        click(&ctx, Pos2::new(rect.right() - 1.0, rect.center().y), |ui| {
            slider(ui, p, "Line spacing", &mut value, 1.0..=2.0, 0.05, 200.0);
        });
        assert_eq!(value, 2.0);
        click(&ctx, rect.center(), |ui| {
            slider(ui, p, "Line spacing", &mut value, 1.0..=2.0, 0.05, 200.0);
        });
        assert!((value - 1.5).abs() < 0.001, "snapped to a step: {value}");
    }

    #[test]
    fn stepper_stops_at_its_bounds() {
        let ctx = context();
        let p = Palette::new(Config::default().theme);
        let mut value = 31.0f32;
        let mut origin = Pos2::ZERO;
        frame(&ctx, vec![], |ui| {
            origin = ui.cursor().min;
            stepper(ui, p, "Font size", &mut value, 9.0..=32.0, 1.0, "31");
        });
        let plus = origin + vec2(112.0 - 15.0, 14.0);
        for _ in 0..3 {
            click(&ctx, plus, |ui| {
                stepper(ui, p, "Font size", &mut value, 9.0..=32.0, 1.0, "");
            });
        }
        assert_eq!(value, 32.0);
    }

    #[test]
    fn chords_split_into_keys_and_keep_a_literal_plus() {
        assert_eq!(chord_keys("Ctrl+Shift+D"), ["Ctrl", "Shift", "D"]);
        assert_eq!(chord_keys("Ctrl++"), ["Ctrl", "+"]);
        assert_eq!(chord_keys("Ctrl+-"), ["Ctrl", "-"]);
        assert_eq!(chord_keys("+"), ["+"]);
        assert_eq!(chord_keys("⌘D"), ["⌘D"]);
        assert_eq!(chord_keys("Esc"), ["Esc"]);
    }

    #[test]
    fn shortcuts_name_the_platform_chord() {
        if cfg!(target_os = "macos") {
            assert_eq!(shortcut("D"), "⌘D");
        } else {
            assert_eq!(shortcut("D"), "Ctrl+Shift+D");
            assert_eq!(edit_shortcut(","), "Ctrl+,");
        }
    }
}

//! Terminal panes: rounded content surfaces arranged by the workspace layout.
use super::helpers::{self, animate, capsule, elided, galley_at, menu_item, shortcut};
use super::{Action, PaneRender};
use crate::{
    config::Config,
    icons::{self, Icon},
    theme::{self, Palette, metrics},
};
use eframe::egui::{
    self, Align, Align2, CursorIcon, Layout, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, UiBuilder,
    Vec2, WidgetInfo, WidgetType, vec2,
};
use pace_model::{Axis, PaneId};
use std::collections::BTreeMap;
use terminal_core::{Mode as TermMode, SessionMetadata, SessionStatus, ViewportSnapshot};

pub struct PanePresentation {
    pub metadata: SessionMetadata,
    pub snapshot: ViewportSnapshot,
    /// The shell has been requested but has not started yet.
    pub starting: bool,
}

/// Per-frame inputs shared by every pane of the visible layout.
pub struct Stage<'a> {
    pub presentations: &'a BTreeMap<PaneId, PanePresentation>,
    pub active: PaneId,
    /// More than one pane is visible, so panes carry headers and focus cues.
    pub multiple: bool,
    pub zoomed: bool,
    /// No overlay is open, so the focused terminal may own the keyboard.
    pub keyboard: bool,
    /// The terminal widget that owned the keyboard on the previous frame.
    pub previous_terminal: Option<egui::Id>,
    pub config: &'a Config,
    pub p: Palette,
    /// Highlighted in the focused pane while search is open.
    pub search: &'a str,
}

#[derive(Default)]
pub struct StageOutput {
    /// Terminal grid of the focused pane, for input and pointer routing.
    pub active_body: Option<Rect>,
    /// Widget identity of the focused terminal, which holds keyboard focus.
    pub active_terminal: Option<egui::Id>,
}

/// The program or shell a pane is showing. Prompt-style titles such as
/// `user@host:~/dir` fall back to the shell name; the path is shown separately.
pub fn pane_label(metadata: &SessionMetadata) -> String {
    let custom = !metadata.title.is_empty()
        && !metadata.title.contains('@')
        && !metadata.title.contains(":/")
        && !metadata.title.contains(":~");
    if custom {
        metadata.title.clone()
    } else {
        std::path::Path::new(&metadata.shell)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    }
}

fn status_text(status: &SessionStatus) -> Option<(String, bool)> {
    match status {
        SessionStatus::Running => None,
        SessionStatus::Exited {
            signal: Some(signal),
            ..
        } => Some((format!("Stopped by {signal}"), true)),
        SessionStatus::Exited { code: 0, .. } => Some(("Process exited".into(), false)),
        SessionStatus::Exited { code, .. } => Some((format!("Exited with code {code}"), true)),
        SessionStatus::Error(_) => Some(("Could not start shell".into(), true)),
    }
}

struct Status<'a> {
    message: &'a str,
    /// Draws the indicator in the problem colour.
    problem: bool,
    /// Longer explanation shown on hover.
    detail: Option<&'a str>,
    action: &'a str,
    action_hint: &'a str,
}

/// A floating status line with one action, centred near the pane's bottom edge.
fn status_capsule(
    ui: &mut Ui,
    card: Rect,
    p: Palette,
    salt: (&str, PaneId),
    status: Status,
) -> bool {
    let Status {
        message,
        problem,
        detail,
        action,
        action_hint,
    } = status;
    let font = theme::medium(12.0);
    let painter = ui.painter().clone();
    let action_galley = painter.layout_no_wrap(action.to_owned(), font.clone(), p.accent);
    let action_width = action_galley.size().x + 20.0;
    let max_text = (card.width() - 24.0 - 34.0 - action_width - 12.0).max(0.0);
    let text = elided(&painter, message, font, p.fg, max_text);
    let width = 30.0 + text.size().x + 12.0 + action_width + 5.0;
    let rect = Rect::from_center_size(
        Pos2::new(card.center().x, card.bottom() - 34.0),
        vec2(width.min(card.width() - 16.0), 34.0),
    );
    if rect.width() < 60.0 || card.height() < 60.0 {
        return false;
    }
    capsule(&painter, rect, p);
    painter.circle_filled(
        Pos2::new(rect.left() + 17.0, rect.center().y),
        3.5,
        if problem { p.red } else { p.muted },
    );
    let text_rect = galley_at(
        &painter,
        Pos2::new(rect.left() + 30.0, rect.center().y),
        text,
    );
    if let Some(detail) = detail {
        ui.interact(text_rect, ui.id().with((salt, "detail")), Sense::hover())
            .on_hover_text(detail);
    }
    let button = Rect::from_min_max(
        Pos2::new(rect.right() - action_width - 5.0, rect.top() + 5.0),
        rect.max - vec2(5.0, 5.0),
    );
    let response = ui.interact(button, ui.id().with((salt, "action")), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, action));
    painter.rect_filled(
        button,
        12,
        theme::tint(
            p.accent,
            if response.is_pointer_button_down_on() {
                0.34
            } else if response.hovered() {
                0.26
            } else {
                0.16
            },
        ),
    );
    if response.has_focus() {
        painter.rect_stroke(button, 12, Stroke::new(1.5, p.accent), StrokeKind::Inside);
    }
    painter.galley(
        button.center() - action_galley.size() * 0.5,
        action_galley,
        p.accent,
    );
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    if action_hint.is_empty() {
        response.clicked()
    } else {
        response.on_hover_text(action_hint).clicked()
    }
}

fn pane_menu(ui: &mut Ui, p: Palette, id: PaneId, zoomed: bool, actions: &mut Vec<Action>) {
    helpers::menu_layout(ui, 240.0);
    let mut chosen: Vec<Action> = Vec::new();
    if menu_item(ui, p, Icon::Copy, "Copy", &shortcut("C"), false) {
        chosen.push(Action::Copy(id));
    }
    if menu_item(ui, p, Icon::Clipboard, "Paste", &shortcut("V"), false) {
        chosen.push(Action::Paste(id));
    }
    helpers::menu_separator(ui, p);
    if menu_item(ui, p, Icon::Search, "Find…", &shortcut("F"), false) {
        chosen.extend([Action::Focus(id), Action::Find]);
    }
    helpers::menu_separator(ui, p);
    if menu_item(
        ui,
        p,
        Icon::SplitVertical,
        "Split right",
        &shortcut("D"),
        false,
    ) {
        chosen.extend([Action::Focus(id), Action::Split(id, Axis::Vertical)]);
    }
    if menu_item(
        ui,
        p,
        Icon::SplitHorizontal,
        "Split below",
        &shortcut("E"),
        false,
    ) {
        chosen.extend([Action::Focus(id), Action::Split(id, Axis::Horizontal)]);
    }
    if menu_item(
        ui,
        p,
        if zoomed {
            Icon::Minimize
        } else {
            Icon::Maximize
        },
        if zoomed {
            "Show all terminals"
        } else {
            "Zoom terminal"
        },
        &shortcut("Enter"),
        false,
    ) {
        chosen.extend([Action::Focus(id), Action::Zoom]);
    }
    helpers::menu_separator(ui, p);
    if menu_item(ui, p, Icon::Eraser, "Clear scrollback", "", false) {
        chosen.push(Action::Clear(id));
    }
    if menu_item(ui, p, Icon::Refresh, "Restart terminal", "", false) {
        chosen.push(Action::Restart(id));
    }
    helpers::menu_separator(ui, p);
    if menu_item(ui, p, Icon::Close, "Close terminal", &shortcut("W"), true) {
        chosen.push(Action::ClosePane(id));
    }
    if !chosen.is_empty() {
        actions.extend(chosen);
        ui.close();
    }
}

fn pane_header(
    ui: &mut Ui,
    id: PaneId,
    header: Rect,
    metadata: &SessionMetadata,
    reveal: f32,
    stage: &Stage,
    actions: &mut Vec<Action>,
) {
    let p = stage.p;
    let selected = id == stage.active;
    let show_close = header.width() >= 72.0;
    let show_all = header.width() >= 260.0;
    let controls_width = if show_all {
        118.0
    } else if show_close {
        34.0
    } else {
        0.0
    };
    let text_rect = Rect::from_min_max(
        Pos2::new((header.left() + 14.0).min(header.right()), header.top()),
        Pos2::new(
            (header.right() - controls_width - 6.0).max(header.left() + 14.0),
            header.bottom(),
        ),
    );
    let painter = ui.painter().with_clip_rect(text_rect);
    let title = elided(
        &painter,
        &pane_label(metadata),
        theme::medium(12.0),
        if selected { p.fg } else { p.secondary },
        text_rect.width().min(220.0),
    );
    let title_width = title.size().x;
    galley_at(
        &painter,
        Pos2::new(text_rect.left(), header.center().y + 1.0),
        title,
    );
    let remaining = text_rect.width() - title_width - 9.0;
    if remaining > 36.0 {
        let folder = metadata
            .cwd
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| helpers::compact_path(&metadata.cwd));
        galley_at(
            &painter,
            Pos2::new(
                text_rect.left() + title_width + 9.0,
                header.center().y + 1.0,
            ),
            elided(&painter, &folder, theme::regular(11.5), p.muted, remaining),
        );
    }
    let title_response = ui.interact(text_rect, ui.id().with(("pane-title", id)), Sense::click());
    if title_response.clicked() {
        actions.push(Action::Focus(id));
    }
    if title_response.double_clicked() {
        actions.push(Action::Zoom);
    }
    title_response.on_hover_text(format!(
        "{}\n{}",
        if metadata.title.is_empty() {
            pane_label(metadata)
        } else {
            metadata.title.clone()
        },
        metadata.cwd.display()
    ));
    if !show_close {
        return;
    }
    ui.scope_builder(
        UiBuilder::new()
            .id_salt(("pane-controls", id))
            .max_rect(Rect::from_min_max(
                Pos2::new(header.right() - controls_width, header.top() + 1.0),
                header.max - vec2(4.0, 1.0),
            ))
            .layout(Layout::right_to_left(Align::Center)),
        |ui| {
            ui.set_clip_rect(header.intersect(ui.clip_rect()));
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.set_opacity(reveal);
            if icons::button_with_hint(ui, Icon::Close, "Close terminal", &shortcut("W")).clicked()
            {
                actions.push(Action::ClosePane(id));
            }
            if !show_all {
                return;
            }
            if icons::button_with_hint(ui, Icon::SplitHorizontal, "Split below", &shortcut("E"))
                .clicked()
            {
                actions.extend([Action::Focus(id), Action::Split(id, Axis::Horizontal)]);
            }
            if icons::button_with_hint(ui, Icon::SplitVertical, "Split right", &shortcut("D"))
                .clicked()
            {
                actions.extend([Action::Focus(id), Action::Split(id, Axis::Vertical)]);
            }
            if icons::button_with_hint(ui, Icon::Maximize, "Zoom terminal", &shortcut("Enter"))
                .clicked()
            {
                actions.extend([Action::Focus(id), Action::Zoom]);
            }
        },
    );
}

fn draw_pane(
    ui: &mut Ui,
    id: PaneId,
    card: Rect,
    pane: &mut PaneRender,
    stage: &Stage,
    actions: &mut Vec<Action>,
    output: &mut StageOutput,
) {
    let p = stage.p;
    let Some(presentation) = stage.presentations.get(&id) else {
        ui.painter().rect_filled(card, metrics::PANE_RADIUS, p.bg);
        return;
    };
    let metadata = &presentation.metadata;
    let selected = id == stage.active;
    let header_height = if stage.multiple {
        metrics::PANE_HEADER
    } else {
        0.0
    };
    let body_min = card.min + vec2(12.0, if stage.multiple { header_height } else { 10.0 });
    let body = Rect::from_min_max(body_min, (card.max - vec2(12.0, 10.0)).max(body_min));

    // Measure and prepare first, so the surface matches a background the
    // running program may have set.
    if let Some(geometry) = pane.cache.geometry(ui, body, stage.config) {
        actions.push(Action::Resize(id, geometry));
    }
    pane.cache
        .prepare(ui, &presentation.snapshot, stage.config, p);
    let surface = pane.cache.background();
    ui.painter()
        .rect_filled(card, metrics::PANE_RADIUS, surface);

    let focus = animate(ui.ctx(), ui.id().with(("pane-focus", id)), selected, 0.14);
    if stage.multiple {
        let hovered = ui.rect_contains_pointer(card);
        let reveal = animate(
            ui.ctx(),
            ui.id().with(("pane-controls-reveal", id)),
            hovered || selected,
            0.12,
        );
        pane_header(
            ui,
            id,
            Rect::from_min_size(card.min, vec2(card.width(), header_height)),
            metadata,
            reveal,
            stage,
            actions,
        );
    }

    ui.scope_builder(UiBuilder::new().id_salt(id).max_rect(body), |ui| {
        let painted = pane.cache.paint(
            ui,
            body,
            stage.config,
            p,
            selected,
            if selected { stage.search } else { "" },
            &pane.preedit,
        );
        let response = painted.response;
        response.widget_info(|| {
            WidgetInfo::labeled(
                WidgetType::Other,
                true,
                format!("Terminal pane {}", id.get()),
            )
        });
        if let Some(interaction) = painted.interaction {
            actions.push(Action::Selection(id, interaction));
        }
        // A focused widget also reports Enter and Space as clicks; only the
        // pointer changes which pane is focused.
        if (response.clicked() && response.interact_pointer_pos().is_some())
            || response.drag_started()
        {
            actions.push(Action::Focus(id));
        }
        if selected {
            output.active_terminal = Some(response.id);
            if stage.keyboard {
                // The terminal owns Tab, arrows and Escape. It takes focus
                // when nothing has it, or straight from the terminal that had
                // it before a split, close or workspace switch. A focused
                // field such as search keeps the keyboard.
                let focused = ui.memory(|memory| memory.focused());
                if focused != Some(response.id)
                    && (focused.is_none() || focused == stage.previous_terminal)
                {
                    response.request_focus();
                    // The key filter below only takes effect from the frame
                    // after focus is gained.
                    ui.ctx().request_repaint();
                }
                ui.memory_mut(|memory| {
                    memory.set_focus_lock_filter(
                        response.id,
                        egui::EventFilter {
                            tab: true,
                            horizontal_arrows: true,
                            vertical_arrows: true,
                            escape: true,
                        },
                    );
                });
            }
        } else if response.has_focus() {
            response.surrender_focus();
            ui.ctx().request_repaint();
        }
        if !pane.cache.mode.intersects(TermMode::MOUSE_MODE) || ui.input(|i| i.modifiers.shift) {
            response.context_menu(|ui| pane_menu(ui, p, id, stage.zoomed, actions));
        }
    });

    let painter = ui.painter().clone();
    if stage.multiple {
        // Unfocused panes recede slightly; the focused one carries the accent.
        if focus < 1.0 {
            painter.rect_filled(
                card,
                metrics::PANE_RADIUS,
                theme::tint(surface, 0.24 * (1.0 - focus)),
            );
        }
    }
    painter.rect_stroke(
        card,
        metrics::PANE_RADIUS,
        Stroke::new(1.0, p.separator),
        StrokeKind::Inside,
    );
    if stage.multiple && focus > 0.0 {
        painter.rect_stroke(
            card,
            metrics::PANE_RADIUS,
            Stroke::new(1.5, theme::tint(p.accent, 0.85 * focus)),
            StrokeKind::Inside,
        );
    }

    if presentation.starting {
        let center = card.center();
        egui::Spinner::new().size(14.0).color(p.muted).paint_at(
            ui,
            Rect::from_center_size(center - vec2(52.0, 0.0), Vec2::splat(14.0)),
        );
        painter.text(
            center - vec2(38.0, 0.0),
            Align2::LEFT_CENTER,
            "Starting shell…",
            theme::regular(12.5),
            p.muted,
        );
    } else if let Some((message, problem)) = status_text(&metadata.status) {
        let detail = match &metadata.status {
            SessionStatus::Error(error) => Some(error.as_str()),
            _ => None,
        };
        if status_capsule(
            ui,
            card,
            p,
            ("pane-status", id),
            Status {
                message: &message,
                problem,
                detail,
                action: "Restart",
                action_hint: "Start a new shell   Enter",
            },
        ) {
            actions.extend([Action::Focus(id), Action::Restart(id)]);
        }
    } else if let Some(error) = pane.cache.resize_error.clone()
        && status_capsule(
            ui,
            card,
            p,
            ("resize-error", id),
            Status {
                message: "Resize unavailable",
                problem: true,
                detail: Some(&error),
                action: "Retry",
                action_hint: "",
            },
        )
    {
        pane.cache.retry_resize();
        ui.ctx().request_repaint();
    }

    if pane.cache.display_offset > 0 && card.width() > 150.0 && card.height() > 80.0 {
        let chip = Rect::from_min_size(card.right_bottom() - vec2(140.0, 42.0), vec2(128.0, 28.0));
        let response = ui.interact(chip, ui.id().with(("scroll-bottom", id)), Sense::click());
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, "Back to bottom"));
        capsule(&painter, chip, p);
        let ink = if response.hovered() {
            p.fg
        } else {
            p.secondary
        };
        icons::paint(
            &painter,
            Rect::from_center_size(
                Pos2::new(chip.left() + 17.0, chip.center().y),
                Vec2::splat(12.0),
            ),
            Icon::ArrowDown,
            ink,
        );
        painter.text(
            Pos2::new(chip.left() + 29.0, chip.center().y),
            Align2::LEFT_CENTER,
            "Back to bottom",
            theme::medium(11.5),
            ink,
        );
        if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            actions.push(Action::ScrollBottom(id));
        }
    }
    if selected {
        output.active_body = Some(body);
    }
}

pub fn draw_node(
    ui: &mut Ui,
    node: &pace_model::Layout,
    rect: Rect,
    panes: &mut BTreeMap<PaneId, PaneRender>,
    stage: &Stage,
    actions: &mut Vec<Action>,
    output: &mut StageOutput,
) {
    match node {
        pace_model::Layout::Leaf(id) => {
            if let Some(pane) = panes.get_mut(id) {
                draw_pane(ui, *id, rect, pane, stage, actions, output);
            }
        }
        pace_model::Layout::Split {
            id: split,
            axis,
            ratio,
            first,
            second,
        } => {
            let vertical = *axis == Axis::Vertical;
            let length = if vertical {
                rect.width()
            } else {
                rect.height()
            };
            let min = if vertical { 200.0 } else { 130.0 };
            let low = (min / length).min(0.45);
            let cut = length * ratio.clamp(low, 1.0 - low);
            let half = metrics::GUTTER * 0.5;
            let (a, b, gap) = if vertical {
                (
                    Rect::from_min_max(
                        rect.min,
                        Pos2::new(rect.left() + cut - half, rect.bottom()),
                    ),
                    Rect::from_min_max(Pos2::new(rect.left() + cut + half, rect.top()), rect.max),
                    Rect::from_min_max(
                        Pos2::new(rect.left() + cut - half, rect.top()),
                        Pos2::new(rect.left() + cut + half, rect.bottom()),
                    ),
                )
            } else {
                (
                    Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.top() + cut - half)),
                    Rect::from_min_max(Pos2::new(rect.left(), rect.top() + cut + half), rect.max),
                    Rect::from_min_max(
                        Pos2::new(rect.left(), rect.top() + cut - half),
                        Pos2::new(rect.right(), rect.top() + cut + half),
                    ),
                )
            };
            // A slightly wider grab area than the visible gutter.
            let grab = if vertical {
                gap.expand2(vec2(2.0, 0.0))
            } else {
                gap.expand2(vec2(0.0, 2.0))
            };
            // Click sensing is what reports the double-click that evens a split.
            let response = ui
                .interact(
                    grab,
                    ui.id().with(("divider", split.get())),
                    Sense::click_and_drag(),
                )
                .on_hover_and_drag_cursor(if vertical {
                    CursorIcon::ResizeHorizontal
                } else {
                    CursorIcon::ResizeVertical
                });
            if response.dragged()
                && let Some(pos) = response.interact_pointer_pos()
            {
                let ratio = if vertical {
                    (pos.x - rect.left()) / length
                } else {
                    (pos.y - rect.top()) / length
                };
                actions.push(Action::Ratio(*split, ratio.clamp(0.1, 0.9)));
            }
            if response.double_clicked() || response.triple_clicked() {
                actions.push(Action::Ratio(*split, 0.5));
            }
            let grip = animate(
                ui.ctx(),
                response.id.with("grip"),
                response.hovered() || response.dragged(),
                0.12,
            );
            if grip > 0.0 {
                let size = if vertical {
                    vec2(3.0, 40.0f32.min(gap.height() - 16.0))
                } else {
                    vec2(40.0f32.min(gap.width() - 16.0), 3.0)
                };
                ui.painter().rect_filled(
                    Rect::from_center_size(gap.center(), size.max(Vec2::ZERO)),
                    2,
                    theme::tint(
                        if response.dragged() {
                            stage.p.accent
                        } else {
                            stage.p.muted
                        },
                        grip,
                    ),
                );
            }
            draw_node(ui, first, a, panes, stage, actions, output);
            draw_node(ui, second, b, panes, stage, actions, output);
        }
    }
}

/// A calm placeholder for a window without a workspace.
pub fn empty_state(
    ui: &mut Ui,
    rect: Rect,
    p: Palette,
    restoring: bool,
    actions: &mut Vec<Action>,
) {
    ui.painter().rect_filled(rect, metrics::PANE_RADIUS, p.bg);
    ui.painter().rect_stroke(
        rect,
        metrics::PANE_RADIUS,
        Stroke::new(1.0, p.separator),
        StrokeKind::Inside,
    );
    let center = rect.center();
    if restoring {
        egui::Spinner::new().size(16.0).color(p.muted).paint_at(
            ui,
            Rect::from_center_size(center - vec2(0.0, 16.0), Vec2::splat(16.0)),
        );
        ui.painter().text(
            center + vec2(0.0, 12.0),
            Align2::CENTER_CENTER,
            "Restoring workspaces…",
            theme::regular(13.0),
            p.secondary,
        );
        return;
    }
    let tile = Rect::from_center_size(center - vec2(0.0, 58.0), Vec2::splat(52.0));
    ui.painter()
        .rect_filled(tile, 14, theme::tint(p.accent, 0.16));
    icons::paint(ui.painter(), tile.shrink(14.0), Icon::Terminal, p.accent);
    ui.painter().text(
        center - vec2(0.0, 8.0),
        Align2::CENTER_CENTER,
        "No open workspaces",
        theme::semibold(15.0),
        p.fg,
    );
    ui.painter().text(
        center + vec2(0.0, 14.0),
        Align2::CENTER_CENTER,
        "Start a shell in your home directory.",
        theme::regular(12.5),
        p.secondary,
    );
    let clicked = ui
        .scope_builder(
            UiBuilder::new()
                .id_salt("empty-state")
                .max_rect(Rect::from_center_size(
                    center + vec2(0.0, 56.0),
                    vec2(160.0, metrics::CONTROL_HEIGHT),
                ))
                .layout(Layout::top_down(Align::Center)),
            |ui| helpers::button(ui, p, "New workspace", helpers::ButtonKind::Primary).clicked(),
        )
        .inner;
    if clicked {
        actions.push(Action::New);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata(title: &str) -> SessionMetadata {
        SessionMetadata {
            title: title.into(),
            shell: "/usr/bin/zsh".into(),
            cwd: "/tmp".into(),
            process_id: None,
            status: SessionStatus::Running,
            bell_count: 0,
        }
    }

    #[test]
    fn prompt_style_titles_fall_back_to_the_shell_name() {
        assert_eq!(pane_label(&metadata("")), "zsh");
        assert_eq!(pane_label(&metadata("me@host:~/code")), "zsh");
        assert_eq!(pane_label(&metadata("host:/srv")), "zsh");
        assert_eq!(pane_label(&metadata("nvim README.md")), "nvim README.md");
    }

    #[test]
    fn exit_states_are_described_without_alarm_for_success() {
        assert_eq!(status_text(&SessionStatus::Running), None);
        assert_eq!(
            status_text(&SessionStatus::Exited {
                code: 0,
                signal: None
            }),
            Some(("Process exited".into(), false))
        );
        assert_eq!(
            status_text(&SessionStatus::Exited {
                code: 2,
                signal: None
            }),
            Some(("Exited with code 2".into(), true))
        );
        assert_eq!(
            status_text(&SessionStatus::Error("missing".into())),
            Some(("Could not start shell".into(), true))
        );
    }
}

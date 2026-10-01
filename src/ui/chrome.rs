//! Window chrome: the full-height sidebar, the toolbar and window controls.
use super::helpers::{
    self, animate, edit_shortcut, elided, galley_at, keycaps, menu_item, menu_layout,
    menu_separator, shortcut,
};
use super::{Action, UiState, WorkspaceView};
use crate::{
    icons::{self, Icon},
    platform::window::{self, WindowOperation},
    theme::{self, Palette, metrics},
};
use eframe::egui::{
    self, Align, Align2, Color32, CursorIcon, Layout, Pos2, Rect, Sense, Stroke, Ui, UiBuilder,
    Vec2, WidgetInfo, WidgetType, vec2,
};
use pace_model::{Destination, PaneId, WorkspaceId};

/// What the chrome needs to know about the frame it surrounds.
pub struct ChromeView<'a> {
    pub workspaces: &'a [WorkspaceView],
    pub active: Option<WorkspaceId>,
    pub pane: Option<PaneId>,
    /// The focused terminal's label and directory.
    pub subtitle: &'a str,
    pub zoomed: bool,
    /// The whole window. The window controls keep their place in it.
    pub window: Rect,
    /// How much of the sidebar is shown: 0 hidden, 1 shown, and between the
    /// two while a toggle slides it.
    pub sidebar: f32,
    /// The sidebar is shown once it has settled.
    pub sidebar_open: bool,
    /// The sidebar's full width.
    pub sidebar_width: f32,
    /// The window is wide enough to show a sidebar at all.
    pub sidebar_available: bool,
    /// A terminal of the active workspace is being carried by its header.
    pub pane_drag: Option<PaneId>,
}

const SIDEBAR_WIDTH: std::ops::RangeInclusive<f32> = 170.0..=360.0;
const DEFAULT_SIDEBAR_WIDTH: f32 = 216.0;
/// Seconds a toggled sidebar takes to slide in or out.
const SIDEBAR_SLIDE: f64 = 0.16;

/// A sidebar toggle in motion. It starts from however much of the sidebar was
/// showing, so a toggle reversed midway turns around without a jump.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SidebarSlide {
    from: f32,
    started: f64,
}

impl SidebarSlide {
    /// Begins the slide that follows a toggle. `shown` is the state being left.
    pub fn toggled(previous: Option<Self>, shown: bool, now: f64) -> Self {
        Self {
            from: previous
                .and_then(|slide| slide.reveal(shown, now))
                .unwrap_or(if shown { 1.0 } else { 0.0 }),
            started: now,
        }
    }

    /// How much of the sidebar is shown at `now`, or `None` once it has settled.
    pub fn reveal(self, shown: bool, now: f64) -> Option<f32> {
        let progress = ((now - self.started) / SIDEBAR_SLIDE) as f32;
        (progress < 1.0).then(|| {
            egui::lerp(
                self.from..=if shown { 1.0 } else { 0.0 },
                egui::emath::easing::cubic_out(progress.max(0.0)),
            )
        })
    }
}

/// Centre of the sidebar toggle from the window's leading edge: beside the
/// window controls while the sidebar is hidden, at its trailing edge while shown.
fn toggle_offset(view: &ChromeView) -> f32 {
    egui::lerp(94.0..=view.sidebar_width - 22.0, view.sidebar)
}

fn drag_region(ui: &mut Ui, rect: Rect, name: &str) {
    let drag = ui.interact(rect, ui.id().with(name), Sense::click_and_drag());
    if drag.drag_started() {
        window::send(ui.ctx(), WindowOperation::StartDrag);
    }
    if drag.double_clicked() {
        toggle_maximized(ui.ctx());
    }
}

fn toggle_maximized(ctx: &egui::Context) {
    let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
    window::send(ctx, WindowOperation::SetMaximized(!maximized));
}

/// Close, minimize and maximize as three quiet lights. Their glyphs appear when
/// the pointer approaches, and they fade to neutral in an inactive window.
pub fn window_controls(ui: &mut Ui, left_center: Pos2, p: Palette, actions: &mut Vec<Action>) {
    let group = Rect::from_min_max(
        left_center + vec2(-6.0, -12.0),
        left_center + vec2(58.0, 12.0),
    );
    let hovered = ui.rect_contains_pointer(group);
    let active = ui.input(|input| input.focused);
    let reveal = animate(ui.ctx(), ui.id().with("window-controls"), hovered, 0.12);
    for (index, (label, rgb)) in [
        ("Close window", 0xff5f57),
        ("Minimize", 0xfebc2e),
        ("Maximize", 0x28c840),
    ]
    .into_iter()
    .enumerate()
    {
        let center = left_center + vec2(6.0 + index as f32 * 20.0, 0.0);
        let response = ui.interact(
            Rect::from_center_size(center, Vec2::splat(20.0)),
            ui.id().with(("window-control", label)),
            Sense::click(),
        );
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
        let color = theme::color(rgb);
        let painter = ui.painter();
        let fill = if active || hovered {
            color
        } else {
            theme::mix(p.chrome, p.muted, 0.55)
        };
        painter.circle_filled(
            center,
            6.0,
            if response.is_pointer_button_down_on() {
                theme::mix(fill, Color32::BLACK, 0.2)
            } else {
                fill
            },
        );
        painter.circle_stroke(center, 6.0, Stroke::new(0.5, Color32::from_black_alpha(48)));
        if response.has_focus() {
            painter.circle_stroke(center, 8.5, Stroke::new(1.5, theme::tint(p.accent, 0.8)));
        }
        if reveal > 0.0 {
            let ink = Stroke::new(
                1.2,
                theme::tint(theme::mix(color, Color32::BLACK, 0.62), reveal),
            );
            match index {
                0 => {
                    for flip in [1.0, -1.0] {
                        painter.line_segment(
                            [
                                center + vec2(-2.4, -2.4 * flip),
                                center + vec2(2.4, 2.4 * flip),
                            ],
                            ink,
                        );
                    }
                }
                1 => {
                    painter.line_segment([center - vec2(3.0, 0.0), center + vec2(3.0, 0.0)], ink);
                }
                _ => {
                    painter.line_segment([center - vec2(3.0, 0.0), center + vec2(3.0, 0.0)], ink);
                    painter.line_segment([center - vec2(0.0, 3.0), center + vec2(0.0, 3.0)], ink);
                }
            }
        }
        if response.clicked() {
            match index {
                0 => actions.push(Action::WindowClose),
                1 => window::send(ui.ctx(), WindowOperation::Minimize),
                _ => toggle_maximized(ui.ctx()),
            }
        }
        response.on_hover_text(label);
    }
}

fn icon_at(
    ui: &mut Ui,
    center: Pos2,
    icon: Icon,
    label: &str,
    hint: &str,
    salt: &str,
) -> egui::Response {
    ui.scope_builder(
        UiBuilder::new()
            .id_salt(salt)
            .max_rect(Rect::from_center_size(center, Vec2::splat(28.0))),
        |ui| icons::button_with_hint(ui, icon, label, hint),
    )
    .inner
}

pub fn toolbar(
    ui: &mut Ui,
    rect: Rect,
    p: Palette,
    view: &ChromeView,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    drag_region(ui, rect, "title-drag");
    let middle = rect.center().y;
    // The title starts clear of the leading controls wherever they overlap the
    // toolbar: always while the sidebar is hidden, and in passing as it slides.
    let leading = if view.sidebar_available {
        toggle_offset(view) + 24.0
    } else {
        78.0
    };
    let left = (rect.left() + 8.0).max(view.window.left() + leading);

    // The command field stays centred on the content; narrow windows fall back
    // to an icon so the title keeps its room. A sliding sidebar decides this
    // from the width it is heading for, so the field does not swap midway.
    let field_width = (rect.width() * 0.32).clamp(210.0, 320.0);
    let settled_width = view.window.width()
        - if view.sidebar_open {
            view.sidebar_width
        } else {
            0.0
        };
    let show_field = settled_width >= 760.0;
    let field = Rect::from_center_size(rect.center(), vec2(field_width, 28.0));

    let mut right = rect.right() - 8.0;
    // "New workspace" moves here while the sidebar is away. It fades in from
    // the trailing edge so its neighbours make room gradually.
    let cluster = ui
        .scope_builder(
            UiBuilder::new()
                .id_salt("toolbar-actions")
                .max_rect(Rect::from_min_max(
                    Pos2::new(left, rect.top() + 8.0),
                    Pos2::new(
                        right
                            + if view.sidebar < 1.0 {
                                30.0 * view.sidebar
                            } else {
                                0.0
                            },
                        rect.bottom() - 8.0,
                    ),
                ))
                .layout(Layout::right_to_left(Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                if view.sidebar < 1.0 {
                    ui.set_opacity(1.0 - view.sidebar);
                    let create =
                        icons::button_with_hint(ui, Icon::Plus, "New workspace", &shortcut("T"));
                    ui.set_opacity(1.0);
                    if create.clicked() {
                        actions.push(Action::New);
                    }
                }
                if let Some(pane) = view.pane {
                    if icons::button_with_hint(
                        ui,
                        Icon::SplitHorizontal,
                        "Split below",
                        &shortcut("E"),
                    )
                    .clicked()
                    {
                        actions.push(Action::Split(pane, pace_model::Axis::Horizontal));
                    }
                    if icons::button_with_hint(
                        ui,
                        Icon::SplitVertical,
                        "Split right",
                        &shortcut("D"),
                    )
                    .clicked()
                    {
                        actions.push(Action::Split(pane, pace_model::Axis::Vertical));
                    }
                    if icons::button_with_hint(ui, Icon::Search, "Find in terminal", &shortcut("F"))
                        .clicked()
                    {
                        actions.push(Action::Find);
                    }
                }
                if !show_field
                    && icons::button_with_hint(ui, Icon::Command, "Command palette", &shortcut("P"))
                        .clicked()
                {
                    actions.push(Action::Palette);
                }
                ui.min_rect().left()
            },
        )
        .inner;
    right = cluster - 6.0;

    if view.zoomed {
        let chip = Rect::from_min_max(
            Pos2::new(right - 84.0, middle - 11.0),
            Pos2::new(right, middle + 11.0),
        );
        if chip.left() > left + 40.0 {
            let response = ui.interact(chip, ui.id().with("toolbar-zoom"), Sense::click());
            response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, "Exit zoom"));
            let painter = ui.painter();
            painter.rect_filled(
                chip,
                11,
                theme::tint(p.accent, if response.hovered() { 0.3 } else { 0.2 }),
            );
            icons::paint(
                painter,
                Rect::from_center_size(Pos2::new(chip.left() + 15.0, middle), Vec2::splat(12.0)),
                Icon::Minimize,
                p.accent,
            );
            painter.text(
                Pos2::new(chip.left() + 26.0, middle),
                Align2::LEFT_CENTER,
                "Zoomed",
                theme::medium(11.5),
                p.accent,
            );
            if response
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text(format!("Show all terminals   {}", shortcut("Enter")))
                .clicked()
            {
                actions.push(Action::Zoom);
            }
            right = chip.left() - 8.0;
        }
    }

    if state.search_open && view.pane.is_some() {
        // Search takes the command field's place. In a narrow window it uses
        // the title's room instead, so its controls stay reachable.
        let field = if show_field {
            let width = (rect.width() * 0.42).clamp(300.0, 440.0);
            let centre = rect.center().x.min(right - 8.0 - width * 0.5);
            Rect::from_center_size(Pos2::new(centre, middle), vec2(width, 30.0))
        } else {
            Rect::from_min_max(
                Pos2::new(left + 6.0, middle - 15.0),
                Pos2::new(right - 4.0, middle + 15.0),
            )
        };
        if field.width() >= 150.0 {
            super::search::show(ui, field, p, state, actions);
        }
        if !show_field {
            return;
        }
        right = right.min(field.left() - 14.0);
    } else if show_field {
        let response = ui.interact(field, ui.id().with("toolbar-commands"), Sense::click());
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, "Command palette"));
        let painter = ui.painter();
        painter.rect_filled(
            field,
            metrics::CONTROL_RADIUS,
            if response.is_pointer_button_down_on() {
                p.pressed
            } else if response.hovered() {
                p.hover
            } else {
                p.control
            },
        );
        if response.has_focus() {
            painter.rect_stroke(
                field,
                metrics::CONTROL_RADIUS,
                Stroke::new(1.5, p.accent),
                egui::StrokeKind::Inside,
            );
        }
        icons::paint(
            painter,
            Rect::from_center_size(Pos2::new(field.left() + 16.0, middle), Vec2::splat(13.0)),
            Icon::Search,
            p.muted,
        );
        let caps = keycaps(
            painter,
            Pos2::new(field.right() - 6.0, middle),
            &shortcut("P"),
            p.muted,
        );
        galley_at(
            painter,
            Pos2::new(field.left() + 30.0, middle),
            elided(
                painter,
                "Search commands",
                theme::regular(12.5),
                p.muted,
                field.width() - 44.0 - caps,
            ),
        );
        if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            actions.push(Action::Palette);
        }
        right = right.min(field.left() - 14.0);
    }

    let Some(workspace) = view.workspaces.iter().find(|w| Some(w.id) == view.active) else {
        return;
    };
    let budget = right - left - 6.0;
    if budget < 36.0 {
        return;
    }
    let painter = ui.painter().with_clip_rect(Rect::from_min_max(
        Pos2::new(left, rect.top()),
        Pos2::new(right, rect.bottom()),
    ));
    let name = elided(&painter, &workspace.name, theme::medium(13.0), p.fg, budget);
    let name_width = name.size().x;
    galley_at(&painter, Pos2::new(left + 6.0, middle), name);
    let remaining = budget - name_width - 12.0;
    if remaining > 48.0 && !view.subtitle.is_empty() {
        galley_at(
            &painter,
            Pos2::new(left + 6.0 + name_width + 10.0, middle + 0.5),
            elided(
                &painter,
                view.subtitle,
                theme::regular(12.0),
                p.secondary,
                remaining,
            ),
        );
    }
}

fn workspace_menu(ui: &mut Ui, p: Palette, workspace: &WorkspaceView, actions: &mut Vec<Action>) {
    menu_layout(ui, 210.0);
    if menu_item(ui, p, Icon::Pencil, "Rename…", "", false) {
        actions.push(Action::Rename(workspace.id));
        ui.close();
    }
    if workspace.remote.is_some() {
        if menu_item(ui, p, Icon::Globe, "Disconnect from SSH", "", false) {
            actions.push(Action::Disconnect(workspace.id));
            ui.close();
        }
    } else if menu_item(ui, p, Icon::Globe, "Connect over SSH…", "", false) {
        actions.push(Action::Ssh(Some(workspace.id)));
        ui.close();
    }
    menu_separator(ui, p);
    if menu_item(ui, p, Icon::Close, "Close workspace", "", true) {
        actions.push(Action::CloseWorkspace(workspace.id));
        ui.close();
    }
}

fn initial(name: &str) -> String {
    name.chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_else(|| "·".into())
}

fn workspace_row(
    ui: &mut Ui,
    p: Palette,
    workspace: &WorkspaceView,
    selected: bool,
    drag: Option<PaneId>,
    // The carried terminal's session can run in this workspace.
    accepts: bool,
    actions: &mut Vec<Action>,
) {
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::hover());
    let response = ui.interact(
        row,
        ui.id().with(("workspace", workspace.id.get())),
        Sense::click(),
    );
    let more_id = ui.id().with(("workspace-more", workspace.id.get()));
    let menu_open =
        egui::Popup::is_id_open(ui.ctx(), more_id.with("popup")) || response.context_menu_opened();
    let hovered = ui.rect_contains_pointer(row) || menu_open;
    let painter = ui.painter().clone();
    if selected {
        painter.rect_filled(row, metrics::ROW_RADIUS, p.pressed);
    } else if hovered {
        painter.rect_filled(row, metrics::ROW_RADIUS, p.hover);
    }
    if response.has_focus() {
        painter.rect_stroke(
            row,
            metrics::ROW_RADIUS,
            Stroke::new(1.5, p.accent),
            egui::StrokeKind::Inside,
        );
    }
    // A carried terminal can be dropped on any workspace but its own, as
    // long as that workspace is on the same machine.
    let receiving = drag.filter(|_| !selected && accepts && ui.rect_contains_pointer(row));
    let receive = animate(
        ui.ctx(),
        ui.id().with(("workspace-drop", workspace.id.get())),
        receiving.is_some(),
        0.12,
    );
    if receive > 0.0 {
        painter.rect_filled(
            row,
            metrics::ROW_RADIUS,
            theme::tint(p.accent, 0.2 * receive),
        );
        painter.rect_stroke(
            row,
            metrics::ROW_RADIUS,
            Stroke::new(1.5, theme::tint(p.accent, 0.9 * receive)),
            egui::StrokeKind::Inside,
        );
    }
    if let Some(pane) = receiving
        && ui.input(|input| input.pointer.primary_released())
    {
        actions.push(Action::MovePane(pane, Destination::Workspace(workspace.id)));
    }

    let identity = theme::identity_color(workspace.id.get(), p.dark);
    let tile = Rect::from_center_size(
        Pos2::new(row.left() + 23.0, row.center().y),
        Vec2::splat(28.0),
    );
    let (tile_fill, tile_ink) = if selected {
        (
            identity,
            if identity.r() as u32 + identity.g() as u32 > 420 {
                theme::color(0x1d1d1f)
            } else {
                Color32::WHITE
            },
        )
    } else {
        (
            theme::tint(identity, if p.dark { 0.2 } else { 0.16 }),
            if p.dark {
                identity
            } else {
                theme::mix(identity, Color32::BLACK, 0.18)
            },
        )
    };
    painter.rect_filled(tile, 8, tile_fill);
    painter.text(
        tile.center(),
        Align2::CENTER_CENTER,
        initial(&workspace.name),
        theme::semibold(12.5),
        tile_ink,
    );
    if !workspace.running {
        // Every terminal here has stopped.
        let badge = tile.right_top() + vec2(-1.0, 1.0);
        painter.circle_filled(badge, 4.5, p.chrome);
        painter.circle_filled(badge, 3.0, p.red);
    }

    let text_left = tile.right() + 10.0;
    let trailing = 30.0;
    let text_width = row.right() - text_left - trailing;
    galley_at(
        &painter,
        Pos2::new(text_left, row.center().y - 8.0),
        elided(
            &painter,
            &workspace.name,
            theme::medium(13.0),
            if selected || hovered {
                p.fg
            } else {
                theme::mix(p.secondary, p.fg, 0.45)
            },
            text_width,
        ),
    );
    let detail = Pos2::new(text_left, row.center().y + 9.0);
    if let Some(destination) = &workspace.remote {
        // A remote workspace shows its host where a local one shows its folder.
        icons::paint(
            &painter,
            Rect::from_center_size(detail + vec2(5.5, 0.0), Vec2::splat(11.0)),
            Icon::Globe,
            p.muted,
        );
        galley_at(
            &painter,
            detail + vec2(15.0, 0.0),
            elided(
                &painter,
                destination,
                theme::regular(11.0),
                p.muted,
                text_width - 15.0,
            ),
        );
    } else {
        painter.text(
            detail,
            Align2::LEFT_CENTER,
            helpers::path_label(&workspace.cwd, (text_width / 5.9).max(4.0) as usize),
            theme::regular(11.0),
            p.muted,
        );
    }

    response.widget_info(|| {
        WidgetInfo::selected(WidgetType::SelectableLabel, true, selected, &workspace.name)
    });
    if response.clicked() {
        actions.push(Action::SelectWorkspace(workspace.id));
    }
    if response.double_clicked() {
        actions.push(Action::Rename(workspace.id));
    }
    response.context_menu(|ui| workspace_menu(ui, p, workspace, actions));

    let more = Rect::from_center_size(
        Pos2::new(row.right() - 17.0, row.center().y),
        Vec2::splat(24.0),
    );
    let more_response = ui.interact(more, more_id, Sense::click());
    more_response.widget_info(|| {
        WidgetInfo::labeled(
            WidgetType::Button,
            true,
            format!("Actions for {}", workspace.name),
        )
    });
    // While a terminal is carried the row is a destination, not a control.
    if (hovered && drag.is_none()) || more_response.has_focus() {
        if more_response.hovered() || menu_open {
            painter.rect_filled(more, 6, p.pressed);
        }
        icons::paint(
            &painter,
            more.shrink(5.0),
            Icon::Ellipsis,
            if more_response.hovered() {
                p.fg
            } else {
                p.secondary
            },
        );
    } else if workspace.panes > 1 {
        painter.text(
            more.center(),
            Align2::CENTER_CENTER,
            workspace.panes.to_string(),
            theme::medium(11.0),
            p.muted,
        );
    }
    egui::Popup::menu(&more_response).show(|ui| workspace_menu(ui, p, workspace, actions));
}

/// The window controls and the sidebar toggle, drawn over the sidebar and the
/// toolbar. The lights never move; the toggle travels with the sidebar's edge
/// between its place there and its place beside the lights.
pub fn leading_controls(ui: &mut Ui, p: Palette, view: &ChromeView, actions: &mut Vec<Action>) {
    let origin = view.window.min;
    let middle = origin.y + metrics::TOOLBAR_HEIGHT * 0.5;
    window_controls(ui, Pos2::new(origin.x + 16.0, middle), p, actions);
    if view.sidebar_available
        && icon_at(
            ui,
            Pos2::new(origin.x + toggle_offset(view).round(), middle),
            Icon::Sidebar,
            "Toggle sidebar",
            &shortcut("B"),
            "sidebar-toggle",
        )
        .clicked()
    {
        actions.push(Action::ToggleSidebar);
    }
}

/// The sidebar spans the full window height, like a native source list.
/// `drag` holds the width while its edge is being dragged. While a toggle
/// slides it, `rect` moves past the window's leading edge.
pub fn sidebar(
    ui: &mut Ui,
    rect: Rect,
    p: Palette,
    view: &ChromeView,
    drag: &mut Option<f32>,
    actions: &mut Vec<Action>,
) {
    let strip = Rect::from_min_size(rect.min, vec2(rect.width(), metrics::TOOLBAR_HEIGHT));
    drag_region(ui, strip, "sidebar-drag");

    ui.painter().text(
        Pos2::new(rect.left() + 18.0, strip.bottom() + 14.0),
        Align2::LEFT_CENTER,
        "Workspaces",
        theme::medium(11.5),
        p.muted,
    );

    let footer_top = rect.bottom() - 50.0;
    ui.scope_builder(
        UiBuilder::new()
            .id_salt("workspace-list")
            .max_rect(Rect::from_min_max(
                Pos2::new(rect.left() + 8.0, strip.bottom() + 30.0),
                Pos2::new(rect.right() - 8.0, footer_top - 4.0),
            )),
        |ui| {
            egui::ScrollArea::vertical()
                .id_salt("workspaces")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    let machine = view
                        .workspaces
                        .iter()
                        .find(|workspace| Some(workspace.id) == view.active)
                        .map(|workspace| &workspace.remote);
                    for workspace in view.workspaces {
                        workspace_row(
                            ui,
                            p,
                            workspace,
                            Some(workspace.id) == view.active,
                            view.pane_drag,
                            Some(&workspace.remote) == machine,
                            actions,
                        );
                    }
                });
        },
    );

    // Footer: the primary creation action, with preferences beside it.
    let create = Rect::from_min_max(
        Pos2::new(rect.left() + 8.0, footer_top + 8.0),
        Pos2::new(rect.right() - 44.0, footer_top + 42.0),
    );
    let response = ui.interact(create, ui.id().with("workspace-create"), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, "New workspace"));
    let painter = ui.painter();
    if response.is_pointer_button_down_on() {
        painter.rect_filled(create, metrics::ROW_RADIUS, p.pressed);
    } else if response.hovered() {
        painter.rect_filled(create, metrics::ROW_RADIUS, p.hover);
    }
    if response.has_focus() {
        painter.rect_stroke(
            create,
            metrics::ROW_RADIUS,
            Stroke::new(1.5, p.accent),
            egui::StrokeKind::Inside,
        );
    }
    let ink = if response.hovered() {
        p.fg
    } else {
        p.secondary
    };
    icons::paint(
        painter,
        Rect::from_center_size(
            Pos2::new(create.left() + 15.0, create.center().y),
            Vec2::splat(14.0),
        ),
        Icon::Plus,
        ink,
    );
    galley_at(
        painter,
        Pos2::new(create.left() + 31.0, create.center().y),
        elided(
            painter,
            "New workspace",
            theme::medium(12.5),
            ink,
            create.width() - 36.0,
        ),
    );
    if response
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(format!("New workspace   {}", shortcut("T")))
        .clicked()
    {
        actions.push(Action::New);
    }
    if icon_at(
        ui,
        Pos2::new(rect.right() - 24.0, create.center().y),
        Icon::Settings,
        "Preferences",
        &edit_shortcut(","),
        "sidebar-preferences",
    )
    .clicked()
    {
        actions.push(Action::Settings);
    }

    // The trailing edge resizes the sidebar once it rests there; the width is
    // saved on release.
    if view.sidebar < 1.0 {
        return;
    }
    let handle = Rect::from_min_max(
        Pos2::new(rect.right() - 3.0, strip.bottom()),
        Pos2::new(rect.right() + 3.0, rect.bottom() - 12.0),
    );
    let resize = ui
        .interact(
            handle,
            ui.id().with("sidebar-resize"),
            Sense::click_and_drag(),
        )
        .on_hover_and_drag_cursor(CursorIcon::ResizeHorizontal);
    resize.widget_info(|| WidgetInfo::labeled(WidgetType::ResizeHandle, true, "Resize sidebar"));
    if resize.dragged()
        && let Some(pointer) = resize.interact_pointer_pos()
    {
        *drag = Some((pointer.x - rect.left()).clamp(*SIDEBAR_WIDTH.start(), *SIDEBAR_WIDTH.end()));
    }
    if resize.drag_stopped()
        && let Some(width) = drag.take()
    {
        actions.push(Action::SidebarWidth(width));
    }
    // A third quick click still means "reset", not a new gesture.
    if resize.double_clicked() || resize.triple_clicked() {
        *drag = None;
        actions.push(Action::SidebarWidth(DEFAULT_SIDEBAR_WIDTH));
    }
    let grip = animate(
        ui.ctx(),
        resize.id.with("grip"),
        resize.hovered() || resize.dragged(),
        0.12,
    );
    if grip > 0.0 {
        ui.painter().line_segment(
            [
                Pos2::new(rect.right(), strip.bottom() + 8.0),
                Pos2::new(rect.right(), rect.bottom() - 20.0),
            ],
            Stroke::new(
                2.0,
                theme::tint(
                    if resize.dragged() { p.accent } else { p.muted },
                    grip * 0.8,
                ),
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_toggled_sidebar_eases_to_rest_and_reverses_from_where_it_is() {
        let hide = SidebarSlide::toggled(None, true, 10.0);
        assert_eq!(hide.reveal(false, 10.0), Some(1.0));
        let midway = hide.reveal(false, 10.08).unwrap();
        assert!(
            midway > 0.0 && midway < 0.5,
            "ease-out covers most of the way early: {midway}"
        );
        assert_eq!(hide.reveal(false, 10.0 + SIDEBAR_SLIDE), None);

        // Toggled back midway: the slide continues from where the sidebar is.
        let show = SidebarSlide::toggled(Some(hide), false, 10.08);
        assert_eq!(show.reveal(true, 10.08), Some(midway));
        assert!(show.reveal(true, 10.12).unwrap() > midway);
        assert_eq!(show.reveal(true, 10.08 + SIDEBAR_SLIDE), None);

        // A finished slide leaves nothing to continue from.
        assert_eq!(
            SidebarSlide::toggled(Some(hide), false, 11.0).reveal(true, 11.0),
            Some(0.0)
        );
    }

    #[test]
    fn a_carried_terminal_drops_on_another_workspace_row_but_not_its_own() {
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::platform::fonts::bundled_definitions());
        let config = crate::config::Config::default();
        theme::apply(&ctx, &config);
        let workspaces: Vec<WorkspaceView> = [(1, None), (2, None), (3, Some("me@devbox"))]
            .into_iter()
            .map(|(id, remote): (u64, Option<&str>)| WorkspaceView {
                id: WorkspaceId::new(id),
                name: format!("workspace {id}"),
                cwd: "/srv/app".into(),
                remote: remote.map(str::to_owned),
                panes: 2,
                running: true,
            })
            .collect();
        let pane = PaneId::new(7);
        let frame = |drag: Option<PaneId>, events: Vec<egui::Event>| {
            let mut actions = Vec::new();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 600.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    sidebar(
                        ui,
                        Rect::from_min_size(Pos2::ZERO, vec2(216.0, 600.0)),
                        Palette::for_config(&config),
                        &ChromeView {
                            workspaces: &workspaces,
                            active: Some(WorkspaceId::new(1)),
                            pane: Some(pane),
                            subtitle: "",
                            zoomed: false,
                            window: Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 600.0)),
                            sidebar: 1.0,
                            sidebar_open: true,
                            sidebar_width: 216.0,
                            sidebar_available: true,
                            pane_drag: drag,
                        },
                        &mut None,
                        &mut actions,
                    );
                },
            );
            output.textures_delta.clear();
            actions
        };
        // Rows are 46 points tall, two points apart, below the list heading.
        let own = Pos2::new(100.0, metrics::TOOLBAR_HEIGHT + 30.0 + 23.0);
        let other = own + vec2(0.0, 48.0);
        let on_a_host = other + vec2(0.0, 48.0);
        let release = |pos| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        };
        let press = |pos| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        };
        frame(None, vec![]);
        for (pos, drag, expected) in [
            (other, Some(pane), Some(WorkspaceId::new(2))),
            (own, Some(pane), None),
            // Its session is local and would not follow it to another machine.
            (on_a_host, Some(pane), None),
            // Without a carried terminal a release is an ordinary click.
            (other, None, None),
        ] {
            frame(drag, vec![egui::Event::PointerMoved(pos), press(pos)]);
            let moved: Vec<_> = frame(drag, vec![release(pos)])
                .into_iter()
                .filter_map(|action| match action {
                    Action::MovePane(pane, Destination::Workspace(workspace)) => {
                        Some((pane, workspace))
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(
                moved,
                expected
                    .map(|workspace| (pane, workspace))
                    .into_iter()
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn workspace_tiles_use_a_visible_initial() {
        assert_eq!(initial("sandbox"), "S");
        assert_eq!(initial("  ~/日本語"), "日");
        assert_eq!(initial("ßeta"), "SS");
        assert_eq!(initial("---"), "·");
    }
}

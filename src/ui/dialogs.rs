//! Transient dialogs edit only UI drafts and emit targeted commands.
use super::helpers::{
    ButtonKind, SheetPlacement, button, padded, place, sheet, sheet_header, text_field, toast,
};
use super::{Action, Close, OverlayState, UiState};
use crate::theme::{self, Palette};
use eframe::egui::{self, Align, Id, Layout, Ui, vec2};

/// A sheet measures itself in a hidden first pass, where focus cannot be held.
fn accepts_focus(ui: &Ui) -> bool {
    ui.is_enabled() && !ui.is_sizing_pass()
}

/// Enter confirms a dialog only as a fresh press after the dialog has taken
/// the keyboard. The press that opened it, and a held key repeating from the
/// previous surface, must not confirm something the user has not seen.
fn confirmed_by_enter(ctx: &egui::Context, state: &UiState) -> bool {
    !state.overlay_focus
        && ctx.input(|input| {
            input.events.iter().any(|event| {
                matches!(
                    event,
                    egui::Event::Key {
                        key: egui::Key::Enter,
                        pressed: true,
                        repeat: false,
                        ..
                    }
                )
            })
        })
}

/// Trailing actions of a sheet: the confirming action sits at the far edge.
fn footer(ui: &mut Ui, salt: &str, add_buttons: impl FnOnce(&mut Ui)) {
    let (_, rect) = ui.allocate_space(vec2(ui.available_width(), 62.0));
    place(
        ui,
        rect.shrink2(vec2(20.0, 0.0)),
        Layout::right_to_left(Align::Center),
        salt,
        |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            add_buttons(ui);
        },
    );
}

fn rename(
    ctx: &egui::Context,
    p: Palette,
    state: &mut UiState,
    workspace: pace_model::WorkspaceId,
    actions: &mut Vec<Action>,
) {
    let mut save = confirmed_by_enter(ctx, state);
    let mut cancel = false;
    let output = sheet(
        ctx,
        p,
        "Rename workspace",
        380.0,
        SheetPlacement::Center,
        |ui| {
            sheet_header(ui, p, "Rename workspace", None);
            padded(ui, 20.0, |ui| {
                let width = ui.available_width();
                let field = text_field(
                    ui,
                    p,
                    Id::new("workspace-rename"),
                    &mut state.rename_name,
                    "Workspace name",
                    "Workspace rename",
                    width,
                );
                if state.overlay_focus && accepts_focus(ui) {
                    field.request_focus();
                    state.overlay_focus = false;
                }
            });
            ui.add_space(4.0);
            footer(ui, "rename-actions", |ui| {
                save |= button(ui, p, "Save", ButtonKind::Primary).clicked();
                cancel = button(ui, p, "Cancel", ButtonKind::Secondary).clicked();
            });
        },
    );
    if cancel || output.backdrop_clicked {
        actions.push(Action::CloseOverlay);
    } else if save && !state.rename_name.trim().is_empty() {
        actions.push(Action::SetName(workspace, state.rename_name.trim().into()));
        actions.push(Action::CloseOverlay);
    }
}

/// Connects a workspace over SSH, or creates a connected one when `workspace`
/// is `None`. It asks only for the host: signing in happens in the terminal,
/// through the system client, and a new workspace is named after its host.
fn ssh(
    ctx: &egui::Context,
    p: Palette,
    state: &mut UiState,
    workspace: Option<pace_model::WorkspaceId>,
    actions: &mut Vec<Action>,
) {
    let title = if workspace.is_some() {
        "Connect over SSH"
    } else {
        "New SSH workspace"
    };
    let mut connect = confirmed_by_enter(ctx, state);
    let mut cancel = false;
    let output = sheet(ctx, p, title, 420.0, SheetPlacement::Center, |ui| {
        sheet_header(ui, p, title, None);
        padded(ui, 20.0, |ui| {
            let width = ui.available_width();
            let host = text_field(
                ui,
                p,
                Id::new("ssh-host"),
                &mut state.ssh_host,
                "user@host",
                "SSH host",
                width,
            );
            if state.overlay_focus && accepts_focus(ui) {
                host.request_focus();
                state.overlay_focus = false;
            }
            ui.add_space(12.0);
            let problem = (!state.ssh_host.trim().is_empty())
                .then(|| pace_model::Remote::parse(&state.ssh_host).err())
                .flatten();
            let (note, ink) = match problem {
                Some(error) => (error.to_string(), p.red),
                None if workspace.is_some() => (
                    "Every terminal in this workspace restarts on the host. Running processes will stop."
                        .to_owned(),
                    p.secondary,
                ),
                None => (
                    "Terminals open on the host using your SSH configuration and keys. Sign-in prompts appear in the terminal."
                        .to_owned(),
                    p.secondary,
                ),
            };
            ui.add(
                egui::Label::new(
                    egui::RichText::new(note)
                        .font(theme::regular(12.0))
                        .color(ink),
                )
                .wrap()
                .selectable(false),
            );
        });
        ui.add_space(4.0);
        footer(ui, "ssh-actions", |ui| {
            connect |= button(ui, p, "Connect", ButtonKind::Primary).clicked();
            cancel = button(ui, p, "Cancel", ButtonKind::Secondary).clicked();
        });
    });
    if cancel || output.backdrop_clicked {
        actions.push(Action::CloseOverlay);
    } else if connect {
        match pace_model::Remote::parse(&state.ssh_host) {
            Ok(remote) => actions.push(Action::Connect {
                workspace,
                destination: remote.destination().to_owned(),
            }),
            // Enter leaves a single-line field. Return the keyboard to the
            // host so it can be corrected without reaching for the pointer.
            Err(_) => state.overlay_focus = true,
        }
    }
}

/// Title, consequence and confirming verb for each close target.
pub fn close_copy(close: Close) -> (&'static str, &'static str, &'static str) {
    match close {
        Close::Pane(_) => (
            "Close terminal?",
            "Any process running in this terminal will stop.",
            "Close",
        ),
        Close::Workspace(_) => (
            "Close workspace?",
            "Every terminal in this workspace will close, and running processes will stop.",
            "Close",
        ),
        Close::Connection(_) => (
            "Disconnect from SSH?",
            "Every terminal in this workspace restarts as a local shell. Processes running in them will stop.",
            "Disconnect",
        ),
        Close::App => (
            "Quit Pace?",
            "Running processes in all terminals will stop. Workspaces reopen with fresh shells.",
            "Quit",
        ),
    }
}

fn confirm_close(ctx: &egui::Context, p: Palette, close: Close, actions: &mut Vec<Action>) {
    let (title, message, verb) = close_copy(close);
    let mut confirm = false;
    let mut cancel = false;
    let output = sheet(ctx, p, title, 360.0, SheetPlacement::Center, |ui| {
        ui.add_space(22.0);
        padded(ui, 22.0, |ui| {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(title)
                        .font(theme::semibold(15.0))
                        .color(p.fg),
                )
                .wrap()
                .selectable(false),
            );
            ui.add_space(6.0);
            ui.add(
                egui::Label::new(
                    egui::RichText::new(message)
                        .font(theme::regular(13.0))
                        .color(p.secondary),
                )
                .wrap()
                .selectable(false),
            );
        });
        ui.add_space(6.0);
        footer(ui, "confirm-close-actions", |ui| {
            confirm = button(ui, p, verb, ButtonKind::Destructive).clicked();
            cancel = button(ui, p, "Cancel", ButtonKind::Secondary).clicked();
        });
    });
    if confirm {
        actions.push(Action::Confirm(close));
    } else if cancel || output.backdrop_clicked {
        actions.push(Action::CancelClose);
    }
}

pub fn show(ctx: &egui::Context, p: Palette, state: &mut UiState, actions: &mut Vec<Action>) {
    match state.overlay {
        OverlayState::Rename(workspace) => rename(ctx, p, state, workspace, actions),
        OverlayState::Ssh(workspace) => ssh(ctx, p, state, workspace, actions),
        OverlayState::ConfirmClose(close) => confirm_close(ctx, p, close, actions),
        _ => {}
    }
    if let Some(error) = &state.error
        && toast(ctx, p, error)
    {
        actions.push(Action::DismissError);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Pos2;
    use pace_model::{PaneId, WorkspaceId};

    #[test]
    fn rename_gives_its_field_the_keyboard_once_visible() {
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::platform::fonts::bundled_definitions());
        let config = crate::config::Config::default();
        theme::apply(&ctx, &config);
        let p = Palette::for_config(&config);
        let mut state = UiState {
            overlay: OverlayState::Rename(WorkspaceId::new(1)),
            overlay_focus: true,
            ..UiState::default()
        };
        let mut actions = Vec::new();
        for _ in 0..4 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(900.0, 600.0))),
                    ..Default::default()
                },
                |ui| show(ui.ctx(), p, &mut state, &mut actions),
            );
            output.textures_delta.clear();
        }
        assert!(!state.overlay_focus, "the request is made exactly once");
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(Id::new("workspace-rename"))
        );
        assert!(actions.is_empty());
    }

    #[test]
    fn only_a_fresh_enter_after_the_dialog_is_shown_confirms_it() {
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::platform::fonts::bundled_definitions());
        let config = crate::config::Config::default();
        theme::apply(&ctx, &config);
        let p = Palette::for_config(&config);
        let mut state = UiState {
            overlay: OverlayState::Rename(WorkspaceId::new(1)),
            overlay_focus: true,
            rename_name: "Renamed".into(),
            ..UiState::default()
        };
        // The toolkit derives key repeat itself: a press with no release
        // since the previous press is a repeat.
        let enter = |pressed| egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        let frame = |state: &mut UiState, events: Vec<egui::Event>| {
            let mut actions = Vec::new();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(900.0, 600.0))),
                    events,
                    ..Default::default()
                },
                |ui| show(ui.ctx(), p, state, &mut actions),
            );
            output.textures_delta.clear();
            actions
        };
        // The press that opened the dialog arrives with its first frame.
        assert!(frame(&mut state, vec![enter(true)]).is_empty());
        // The key is still held, so it keeps repeating into the dialog.
        assert!(frame(&mut state, vec![enter(true)]).is_empty());
        assert!(frame(&mut state, vec![enter(true)]).is_empty());
        assert!(frame(&mut state, vec![enter(false)]).is_empty());
        let actions = frame(&mut state, vec![enter(true)]);
        assert!(
            matches!(&actions[..], [Action::SetName(workspace, name), Action::CloseOverlay]
                if *workspace == WorkspaceId::new(1) && name == "Renamed"),
            "a deliberate Enter renames the original workspace"
        );
    }

    #[test]
    fn each_close_target_names_its_own_consequence() {
        let pane = close_copy(Close::Pane(PaneId::new(1)));
        let workspace = close_copy(Close::Workspace(WorkspaceId::new(1)));
        let app = close_copy(Close::App);
        let connection = close_copy(Close::Connection(WorkspaceId::new(1)));
        assert_eq!(pane.0, "Close terminal?");
        assert_ne!(pane.0, workspace.0);
        assert_ne!(workspace.1, app.1);
        assert_ne!(connection.1, workspace.1);
        assert_eq!(
            (pane.2, app.2, connection.2),
            ("Close", "Quit", "Disconnect")
        );
    }

    /// Runs the sheet for a frame with a deliberate Enter. Returns its actions
    /// and whether the host field asked for the keyboard back.
    fn confirm_ssh(host: &str, workspace: Option<WorkspaceId>) -> (Vec<Action>, bool) {
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::platform::fonts::bundled_definitions());
        let config = crate::config::Config::default();
        theme::apply(&ctx, &config);
        let p = Palette::for_config(&config);
        let mut state = UiState {
            overlay: OverlayState::Ssh(workspace),
            ssh_host: host.into(),
            ..UiState::default()
        };
        let mut actions = Vec::new();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(900.0, 600.0))),
                events: vec![egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| show(ui.ctx(), p, &mut state, &mut actions),
        );
        output.textures_delta.clear();
        (actions, state.overlay_focus)
    }

    #[test]
    fn the_ssh_sheet_connects_its_own_target_and_only_to_a_usable_host() {
        let existing = WorkspaceId::new(7);
        let (actions, refocus) = confirm_ssh(" me@devbox ", Some(existing));
        assert!(!refocus);
        assert!(matches!(
            &actions[..],
            [Action::Connect { workspace: Some(id), destination }]
                if *id == existing && destination == "me@devbox"
        ));
        assert!(matches!(
            &confirm_ssh("devbox", None).0[..],
            [Action::Connect { workspace: None, destination }] if destination == "devbox"
        ));
        // Nothing is sent for an empty host, or one the client could misread,
        // and the field takes the keyboard back so it can be corrected.
        for host in ["", "  ", "-oProxyCommand=id", "dev box"] {
            let (actions, refocus) = confirm_ssh(host, None);
            assert!(actions.is_empty() && refocus, "{host:?}");
        }
    }
}

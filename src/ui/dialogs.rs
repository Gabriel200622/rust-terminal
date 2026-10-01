//! Transient dialogs edit only UI drafts and emit targeted commands.
use super::helpers::expand_home;
use super::{Action, OverlayState, UiState};
use eframe::egui::{self, Align2, Layout, Vec2};

pub fn show(ctx: &egui::Context, state: &mut UiState, actions: &mut Vec<Action>) {
    match state.overlay {
        OverlayState::NewWorkspace => {
            let mut open = true;
            let mut create = false;
            egui::Window::new("New workspace")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_width(400.0)
                .default_height(0.0)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.add_space(6.0);
                    ui.label("Directory");
                    ui.add(
                        egui::TextEdit::singleline(&mut state.new_cwd)
                            .id(egui::Id::new("workspace-directory"))
                            .desired_width(f32::INFINITY),
                    )
                    .widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::TextEdit,
                            true,
                            "Workspace directory",
                        )
                    });
                    ui.label("Name");
                    ui.add(
                        egui::TextEdit::singleline(&mut state.new_name)
                            .id(egui::Id::new("workspace-name"))
                            .hint_text("Use directory name")
                            .desired_width(f32::INFINITY),
                    )
                    .widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::TextEdit,
                            true,
                            "Workspace name",
                        )
                    });
                    ui.add_space(12.0);
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Create").clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Enter))
                        {
                            create = true;
                        }
                    });
                });
            if create {
                actions.push(Action::Create(
                    expand_home(&state.new_cwd),
                    (!state.new_name.trim().is_empty()).then(|| state.new_name.trim().to_owned()),
                ));
            }
            if !open {
                state.overlay = OverlayState::None;
            }
        }
        OverlayState::Rename(workspace) => {
            let mut done = false;
            egui::Window::new("Rename workspace")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut state.rename_name)
                            .id(egui::Id::new("workspace-rename")),
                    )
                    .widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::TextEdit,
                            true,
                            "Workspace rename",
                        )
                    });
                    if ui.button("Save").clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter))
                    {
                        done = true;
                    }
                });
            if done {
                actions.push(Action::SetName(workspace, state.rename_name.trim().into()));
                state.overlay = OverlayState::None;
            }
        }
        OverlayState::ConfirmClose(close) => {
            egui::Window::new("Close terminal?")
                .collapsible(false)
                .resizable(false)
                .default_width(320.0)
                .default_height(0.0)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label("Running processes in these terminals will stop.");
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            actions.push(Action::CancelClose);
                        }
                        if ui.button("Close").clicked() {
                            actions.push(Action::Confirm(close));
                        }
                    });
                });
        }
        _ => {}
    }
    if let Some(error) = state.error.clone() {
        let mut open = true;
        egui::Window::new("Pace")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(460.0)
            .default_height(0.0)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(error);
                ui.add_space(8.0);
                if ui.button("Dismiss").clicked() {
                    state.error = None;
                }
            });
        if !open {
            state.error = None;
        }
    }
}

//! Window effects are expressed as operations and applied by the desktop host.

use eframe::egui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowOperation {
    StartDrag,
    SetMaximized(bool),
    Minimize,
    /// Bring the window forward, restoring it if minimized.
    Show,
    Close,
    CancelClose,
    Screenshot,
    Resize(egui::ResizeDirection),
}

pub trait WindowService {
    fn apply(&mut self, operation: WindowOperation);
}

impl WindowService for egui::Context {
    fn apply(&mut self, operation: WindowOperation) {
        send(self, operation);
    }
}

pub fn send(ctx: &egui::Context, operation: WindowOperation) {
    let command = match operation {
        WindowOperation::StartDrag => egui::ViewportCommand::StartDrag,
        WindowOperation::SetMaximized(value) => egui::ViewportCommand::Maximized(value),
        WindowOperation::Minimize => egui::ViewportCommand::Minimized(true),
        WindowOperation::Show => {
            // A minimized window cannot take focus. Restoring one that is not
            // minimized could leave a maximized state on some platforms.
            if ctx.input(|input| input.viewport().minimized == Some(true)) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            }
            egui::ViewportCommand::Focus
        }
        WindowOperation::Close => egui::ViewportCommand::Close,
        WindowOperation::CancelClose => egui::ViewportCommand::CancelClose,
        WindowOperation::Screenshot => egui::ViewportCommand::Screenshot(Default::default()),
        WindowOperation::Resize(direction) => egui::ViewportCommand::BeginResize(direction),
    };
    ctx.send_viewport_cmd(command);
}

/// Clears the minimized flag once the native window is showing again.
///
/// On macOS the toolkit records a minimize request but never reads the native
/// state back, and it runs no UI for a window it believes is minimized. A
/// window restored from the Dock would keep its last frame and ignore input.
pub fn sync_minimized(ctx: &egui::Context, native: Option<bool>) {
    let recorded = ctx.input(|input| input.viewport().minimized);
    if recorded == Some(true) && native == Some(false) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
    }
}

#[derive(Default, Debug)]
pub struct RecordingWindow {
    pub operations: Vec<WindowOperation>,
}

impl WindowService for RecordingWindow {
    fn apply(&mut self, operation: WindowOperation) {
        self.operations.push(operation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_effects_can_be_recorded_without_native_window() {
        let mut window = RecordingWindow::default();
        window.apply(WindowOperation::Minimize);
        window.apply(WindowOperation::CancelClose);
        assert_eq!(
            window.operations,
            [WindowOperation::Minimize, WindowOperation::CancelClose]
        );
    }

    /// The commands a hidden-window tick asks for, given the toolkit's
    /// recorded minimized flag and the native window's actual state.
    fn hidden_tick(recorded: Option<bool>, native: Option<bool>) -> Vec<egui::ViewportCommand> {
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .minimized = recorded;
        ctx.run_logic(&input, |ctx| sync_minimized(ctx, native))
            .viewport_commands
            .remove(&egui::ViewportId::ROOT)
            .unwrap_or_default()
    }

    #[test]
    fn restored_window_clears_the_stale_minimized_flag() {
        assert_eq!(
            hidden_tick(Some(true), Some(false)),
            [egui::ViewportCommand::Minimized(false)]
        );
    }

    #[test]
    fn minimized_flag_is_kept_while_the_window_is_minimized_or_unknown() {
        // Still minimized, never minimized, and hosts that cannot report it.
        for (recorded, native) in [
            (Some(true), Some(true)),
            (Some(true), None),
            (Some(false), Some(false)),
            (Some(false), Some(true)),
            (None, Some(false)),
        ] {
            assert!(
                hidden_tick(recorded, native).is_empty(),
                "recorded {recorded:?}, native {native:?}"
            );
        }
    }
}

//! Window effects are expressed as operations and applied by the desktop host.

use eframe::egui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowOperation {
    StartDrag,
    SetMaximized(bool),
    Minimize,
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
        WindowOperation::Close => egui::ViewportCommand::Close,
        WindowOperation::CancelClose => egui::ViewportCommand::CancelClose,
        WindowOperation::Screenshot => egui::ViewportCommand::Screenshot(Default::default()),
        WindowOperation::Resize(direction) => egui::ViewportCommand::BeginResize(direction),
    };
    ctx.send_viewport_cmd(command);
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
}

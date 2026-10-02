//! One close transaction captures pane identities across asynchronous OS checks.
use super::*;
use terminal_core::ProcessActivity;
use ui::CloseStatus;

pub(super) struct PendingClose {
    target: Close,
    panes: Vec<(PaneId, u64)>,
    checks: Vec<mpsc::Receiver<ProcessActivity>>,
    since: Instant,
    running: usize,
    unknown: usize,
}

impl App {
    fn close_panes(&self, close: Close) -> Vec<(PaneId, u64)> {
        self.controller
            .model()
            .workspaces()
            .iter()
            .filter(|workspace| match close {
                Close::Workspace(id) | Close::Connection(id) => workspace.id() == id,
                _ => true,
            })
            .flat_map(|workspace| workspace.panes())
            .filter(|pane| !matches!(close, Close::Pane(id) if pane.id() != id))
            .map(|pane| (pane.id(), pane.generation()))
            .collect()
    }

    pub(super) fn request_close(&mut self, ctx: &egui::Context, target: Close) {
        if self
            .pending_close
            .as_ref()
            .is_some_and(|pending| pending.target == target)
            && self.ui.overlay == OverlayState::ConfirmClose(target)
        {
            return;
        }
        self.pending_close = None;
        if !self.config.confirm_close && !self.config.warn_running_processes {
            self.finish_close(ctx, target);
            return;
        }
        let panes = self.close_panes(target);
        let mut pending = PendingClose {
            target,
            panes,
            checks: Vec::new(),
            since: Instant::now(),
            running: 0,
            unknown: usize::from(self.startup.is_some()),
        };
        if self.config.warn_running_processes {
            for &(pane, generation) in &pending.panes {
                if let Some(session) = self.sessions.get(pane)
                    && self.sessions.generation(pane) == Some(generation)
                {
                    if matches!(session.metadata().status, SessionStatus::Exited { .. }) {
                        continue;
                    }
                    // A local SSH process cannot inspect the remote host's jobs.
                    // Closing it is itself destructive, even at a remote prompt.
                    if self.remote_of(pane).is_some() {
                        pending.running += 1;
                    } else {
                        pending.checks.push(session.check_process_activity());
                    }
                } else if self.controller.model().pane(pane).is_some_and(|pane| {
                    !matches!(pane.lifecycle(), Lifecycle::Exited | Lifecycle::Failed(_))
                }) {
                    pending.unknown += 1;
                }
            }
        }
        self.ui.overlay = OverlayState::ConfirmClose(target);
        self.ui.close_status = CloseStatus::Checking;
        self.pending_close = Some(pending);
        self.poll_close(ctx);
    }

    pub(super) fn poll_close(&mut self, ctx: &egui::Context) {
        let Some(mut pending) = self.pending_close.take() else {
            return;
        };
        if self.ui.overlay != OverlayState::ConfirmClose(pending.target) {
            return;
        }
        if self.close_panes(pending.target) != pending.panes {
            // Restart, movement or restoration changed the affected sessions.
            // Old observations (and old confirmations) cannot authorize them.
            self.request_close(ctx, pending.target);
            return;
        }
        pending.checks.retain(|check| {
            match check.try_recv() {
                Ok(ProcessActivity::Running) => pending.running += 1,
                Ok(ProcessActivity::Unknown) | Err(mpsc::TryRecvError::Disconnected) => {
                    pending.unknown += 1
                }
                Ok(ProcessActivity::Idle) => {}
                Err(mpsc::TryRecvError::Empty)
                    if pending.since.elapsed() < Duration::from_secs(2) =>
                {
                    return true;
                }
                Err(mpsc::TryRecvError::Empty) => pending.unknown += 1,
            }
            false
        });
        if pending.checks.is_empty() && self.ui.close_status == CloseStatus::Checking {
            ctx.request_repaint();
        }
        if !pending.checks.is_empty() {
            // Workers wake on completion; the deadline also covers stalled I/O.
            ctx.request_repaint_after(Duration::from_millis(100));
        } else if pending.running > 0 {
            self.ui.close_status = CloseStatus::Running {
                terminals: pending.running,
                unknown: pending.unknown,
            };
        } else if pending.unknown > 0 && self.config.warn_running_processes {
            self.ui.close_status = CloseStatus::Unknown;
        } else if self.config.confirm_close {
            self.ui.close_status = CloseStatus::General;
        } else {
            self.finish_close(ctx, pending.target);
            return;
        }
        self.pending_close = Some(pending);
    }

    pub(super) fn confirm_close(&mut self, ctx: &egui::Context, target: Close) {
        let Some(pending) = self.pending_close.as_ref() else {
            return;
        };
        if pending.target != target
            || self.ui.overlay != OverlayState::ConfirmClose(target)
            || self.ui.close_status == CloseStatus::Checking
        {
            return;
        }
        if self.close_panes(target) != pending.panes {
            self.pending_close = None;
            self.request_close(ctx, target);
            return;
        }
        self.finish_close(ctx, target);
    }

    fn finish_close(&mut self, ctx: &egui::Context, target: Close) {
        self.pending_close = None;
        self.ui.overlay = OverlayState::None;
        match target {
            Close::App => self.exit_approved = true,
            Close::Pane(pane) => self.dispatch(ctx, Command::ClosePane(pane)),
            Close::Workspace(workspace) => self.dispatch(ctx, Command::CloseWorkspace(workspace)),
            Close::Connection(workspace) => self.dispatch(
                ctx,
                Command::SetWorkspaceRemote {
                    workspace,
                    remote: None,
                },
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (App, egui::Context, tempfile::TempDir, PaneId) {
        let root = tempfile::tempdir().unwrap();
        let (mut app, _sender) = crate::app::tests::fixture(root.path());
        app.startup = None;
        app.controller
            .dispatch(Command::AddWorkspace {
                cwd: root.path().into(),
                name: "Close test".into(),
                remote: None,
                group: None,
            })
            .unwrap();
        let pane = app.controller.model().active_pane().unwrap();
        (app, egui::Context::default(), root, pane)
    }

    fn observation(app: &mut App, target: Close) -> mpsc::SyncSender<ProcessActivity> {
        let (sender, receiver) = mpsc::sync_channel(1);
        app.ui.overlay = OverlayState::ConfirmClose(target);
        app.ui.close_status = CloseStatus::Checking;
        app.pending_close = Some(PendingClose {
            target,
            panes: app.close_panes(target),
            checks: vec![receiver],
            since: Instant::now(),
            running: 0,
            unknown: 0,
        });
        sender
    }

    #[test]
    fn close_process_warning_is_independent_and_idle_results_obey_general_preference() {
        for confirm in [false, true] {
            for result in [
                ProcessActivity::Idle,
                ProcessActivity::Running,
                ProcessActivity::Unknown,
            ] {
                let (mut app, ctx, _root, pane) = setup();
                app.config.confirm_close = confirm;
                let sender = observation(&mut app, Close::Pane(pane));
                sender.send(result).unwrap();
                app.poll_close(&ctx);
                let warns = confirm || result != ProcessActivity::Idle;
                assert_eq!(app.controller.model().pane(pane).is_some(), warns);
                assert_eq!(
                    matches!(app.ui.overlay, OverlayState::ConfirmClose(_)),
                    warns
                );
                if warns {
                    app.action(&ctx, Action::CancelClose);
                    assert!(app.controller.model().pane(pane).is_some());
                    assert!(app.pending_close.is_none());
                }
            }
        }
    }

    #[test]
    fn close_disabled_preferences_skip_checks_and_starting_sessions_otherwise_warn() {
        for warn in [false, true] {
            let (mut app, ctx, _root, pane) = setup();
            app.config.confirm_close = false;
            app.config.warn_running_processes = warn;
            app.action(&ctx, Action::ClosePane(pane));
            assert_eq!(app.controller.model().pane(pane).is_some(), warn);
            if warn {
                assert_eq!(app.ui.close_status, CloseStatus::Unknown);
            }
        }
    }

    #[test]
    fn close_failed_and_exited_sessions_do_not_trigger_process_warnings() {
        for failed in [false, true] {
            let (mut app, ctx, _root, pane) = setup();
            app.config.confirm_close = false;
            let completion = if failed {
                Command::SessionFailed {
                    pane,
                    generation: 1,
                    error: "spawn failed".into(),
                }
            } else {
                app.controller
                    .dispatch(Command::SessionStarted {
                        pane,
                        generation: 1,
                    })
                    .unwrap();
                Command::SessionExited {
                    pane,
                    generation: 1,
                }
            };
            app.controller.dispatch(completion).unwrap();
            app.action(&ctx, Action::ClosePane(pane));
            assert!(app.controller.model().pane(pane).is_none());
            assert_eq!(app.ui.overlay, OverlayState::None);
        }
    }

    #[test]
    fn close_general_confirmation_can_run_without_a_process_check() {
        let (mut app, ctx, _root, pane) = setup();
        app.config.warn_running_processes = false;
        app.action(&ctx, Action::ClosePane(pane));
        assert_eq!(app.ui.close_status, CloseStatus::General);
        assert!(app.pending_close.as_ref().unwrap().checks.is_empty());
        app.action(&ctx, Action::Confirm(Close::Pane(pane)));
        assert!(app.controller.model().pane(pane).is_none());
    }

    #[test]
    fn close_late_results_cannot_close_cancelled_or_replaced_panes() {
        let (mut app, ctx, _root, pane) = setup();
        app.config.confirm_close = false;
        let sender = observation(&mut app, Close::Pane(pane));
        app.action(&ctx, Action::CancelClose);
        let _ = sender.send(ProcessActivity::Idle);
        app.poll_close(&ctx);
        assert!(app.controller.model().pane(pane).is_some());
        let sender = observation(&mut app, Close::Pane(pane));
        app.controller.dispatch(Command::RestartPane(pane)).unwrap();
        sender.send(ProcessActivity::Idle).unwrap();
        app.poll_close(&ctx);
        assert!(app.controller.model().pane(pane).is_some());
        assert_eq!(app.ui.close_status, CloseStatus::Unknown);
    }

    #[test]
    fn close_timeout_disconnect_and_confirmation_during_check_never_auto_close() {
        for disconnect in [false, true] {
            let (mut app, ctx, _root, pane) = setup();
            app.config.confirm_close = false;
            let sender = observation(&mut app, Close::Pane(pane));
            app.action(&ctx, Action::Confirm(Close::Pane(pane)));
            assert!(app.controller.model().pane(pane).is_some());
            if disconnect {
                drop(sender);
            } else {
                app.pending_close.as_mut().unwrap().since -= Duration::from_secs(3);
            }
            app.poll_close(&ctx);
            assert_eq!(app.ui.close_status, CloseStatus::Unknown);
            app.action(&ctx, Action::Confirm(Close::Pane(pane)));
            assert!(app.controller.model().pane(pane).is_none());
        }
    }

    #[test]
    fn close_app_and_workspace_include_hidden_panes_and_validate_confirmed_generations() {
        let (mut app, ctx, root, hidden) = setup();
        let workspace = app.controller.model().active_workspace().unwrap();
        app.controller
            .dispatch(Command::AddWorkspace {
                cwd: root.path().into(),
                name: "Visible".into(),
                remote: None,
                group: None,
            })
            .unwrap();
        assert_eq!(app.close_panes(Close::App).len(), 2);
        assert_eq!(
            app.close_panes(Close::Workspace(workspace)),
            vec![(hidden, 1)]
        );
        app.config.confirm_close = false;
        let sender = observation(&mut app, Close::App);
        sender.send(ProcessActivity::Running).unwrap();
        app.poll_close(&ctx);
        assert!(!app.exit_approved);
        app.controller
            .dispatch(Command::RestartPane(hidden))
            .unwrap();
        app.action(&ctx, Action::Confirm(Close::App));
        assert!(!app.exit_approved);
        app.action(&ctx, Action::Confirm(Close::App));
        assert!(app.exit_approved);
    }
}

//! Executes bounded effects and feeds completions to the same controller used by
//! shortcuts, sidebar, dialogs and pane widgets.
use super::*;
use crate::persistence::workspace_state::StateSnapshot;
use crate::runtime::persistence::SaveKind;
impl App {
    pub(super) fn dispatch(&mut self, ctx: &egui::Context, command: Command) {
        match self.controller.dispatch(command) {
            Ok(effects) => self.execute(ctx, effects),
            Err(error) => self.ui.error = Some(error.to_string()),
        }
    }
    pub(super) fn execute(&mut self, ctx: &egui::Context, effects: Vec<Effect>) {
        let replacements: std::collections::HashSet<_> = effects
            .iter()
            .filter_map(|effect| match effect {
                Effect::StartSession {
                    pane,
                    replacement: true,
                    ..
                } => Some(*pane),
                _ => None,
            })
            .collect();
        for effect in effects {
            match effect {
                Effect::StartSession {
                    pane,
                    generation,
                    cwd,
                    replacement,
                } => {
                    self.renders.insert(pane, PaneRender::new(pane));
                    let wake = ctx.clone();
                    let options = SessionOptions {
                        cwd,
                        cols: 100,
                        rows: 32,
                        scrollback: self.config.scrollback,
                        shell: self.config.shell.clone(),
                        ..Default::default()
                    };
                    if let Err(error) = self.sessions.start(
                        pane,
                        generation,
                        replacement,
                        options,
                        Arc::new(move || wake.request_repaint()),
                    ) {
                        self.diagnostics
                            .failure("start", Some(pane), Some(generation), "Runtime");
                        self.dispatch(
                            ctx,
                            Command::SessionFailed {
                                pane,
                                generation,
                                error: error.clone(),
                            },
                        );
                        self.ui.error = Some(format!("Pane {pane}: {error}"));
                    }
                }
                Effect::StopSession { pane, .. } => {
                    if !replacements.contains(&pane) {
                        self.sessions.close(pane);
                    }
                    self.renders.remove(&pane);
                }
                Effect::Focus { old, new } => {
                    if let Some(id) = old
                        && let Some(session) = self.sessions.get(id)
                    {
                        let _ = session.focus(false);
                        if let Some(render) = self.renders.get_mut(&id)
                            && let Some(button) = render.mouse_button.take()
                        {
                            let (col, row, _) = render.mouse_cell.unwrap_or((0, 0, None));
                            if let Some(bytes) = crate::input::mouse(
                                button,
                                col,
                                row,
                                false,
                                Default::default(),
                                render.cache.mode,
                            ) {
                                let _ = session.write(&bytes);
                            }
                        }
                    }
                    if let Some(id) = new
                        && let Some(session) = self.sessions.get(id)
                    {
                        let _ = session.focus(true);
                    }
                }
                Effect::ResetSearch => {
                    self.search_point = None;
                    self.search_task = None;
                    self.ui.search_error = None;
                }
                Effect::Persist { .. } => self.save_state(),
                Effect::SavePreferences => {
                    self.preference_generation += 1;
                    if let Some(writer) = &self.writer
                        && let Err(error) =
                            writer.submit_config(self.preference_generation, self.config.clone())
                    {
                        self.ui.error = Some(error);
                    }
                }
            }
        }
    }
    pub(super) fn save_state(&mut self) {
        if self.ephemeral || !self.state_writable {
            return;
        }
        if let Some(writer) = &self.writer
            && let Err(error) = writer.submit_state(
                self.controller.generation(),
                StateSnapshot::from_model(self.controller.model()),
            )
        {
            self.ui.error = Some(format!("Cannot queue workspace save: {error}"));
        }
    }
    pub(super) fn observe_window(&mut self, ctx: &egui::Context, size: egui::Vec2) {
        if self.ephemeral || !self.window_writable {
            return;
        }
        if self.restore_maximized {
            // eframe maps the initially hidden window after its first render.
            // Some window managers ignore the builder's pre-map maximize
            // request. Reapply it after rendering and keep startup observations
            // from replacing the saved geometry while the OS handles it.
            if ctx.input(|input| input.viewport().maximized) != Some(true)
                && self.started.elapsed() < Duration::from_secs(2)
            {
                crate::platform::window::send(
                    ctx,
                    crate::platform::window::WindowOperation::SetMaximized(true),
                );
                ctx.request_repaint_after(Duration::from_millis(30));
                return;
            }
            self.restore_maximized = false;
        }
        let changed = ctx.input(|input| {
            let viewport = input.viewport();
            self.window_state.observe(
                [size.x, size.y],
                viewport.maximized,
                viewport.minimized.unwrap_or(false),
                viewport.fullscreen.unwrap_or(false),
            )
        });
        if changed {
            self.window_generation += 1;
            self.save_window();
        }
    }
    pub(super) fn save_window(&mut self) {
        if self.ephemeral || !self.window_writable {
            return;
        }
        if let Some(writer) = &self.writer
            && let Err(error) = writer.submit_window(self.window_generation, self.window_state)
        {
            self.ui.error = Some(format!("Cannot queue window save: {error}"));
        }
    }
    pub(super) fn poll_saves(&mut self, ctx: &egui::Context) {
        let events = self
            .writer
            .as_ref()
            .map(|writer| writer.drain_events())
            .unwrap_or_default();
        for event in events {
            match event.result {
                Ok(()) => {
                    if event.kind == SaveKind::State {
                        self.dispatch(
                            ctx,
                            Command::AcknowledgeSave {
                                generation: event.generation,
                            },
                        );
                    }
                }
                Err(error) => {
                    self.diagnostics.failure(
                        match event.kind {
                            SaveKind::State => "save_state",
                            SaveKind::Config => "save_preferences",
                            SaveKind::Window => "save_window",
                        },
                        None,
                        Some(event.generation),
                        "Persistence",
                    );
                    self.ui.error = Some(format!(
                        "{:?} save {} failed: {error}",
                        event.kind, event.generation
                    ))
                }
            }
        }
    }
    pub(super) fn action(&mut self, ctx: &egui::Context, action: Action) {
        if self.startup.is_some()
            && matches!(
                action,
                Action::Create(..) | Action::Preferences(_) | Action::ToggleSidebar
            )
        {
            if matches!(action, Action::Preferences(_)) {
                self.deferred_actions
                    .retain(|pending| !matches!(pending, Action::Preferences(_)));
            }
            if self.deferred_actions.len() >= 24 {
                self.ui.error =
                    Some("Too many workspace operations are waiting for restoration".into());
                return;
            }
            if matches!(action, Action::Create(..)) {
                self.ui.overlay = OverlayState::None;
            }
            self.deferred_actions.push(action);
            ctx.request_repaint();
            return;
        }
        match action {
            Action::Create(cwd, name) => {
                let name = name.unwrap_or_else(|| {
                    cwd.file_name()
                        .unwrap_or_else(|| std::ffi::OsStr::new("Home"))
                        .to_string_lossy()
                        .into_owned()
                });
                self.dispatch(ctx, Command::AddWorkspace { cwd, name });
                self.ui.overlay = OverlayState::None;
            }
            Action::Split(pane, axis) => {
                if let Some(workspace) = self.controller.model().workspace_for_pane(pane) {
                    let cwd = self
                        .sessions
                        .get(pane)
                        .map(|session| session.metadata().cwd)
                        .or_else(|| self.controller.model().pane(pane).map(|p| p.cwd().into()))
                        .unwrap_or_default();
                    self.dispatch(
                        ctx,
                        Command::SplitPane {
                            workspace,
                            pane,
                            axis,
                            cwd,
                        },
                    );
                }
            }
            Action::SelectWorkspace(id) => self.dispatch(ctx, Command::SelectWorkspace(id)),
            Action::Focus(pane) => {
                if let Some(workspace) = self.controller.model().workspace_for_pane(pane) {
                    self.dispatch(ctx, Command::FocusPane { workspace, pane });
                }
            }
            Action::Ratio(split, ratio) => {
                self.dispatch(ctx, Command::SetSplitRatio { split, ratio })
            }
            Action::ClosePane(pane) => self.request_close(ctx, Close::Pane(pane)),
            Action::CloseWorkspace(id) => self.request_close(ctx, Close::Workspace(id)),
            Action::WindowClose => self.request_close(ctx, Close::App),
            Action::New => {
                if let Some(dirs) = directories::BaseDirs::new() {
                    self.action(ctx, Action::Create(dirs.home_dir().into(), None));
                } else {
                    self.ui.error = Some("Could not determine your home directory".into());
                }
            }
            Action::Rename(id) => {
                if let Some(w) = self.controller.model().workspace(id) {
                    self.ui.rename_name = w.name().into();
                    self.ui.overlay = OverlayState::Rename(id);
                    self.ui.overlay_focus = true;
                }
            }
            Action::SetName(workspace, name) => {
                self.dispatch(ctx, Command::RenameWorkspace { workspace, name })
            }
            Action::Settings => {
                self.ui.overlay = if self.ui.overlay == OverlayState::Settings {
                    OverlayState::None
                } else {
                    OverlayState::Settings
                };
            }
            Action::Palette => {
                self.ui.palette_query.clear();
                self.ui.palette_selected = 0;
                self.ui.overlay = if self.ui.overlay == OverlayState::Palette {
                    OverlayState::None
                } else {
                    OverlayState::Palette
                };
            }
            Action::ToggleSidebar => {
                let shown = self.controller.model().sidebar();
                self.ui.sidebar_slide = Some(ui::chrome::SidebarSlide::toggled(
                    self.ui.sidebar_slide,
                    shown,
                    ctx.input(|input| input.time),
                ));
                self.dispatch(ctx, Command::SetSidebar(!shown));
                ctx.request_repaint();
            }
            Action::SidebarWidth(width) => {
                let config = Config {
                    sidebar_width: width.clamp(170.0, 360.0),
                    ..self.config.clone()
                };
                self.action(ctx, Action::Preferences(config));
            }
            Action::Zoom => self.ui.zoomed = !self.ui.zoomed,
            Action::ZoomUiIn => egui::gui_zoom::zoom_in(ctx),
            Action::ZoomUiOut => egui::gui_zoom::zoom_out(ctx),
            Action::ResetUiZoom => ctx.set_zoom_factor(1.0),
            Action::Find => {
                let editing = ctx.memory(|memory| memory.has_focus(ui::search::input_id()));
                if self.ui.search_open && !editing {
                    // Search is open but the terminal has the keyboard: return
                    // to the field instead of closing it.
                    self.ui.search_focus = true;
                } else {
                    self.ui.search_open = !self.ui.search_open;
                    self.ui.search_focus = self.ui.search_open;
                    self.search_point = None;
                    self.search_task = None;
                }
            }
            Action::SearchChanged => {
                self.search_point = None;
                self.find_next(false);
            }
            Action::FindNext { reverse } => self.find_next(reverse),
            Action::CloseSearch => {
                self.ui.search_open = false;
                self.search_task = None;
            }
            Action::CloseOverlay => self.ui.overlay = OverlayState::None,
            Action::DismissError => self.ui.error = None,
            Action::Clear(pane) => {
                if let Some(session) = self.sessions.get(pane) {
                    session.clear_history();
                    session.scroll_to_bottom();
                    if let Err(error) = session.write(b"\x0c") {
                        self.ui.error = Some(error.to_string());
                    }
                }
            }
            Action::Restart(pane) => self.dispatch(ctx, Command::RestartPane(pane)),
            Action::Copy(pane) => {
                if let Some(text) = self
                    .sessions
                    .get(pane)
                    .and_then(|session| session.selected_text())
                {
                    crate::platform::clipboard::copy(ctx, text);
                }
            }
            Action::Paste(pane) => match crate::platform::clipboard::read() {
                Ok(text) => {
                    if let Some(session) = self.sessions.get(pane)
                        && let Err(error) = session.paste(&text)
                    {
                        self.ui.error = Some(error.to_string());
                    }
                }
                Err(error) => self.ui.error = Some(error),
            },
            Action::Preferences(mut config) => {
                if let Err(error) = config.validate() {
                    self.ui.error = Some(error.to_string());
                    return;
                }
                self.config = config;
                theme::apply(ctx, &self.config);
                for (_, session) in self.sessions.iter() {
                    set_session_palette(session, Palette::for_config(&self.config));
                }
                self.dispatch(ctx, Command::UpdatePreferences);
            }
            Action::Confirm(close) => {
                self.ui.overlay = OverlayState::None;
                match close {
                    Close::App => self.exit_approved = true,
                    Close::Pane(pane) => self.dispatch(ctx, Command::ClosePane(pane)),
                    Close::Workspace(workspace) => {
                        self.dispatch(ctx, Command::CloseWorkspace(workspace))
                    }
                }
            }
            Action::CancelClose => self.ui.overlay = OverlayState::None,
            Action::Resize(pane, geometry) => {
                let result = self.sessions.resize(
                    pane,
                    geometry.columns,
                    geometry.lines,
                    geometry.pixel_width,
                    geometry.pixel_height,
                    self.config.scrollback,
                );
                if let Some(render) = self.renders.get_mut(&pane) {
                    render.cache.resize_error = result.err();
                }
                ctx.request_repaint();
            }
            Action::Selection(pane, interaction) => {
                if let Some(session) = self.sessions.get(pane) {
                    use crate::terminal_view::SelectionInteraction;
                    match interaction {
                        SelectionInteraction::Start { point, kind } => {
                            session.start_selection_at(point, kind)
                        }
                        SelectionInteraction::Update(point) => session.update_selection_at(point),
                        SelectionInteraction::Clear => session.clear_selection(),
                    }
                }
            }
            Action::ScrollBottom(pane) => {
                if let Some(session) = self.sessions.get(pane) {
                    session.scroll_to_bottom();
                }
            }
        }
    }

    pub(super) fn send_startup_command(&mut self) {
        if self.command.is_none() || self.started.elapsed() < Duration::from_millis(650) {
            return;
        }
        let Some((pane, generation)) = self.command_target else {
            return;
        };
        if self
            .controller
            .model()
            .pane(pane)
            .is_none_or(|target| target.generation() != generation)
        {
            self.command = None;
            self.ui.error = Some(format!(
                "Startup command cancelled: pane {pane} was closed or restarted"
            ));
            return;
        }
        if self.controller.model().pane(pane).is_some_and(|target| {
            matches!(target.lifecycle(), Lifecycle::Failed(_) | Lifecycle::Exited)
        }) {
            self.command = None;
            self.ui.error.get_or_insert_with(|| {
                format!("Startup command cancelled: pane {pane} is unavailable")
            });
            return;
        }
        if let Some(session) = self.sessions.get(pane)
            && let Some(command) = self.command.take()
            && let Err(error) = session.write(format!("{command}\r").as_bytes())
        {
            self.diagnostics.failure(
                "launch_command",
                Some(pane),
                Some(generation),
                &format!("{:?}", error.kind()),
            );
            self.ui.error = Some(error.to_string());
        }
    }
    fn request_close(&mut self, ctx: &egui::Context, close: Close) {
        if self.config.confirm_close {
            self.ui.overlay = OverlayState::ConfirmClose(close);
        } else {
            self.action(ctx, Action::Confirm(close));
        }
    }
}

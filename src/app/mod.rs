//! Thin eframe composition: widgets emit actions, the controller owns transitions,
//! workers own processes/storage, and caches consume immutable terminal snapshots.
mod coordinator;
mod diagnostics;
mod input;
mod ssh;
#[cfg(test)]
mod tests;
use crate::{
    Launch,
    config::{self, Config},
    persistence::{
        window_state,
        workspace_state::{LoadReport, load_state},
    },
    runtime::{
        persistence::PersistenceWriter,
        sessions::{ResourcePolicy, SessionCompletion, SessionManager},
    },
    theme::{self, Palette, metrics},
    ui::{
        self, Action, Close, OverlayState, PaneRender, UiState, WorkspaceView,
        workspace::PanePresentation,
    },
};
use eframe::egui::{self, Pos2, Rect, Stroke, Vec2, emath::GuiRounding as _};
use neptune_model::{Command, Controller, Effect, Lifecycle, Limits, Model, PaneId, Remote};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};
use terminal_core::{
    Direction, Mode, Point, SearchBudget, SearchProgress, SearchQuery, SearchTask, SessionMetadata,
    SessionOptions, SessionStatus, ViewportSnapshot,
};

struct Startup {
    config: Config,
    report: LoadReport,
    error: Option<String>,
}
struct ActiveSearch {
    pane: PaneId,
    generation: u64,
    reverse: bool,
    task: SearchTask,
}
pub struct App {
    controller: Controller,
    sessions: SessionManager,
    renders: BTreeMap<PaneId, PaneRender>,
    config: Config,
    config_path: PathBuf,
    state_path: PathBuf,
    window_path: PathBuf,
    window_state: window_state::WindowState,
    window_writable: bool,
    window_generation: u64,
    restore_maximized: bool,
    writer: Option<PersistenceWriter>,
    state_writable: bool,
    startup: Option<mpsc::Receiver<Startup>>,
    deferred_actions: Vec<Action>,
    initial_cwd: Option<PathBuf>,
    /// SSH destination of the workspace opened when nothing is restored.
    initial_remote: Option<String>,
    /// Program that opens remote terminals.
    ssh_client: String,
    ui: UiState,
    search_point: Option<Point>,
    search_query: Option<(String, Arc<SearchQuery>)>,
    search_task: Option<ActiveSearch>,
    started: Instant,
    command: Option<String>,
    command_target: Option<(PaneId, u64)>,
    screenshot: Option<PathBuf>,
    capture_sent: bool,
    exit_approved: bool,
    ephemeral: bool,
    preference_generation: u64,
    /// An input-method composition is in progress.
    ime_composing: bool,
    /// The terminal widget that owned the keyboard on the previous frame.
    terminal_focus: Option<egui::Id>,
    /// A sheet or the palette was open when focus was last reconciled.
    overlay_was_open: bool,
    _font_shortcut_monitor: crate::platform::keyboard::FontShortcutMonitor,
    diagnostics: diagnostics::Diagnostics,
    link_opener: crate::platform::links::LinkOpener,
}
impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        launch: Launch,
        window: window_state::LoadReport,
    ) -> Self {
        // Neptune routes zoom before terminal input and reserves Ctrl+Shift for
        // terminal font size. The toolkit's permissive shortcuts overlap it.
        cc.egui_ctx
            .options_mut(|options| options.zoom_with_keyboard = false);
        theme::fonts(&cc.egui_ctx);
        if launch.diagnostics
            && let Some(render) = &cc.wgpu_render_state
        {
            let a = render.adapter.get_info();
            eprintln!(
                "{}",
                serde_json::json!({"operation":"renderer","backend":format!("{:?}",a.backend),"adapter":a.name,"scale":cc.egui_ctx.pixels_per_point(),"platform":std::env::consts::OS,"version":env!("CARGO_PKG_VERSION")})
            );
        }
        let data = launch.data_root.unwrap_or_else(config::data_dir);
        let config_path = launch.config.unwrap_or_else(|| data.join("config.toml"));
        let state_path = data.join("workspaces.json");
        let ephemeral = launch.screenshot.is_some();
        let restore =
            !launch.no_restore && launch.cwd.is_none() && launch.ssh.is_none() && !ephemeral;
        let (sender, startup) = mpsc::sync_channel(1);
        let settings = config_path.clone();
        let state = state_path.clone();
        let wake = cc.egui_ctx.clone();
        let thread_result = std::thread::Builder::new()
            .name("neptune-restore".into())
            .spawn(move || {
                let (config, error) = match Config::load(&settings) {
                    Ok(config) => (config, None),
                    Err(error) => (Config::default(), Some(error.to_string())),
                };
                let report = if restore && config.restore_workspaces {
                    load_state(&state, Limits::default())
                } else {
                    LoadReport {
                        model: None,
                        diagnostics: Vec::new(),
                        can_write: true,
                        migrated: false,
                    }
                };
                let _ = sender.send(Startup {
                    config,
                    report,
                    error,
                });
                wake.request_repaint();
            });
        let mut ui = UiState {
            error: window.error,
            ..UiState::default()
        };
        if let Err(error) = thread_result {
            ui.error = Some(format!("Could not start restoration worker: {error}"));
        }
        Self {
            _font_shortcut_monitor: crate::platform::keyboard::FontShortcutMonitor::install(),
            controller: Controller::new(Model::default()),
            sessions: SessionManager::new(ResourcePolicy::default(), launch.diagnostics),
            renders: BTreeMap::new(),
            config: Config::default(),
            config_path,
            state_path,
            window_path: data.join("window.json"),
            window_state: window.state,
            window_writable: window.can_write,
            window_generation: 0,
            restore_maximized: window.state.maximized,
            writer: None,
            state_writable: false,
            startup: Some(startup),
            deferred_actions: Vec::new(),
            initial_cwd: launch.cwd,
            initial_remote: launch.ssh,
            ssh_client: coordinator::SSH_CLIENT.into(),
            ui,
            search_point: None,
            search_query: None,
            search_task: None,
            started: Instant::now(),
            command: launch.command,
            command_target: None,
            screenshot: launch.screenshot,
            capture_sent: false,
            exit_approved: false,
            ephemeral,
            preference_generation: 0,
            ime_composing: false,
            terminal_focus: None,
            overlay_was_open: false,
            diagnostics: diagnostics::Diagnostics::new(launch.diagnostics),
            link_opener: Default::default(),
        }
    }
    fn poll(&mut self, ctx: &egui::Context) {
        if let Some(Err(error)) = self.link_opener.poll() {
            self.ui.error = Some(error.into());
        }
        let startup = self
            .startup
            .as_ref()
            .and_then(|receiver| match receiver.try_recv() {
                Ok(startup) => Some(startup),
                Err(mpsc::TryRecvError::Disconnected) => Some(Startup {
                    config: self.config.clone(),
                    report: LoadReport {
                        model: None,
                        diagnostics: Vec::new(),
                        // Preserve existing storage if restoration never completed.
                        can_write: false,
                        migrated: false,
                    },
                    error: Some("Restoration worker stopped before returning state".into()),
                }),
                Err(mpsc::TryRecvError::Empty) => None,
            });
        if let Some(startup) = startup {
            self.complete_startup(ctx, startup);
        }
        for event in self.sessions.poll() {
            match event {
                SessionCompletion::Started {
                    pane,
                    generation,
                    elapsed,
                } => {
                    if let Some(render) = self.renders.get_mut(&pane) {
                        render.cache.retry_resize();
                        render.cache.invalidate();
                    }
                    if let Some(session) = self.sessions.get(pane) {
                        set_session_palette(session, Palette::for_config(&self.config));
                        if self.controller.model().active_pane() == Some(pane) {
                            let _ = session.focus(true);
                        }
                    }
                    self.dispatch(ctx, Command::SessionStarted { pane, generation });
                    self.diagnostics
                        .operation("spawn", pane, generation, elapsed);
                }
                SessionCompletion::Failed {
                    pane,
                    generation,
                    message,
                } => {
                    self.diagnostics
                        .failure("spawn", Some(pane), Some(generation), "Spawn");
                    self.dispatch(
                        ctx,
                        Command::SessionFailed {
                            pane,
                            generation,
                            error: message.clone(),
                        },
                    );
                    self.ui.error = Some(format!("Pane {pane}, session {generation}: {message}"));
                }
            }
        }
        let metadata: Vec<_> = self
            .sessions
            .iter()
            .filter_map(|(id, session)| {
                self.controller.model().pane(id).map(|pane| {
                    (
                        id,
                        pane.generation(),
                        pane.lifecycle().clone(),
                        session.metadata(),
                    )
                })
            })
            .collect();
        for (pane, generation, lifecycle, metadata) in metadata {
            self.sync_directory(ctx, pane, &metadata);
            if !matches!(metadata.status, SessionStatus::Running) && lifecycle == Lifecycle::Running
            {
                self.dispatch(ctx, Command::SessionExited { pane, generation });
            }
        }
        // The coordinator is the sole consumer of bounded clipboard events.
        // Selection stores are unsupported by the portable desktop clipboard.
        for (_, session) in self.sessions.iter() {
            for event in session.drain_events() {
                match event {
                    terminal_core::TerminalEvent::ClipboardStore {
                        selection: false,
                        text,
                    } => crate::platform::clipboard::copy(ctx, text),
                    terminal_core::TerminalEvent::ClipboardStore {
                        selection: true, ..
                    } => {}
                }
            }
        }
        self.poll_saves(ctx);
        self.poll_search(ctx);
        if self.diagnostics.enabled() {
            ctx.request_repaint_after(Duration::from_secs(1));
        }
        if self.startup.is_some()
            || self.sessions.usage().starting > 0
            || self.sessions.usage().closing > 0
        {
            ctx.request_repaint_after(Duration::from_millis(30));
        }
    }
    fn complete_startup(&mut self, ctx: &egui::Context, startup: Startup) {
        self.startup = None;
        self.config = startup.config;
        ctx.set_zoom_factor(self.config.window_zoom);
        theme::apply(ctx, &self.config);
        self.state_writable = startup.report.can_write;
        let mut errors = startup.report.diagnostics;
        if let Some(error) = startup.error {
            errors.push(error);
        }
        if !errors.is_empty() {
            self.ui.error = Some(errors.join("\n"));
        }
        self.controller = Controller::new(startup.report.model.unwrap_or_default());
        let repaint = ctx.clone();
        match PersistenceWriter::new(
            self.state_path.clone(),
            self.config_path.clone(),
            self.window_path.clone(),
            !self.ephemeral,
            Arc::new(move || repaint.request_repaint()),
        ) {
            Ok(writer) => self.writer = Some(writer),
            Err(error) => {
                self.diagnostics
                    .failure("persistence_worker", None, None, "Persistence");
                self.ui.error = Some(format!("Persistence worker: {error}"));
            }
        }
        self.execute(ctx, self.controller.start_effects());
        if self.controller.model().workspaces().is_empty() {
            let cwd = self.initial_cwd.take().unwrap_or_else(default_cwd);
            let remote = self.initial_remote.take();
            self.create_workspace(ctx, cwd, None, remote, None);
        }
        // Bind the CLI command before queued user actions can change focus.
        self.command_target = self.controller.model().active_pane().and_then(|id| {
            self.controller
                .model()
                .pane(id)
                .map(|pane| (id, pane.generation()))
        });
        for action in std::mem::take(&mut self.deferred_actions) {
            self.action(ctx, action);
        }
        self.save_state();
    }
    /// The SSH destination of the workspace that holds `pane`.
    fn remote_of(&self, pane: PaneId) -> Option<&Remote> {
        let model = self.controller.model();
        model
            .workspace_for_pane(pane)
            .and_then(|id| model.workspace(id))
            .and_then(|workspace| workspace.remote())
    }
    fn views(&self) -> Vec<WorkspaceView> {
        self.controller
            .model()
            .workspaces()
            .iter()
            .map(|w| WorkspaceView {
                id: w.id(),
                group: w.group(),
                name: w.name().into(),
                cwd: w.cwd().into(),
                remote: w.remote().map(|remote| remote.destination().to_owned()),
                panes: w.panes().len(),
                running: w
                    .panes()
                    .iter()
                    .any(|p| matches!(p.lifecycle(), Lifecycle::Starting | Lifecycle::Running)),
            })
            .collect()
    }
    fn presentations(&self) -> BTreeMap<PaneId, PanePresentation> {
        let Some(workspace) = self
            .controller
            .model()
            .active_workspace()
            .and_then(|id| self.controller.model().workspace(id))
        else {
            return BTreeMap::new();
        };
        let remote = workspace
            .remote()
            .map(|remote| remote.destination().to_owned());
        workspace
            .panes()
            .iter()
            .filter(|pane| !self.ui.zoomed || pane.id() == workspace.active())
            .map(|pane| {
                let presentation = if let Some(session) = self.sessions.get(pane.id()) {
                    session.acknowledge_repaint();
                    PanePresentation {
                        metadata: session.metadata(),
                        snapshot: session.viewport(),
                        starting: false,
                        remote: remote.clone(),
                    }
                } else {
                    let title = match pane.lifecycle() {
                        Lifecycle::Failed(error) => format!("Failed: {error}"),
                        _ => "Starting shell…".into(),
                    };
                    PanePresentation {
                        metadata: SessionMetadata {
                            title,
                            shell: if remote.is_some() {
                                self.ssh_client.clone()
                            } else {
                                self.config.shell.clone().unwrap_or_else(|| "shell".into())
                            },
                            cwd: pane.cwd().into(),
                            reported_cwd: None,
                            process_id: None,
                            status: match pane.lifecycle() {
                                Lifecycle::Failed(error) => SessionStatus::Error(error.clone()),
                                _ => SessionStatus::Running,
                            },
                            bell_count: 0,
                        },
                        // A placeholder has no shell, so it shows no cursor.
                        snapshot: ViewportSnapshot {
                            mode: Mode::NONE,
                            ..ViewportSnapshot::blank(80, 24)
                        },
                        starting: !matches!(pane.lifecycle(), Lifecycle::Failed(_)),
                        remote: remote.clone(),
                    }
                };
                (pane.id(), presentation)
            })
            .collect()
    }
    fn find_next(&mut self, reverse: bool) {
        if self.ui.search.is_empty() {
            self.search_task = None;
            return;
        }
        let Some(pane) = self.controller.model().active_pane() else {
            return;
        };
        let Some(session) = self.sessions.get(pane) else {
            return;
        };
        if self
            .search_query
            .as_ref()
            .is_none_or(|(query, _)| query != &self.ui.search)
        {
            match SearchQuery::compile(&ui::helpers::regex_escape(&self.ui.search)) {
                Ok(query) => self.search_query = Some((self.ui.search.clone(), query)),
                Err(error) => {
                    self.ui.search_error = Some(error.to_string());
                    return;
                }
            }
        }
        let Some((_, query)) = &self.search_query else {
            return;
        };
        let direction = if reverse {
            Direction::Left
        } else {
            Direction::Right
        };
        let origin = self
            .search_point
            .map(|point| session.next_search_point(point, direction))
            .unwrap_or_else(|| Point::new(-(session.viewport().display_offset as i32), 0));
        let generation = self
            .controller
            .model()
            .pane(pane)
            .map(|p| p.generation())
            .unwrap_or(0);
        self.search_task = Some(ActiveSearch {
            pane,
            generation,
            reverse,
            task: session.begin_search(
                query.clone(),
                origin,
                if reverse {
                    Direction::Left
                } else {
                    Direction::Right
                },
            ),
        });
        self.ui.search_error = Some("Searching…".into());
    }
    /// When a sheet or the palette closes, its focused control is gone but
    /// would keep the keyboard for one more frame. Release it at once so the
    /// terminal can take over without dropping keys.
    fn release_closed_overlay_focus(&mut self, ctx: &egui::Context) {
        let open = self.ui.overlay != OverlayState::None;
        if self.overlay_was_open && !open {
            ctx.memory_mut(|memory| {
                if let Some(id) = memory.focused() {
                    memory.surrender_focus(id);
                }
            });
        }
        self.overlay_was_open = open;
    }
    fn poll_search(&mut self, ctx: &egui::Context) {
        let Some(mut search) = self.search_task.take() else {
            return;
        };
        if self.controller.model().active_pane() != Some(search.pane)
            || self
                .controller
                .model()
                .pane(search.pane)
                .is_none_or(|p| p.generation() != search.generation)
        {
            return;
        }
        let Some(session) = self.sessions.get(search.pane) else {
            return;
        };
        match session.search_step(
            &mut search.task,
            SearchBudget {
                max_rows: 32,
                max_duration: Duration::from_millis(2),
            },
        ) {
            SearchProgress::Pending => {
                self.search_task = Some(search);
                ctx.request_repaint();
            }
            SearchProgress::Found(range) => {
                session.scroll_to_point(range.start);
                session.set_selection(range.start, range.end);
                self.search_point = Some(if search.reverse {
                    range.start
                } else {
                    range.end
                });
                self.ui.search_error = None;
            }
            SearchProgress::LimitExceeded => {
                self.ui.search_error =
                    Some("Search stopped: a logical line exceeds the 1 MiB search limit".into());
            }
            SearchProgress::NotFound => {
                self.search_point = None;
                self.ui.search_error = Some("No matches".into());
            }
            SearchProgress::Stale => {
                self.find_next(search.reverse);
                ctx.request_repaint_after(Duration::from_millis(30));
            }
            SearchProgress::Cancelled => {}
        }
    }
}
impl eframe::App for App {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array()
    }
    fn raw_input_hook(&mut self, _: &egui::Context, raw_input: &mut egui::RawInput) {
        crate::input::drop_redundant_preedits(&mut raw_input.events, &mut self.ime_composing);
    }
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let tick = Instant::now();
        let ctx = ui.ctx().clone();
        self.poll(&ctx);
        self.observe_window(&ctx, ui.max_rect().size());
        if ctx.input(|i| i.viewport().close_requested())
            && !self.exit_approved
            && self.config.confirm_close
        {
            crate::platform::window::send(
                &ctx,
                crate::platform::window::WindowOperation::CancelClose,
            );
            self.ui.overlay = OverlayState::ConfirmClose(Close::App);
        }
        self.shortcuts(&ctx);
        self.release_closed_overlay_focus(&ctx);
        // Sampled before widgets run: a menu that closes on this frame's key
        // press still owns that key.
        let menu_open = egui::Popup::is_any_open(&ctx);
        // Tab and the arrow keys move focus only inside sheets and menus.
        // Everywhere else they belong to the terminal or the focused field,
        // so the toolkit must not walk focus into the chrome.
        let sheet_open = !matches!(self.ui.overlay, OverlayState::None | OverlayState::Palette);
        if !sheet_open && !menu_open {
            ctx.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
        }
        // A sheet or the palette opening ends a terminal drag without a move.
        if self.ui.pane_drag.is_some() && self.ui.overlay != OverlayState::None {
            self.cancel_pane_drag(&ctx);
        }
        let p = Palette::for_config(&self.config);
        let bounds = ui.max_rect();
        let radius = if ctx.input(|i| {
            !i.viewport().fullscreen.unwrap_or(false)
                && (cfg!(target_os = "macos") || !i.viewport().maximized.unwrap_or(false))
        }) {
            metrics::WINDOW_RADIUS
        } else {
            0
        };
        ui.painter().rect_filled(bounds, radius, p.chrome);
        let mut actions = Vec::new();
        let views = self.views();
        let active = self.controller.model().active_workspace();
        let active_pane = self.controller.model().active_pane();
        let presentations = self.presentations();
        let subtitle = active_pane
            .and_then(|pane| presentations.get(&pane))
            .map(|presentation| {
                format!(
                    "{} — {}",
                    ui::workspace::pane_label(&presentation.metadata),
                    presentation.location()
                )
            })
            .unwrap_or_default();
        let sidebar_available = bounds.width() >= metrics::SIDEBAR_MIN_WINDOW;
        let sidebar_open = self.controller.model().sidebar() && sidebar_available;
        // Only a toggle slides. A window too narrow for the sidebar, like
        // restored state, takes its place at once.
        let sliding = self
            .ui
            .sidebar_slide
            .and_then(|slide| slide.reveal(sidebar_open, ctx.input(|input| input.time)))
            .filter(|_| sidebar_available);
        if sliding.is_some() {
            ctx.request_repaint();
        } else {
            self.ui.sidebar_slide = None;
        }
        if !sidebar_open {
            // Neither an edge drag nor a row drag can finish once the sidebar
            // is leaving.
            self.ui.sidebar_drag = None;
            self.ui.workspace_drag = Default::default();
            self.ui.group_drag = Default::default();
        }
        let sidebar_width = self
            .ui
            .sidebar_drag
            .unwrap_or(self.config.sidebar_width)
            .min(bounds.width() - 420.0);
        // Where the sidebar's trailing edge rests, and where it is this frame.
        let rest = if sidebar_open { sidebar_width } else { 0.0 };
        let edge = sliding.map_or(rest, |reveal| {
            (reveal * sidebar_width).round_to_pixels(ctx.pixels_per_point())
        });
        let reveal = sliding.unwrap_or(if sidebar_open { 1.0 } else { 0.0 });
        let chrome = ui::chrome::ChromeView {
            workspaces: &views,
            groups: self.controller.model().groups(),
            active,
            pane: active_pane,
            subtitle: &subtitle,
            zoomed: self.ui.zoomed,
            window: bounds,
            sidebar: reveal,
            sidebar_open,
            sidebar_width,
            sidebar_available,
            pane_drag: self.ui.pane_drag,
        };
        if edge > 0.0 {
            let side = Rect::from_min_size(
                Pos2::new(bounds.left() + edge - sidebar_width, bounds.top()),
                Vec2::new(sidebar_width, bounds.height()),
            );
            ui::chrome::sidebar(ui, side, p, &chrome, &mut self.ui, &mut actions);
        }
        let toolbar = Rect::from_min_max(
            Pos2::new(bounds.left() + edge, bounds.top()),
            Pos2::new(bounds.right(), bounds.top() + metrics::TOOLBAR_HEIGHT),
        );
        ui::chrome::toolbar(ui, toolbar, p, &chrome, &mut self.ui, &mut actions);
        ui::chrome::leading_controls(ui, p, &chrome, &mut actions);
        // Panes sit in the chrome like inset content; the sidebar supplies its
        // own trailing margin.
        let stage_from = |edge: f32| {
            Rect::from_min_max(
                Pos2::new(bounds.left() + edge.max(metrics::GUTTER), toolbar.bottom()),
                bounds.max - Vec2::splat(metrics::GUTTER),
            )
        };
        let stage = ui::workspace::Placement {
            drawn: stage_from(edge),
            settled: stage_from(rest),
        };
        let visible: std::collections::HashSet<_> = self
            .controller
            .model()
            .active_workspace()
            .and_then(|id| self.controller.model().workspace(id))
            .map(|w| {
                if self.ui.zoomed {
                    vec![w.active()]
                } else {
                    w.panes().iter().map(|p| p.id()).collect()
                }
            })
            .unwrap_or_default()
            .into_iter()
            .collect();
        let cache_limit =
            self.sessions.policy().max_sessions * self.sessions.policy().cache_bytes_per_session;
        let mut cache_bytes = self
            .renders
            .values()
            .map(|render| render.cache.estimated_bytes())
            .sum::<usize>();
        for (id, render) in &mut self.renders {
            if cache_bytes <= cache_limit {
                break;
            }
            if !visible.contains(id) {
                let bytes = render.cache.estimated_bytes();
                render.cache.evict();
                cache_bytes = cache_bytes.saturating_sub(bytes);
            }
        }
        let overlay = self.ui.overlay != OverlayState::None;
        let mut output = ui::workspace::StageOutput::default();
        if let Some(workspace) = active.and_then(|id| self.controller.model().workspace(id)) {
            let layout = if self.ui.zoomed {
                neptune_model::Layout::Leaf(workspace.active())
            } else {
                workspace.layout().clone()
            };
            let stage_view = ui::workspace::Stage {
                presentations: &presentations,
                active: workspace.active(),
                multiple: layout.leaves().len() > 1,
                zoomed: self.ui.zoomed,
                keyboard: !overlay,
                previous_terminal: self.terminal_focus,
                config: &self.config,
                p,
                search: if self.ui.search_open {
                    &self.ui.search
                } else {
                    ""
                },
                drag: self.ui.pane_drag,
            };
            ui::workspace::draw_node(
                ui,
                &layout,
                stage,
                &mut self.renders,
                &stage_view,
                &mut actions,
                &mut output,
            );
            ui::workspace::pane_drag(ui, &stage_view, &output, &mut actions);
        } else {
            ui::workspace::empty_state(ui, stage.drawn, p, self.startup.is_some(), &mut actions);
        }
        // A drag lasts only while its header reports it, so it cannot outlive a
        // release, a workspace switch or the pane itself.
        self.ui.pane_drag = output.dragging;
        for action in actions.drain(..) {
            self.action(&ctx, action);
        }
        // The focused terminal holds keyboard focus itself. Focus still on the
        // terminal that owned the keyboard a frame ago (after a split, close or
        // workspace switch) is in transit to the new one, so keys are not
        // dropped; any other focused widget owns typing.
        let focused = ctx.memory(|memory| memory.focused());
        let in_transit = focused.is_some() && focused == self.terminal_focus;
        let context = if self.ui.overlay != OverlayState::None || menu_open {
            crate::input::RoutingContext::Overlay
        } else if focused == Some(ui::search::input_id()) {
            crate::input::RoutingContext::TerminalSearch
        } else if focused.is_some() && focused != output.active_terminal && !in_transit {
            crate::input::RoutingContext::TextField
        } else if let Some(pane) = self.controller.model().active_pane() {
            crate::input::RoutingContext::TerminalPane(pane.get())
        } else {
            crate::input::RoutingContext::Overlay
        };
        if let Some(rect) = output.active_body {
            self.terminal_input(&ctx, rect, context);
        }
        self.terminal_focus = output.active_terminal;
        // Sheets dim the whole window, following its rounded shape.
        let dim = ui::helpers::animate(
            &ctx,
            egui::Id::new("overlay-scrim"),
            !matches!(self.ui.overlay, OverlayState::None | OverlayState::Palette),
            0.16,
        );
        let dim = if self.ui.overlay == OverlayState::Palette {
            // Opened from the keyboard many times a day: no transition.
            1.0
        } else {
            dim
        };
        ui::helpers::scrim(ui.painter(), bounds, radius, p, dim);
        let zoomed = self.ui.zoomed;
        let message = self.ui.error.is_some();
        match self.ui.overlay {
            OverlayState::Settings => ui::preferences::show(&ctx, &self.config, &mut actions),
            OverlayState::Palette => ui::palette::show(
                &ctx,
                p,
                &mut self.ui,
                &ui::palette::PaletteView {
                    pane: self.controller.model().active_pane(),
                    layout: self
                        .controller
                        .model()
                        .active_workspace()
                        .and_then(|id| self.controller.model().workspace(id))
                        .map(|workspace| workspace.layout()),
                    workspaces: &views,
                    groups: self.controller.model().groups(),
                    active: self.controller.model().active_workspace(),
                    config: &self.config,
                    zoomed,
                    message,
                },
                &mut actions,
            ),
            _ => {}
        }
        ui::dialogs::show(&ctx, p, &mut self.ui, &mut actions);
        for action in actions {
            self.action(&ctx, action);
        }
        self.send_startup_command();
        if self.command.is_some() || self.screenshot.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if self.screenshot.is_some() {
            if self.started.elapsed() > Duration::from_secs(3) && !self.capture_sent {
                crate::platform::window::send(
                    &ctx,
                    crate::platform::window::WindowOperation::Screenshot,
                );
                self.capture_sent = true;
            }
            for event in ctx.input(|i| i.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = event
                    && let Some(path) = self.screenshot.take()
                {
                    match save_png(&path, &image) {
                        Ok(()) => println!("Screenshot saved to {}", path.display()),
                        Err(error) => eprintln!("Screenshot: {error:#}"),
                    };
                    self.exit_approved = true;
                }
            }
        }
        self.diagnostics.frame(
            tick.elapsed(),
            &self.sessions,
            &self.renders,
            self.controller.model(),
        );
        if self.exit_approved {
            crate::platform::window::send(&ctx, crate::platform::window::WindowOperation::Close);
        }
        if radius > 0 {
            ui.painter().rect_stroke(
                bounds,
                radius,
                Stroke::new(1.0, p.border),
                egui::StrokeKind::Inside,
            );
        }
        ui::helpers::resize_edges(ui, bounds);
        self.release_closed_overlay_focus(&ctx);
    }
    fn on_exit(&mut self) {
        // Capture reports received since the final frame before flushing state.
        let ctx = egui::Context::default();
        let metadata: Vec<_> = self
            .sessions
            .iter()
            .map(|(pane, session)| (pane, session.metadata()))
            .collect();
        for (pane, metadata) in metadata {
            self.sync_directory(&ctx, pane, &metadata);
        }
        self.save_state();
        self.save_window();
        if let Some(writer) = &self.writer
            && let Err(error) = writer.flush()
        {
            eprintln!("Could not flush latest persisted state: {error}");
        }
        let complete = self.sessions.shutdown(Duration::from_secs(2));
        self.diagnostics.shutdown(complete);
    }
}
/// Where a workspace opens when no directory was chosen for it.
fn default_cwd() -> PathBuf {
    directories::BaseDirs::new()
        .map(|dirs| dirs.home_dir().into())
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}
fn set_session_palette(session: &terminal_core::TerminalSession, p: Palette) {
    use terminal_core::{NamedColor, Rgb};
    for (index, color) in p.ansi.iter().copied().enumerate().chain([
        (NamedColor::Foreground as usize, p.fg),
        (NamedColor::Background as usize, p.bg),
        (NamedColor::Cursor as usize, p.accent),
    ]) {
        session.set_color(
            index,
            Rgb {
                r: color.r(),
                g: color.g(),
                b: color.b(),
            },
        );
    }
}
fn save_png(path: &std::path::Path, image: &egui::ColorImage) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(std::fs::File::create(path)?),
        image.size[0] as u32,
        image.size[1] as u32,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(
        &image
            .pixels
            .iter()
            .flat_map(|pixel| pixel.to_array())
            .collect::<Vec<_>>(),
    )?;
    Ok(())
}

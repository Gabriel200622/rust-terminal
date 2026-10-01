//! Thin eframe composition: widgets emit actions, the controller owns transitions,
//! workers own processes/storage, and caches consume immutable terminal snapshots.
mod coordinator;
mod diagnostics;
mod input;
#[cfg(test)]
mod tests;
use crate::{
    Launch,
    config::{self, Config},
    persistence::workspace_state::{LoadReport, load_state},
    runtime::{
        persistence::PersistenceWriter,
        sessions::{ResourcePolicy, SessionCompletion, SessionManager},
    },
    theme::{self, Palette},
    ui::{
        self, Action, Close, OverlayState, PaneRender, UiState, WorkspaceView,
        workspace::PanePresentation,
    },
};
use eframe::egui::{self, Pos2, Rect, Stroke, UiBuilder, Vec2};
use pace_model::{Command, Controller, Effect, Lifecycle, Limits, Model, PaneId};
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
    writer: Option<PersistenceWriter>,
    state_writable: bool,
    startup: Option<mpsc::Receiver<Startup>>,
    deferred_actions: Vec<Action>,
    initial_cwd: Option<PathBuf>,
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
    diagnostics: diagnostics::Diagnostics,
}
impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, launch: Launch) -> Self {
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
        let restore = !launch.no_restore && launch.cwd.is_none() && !ephemeral;
        let (sender, startup) = mpsc::sync_channel(1);
        let settings = config_path.clone();
        let state = state_path.clone();
        let wake = cc.egui_ctx.clone();
        let thread_result = std::thread::Builder::new()
            .name("pace-restore".into())
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
        let mut ui = UiState::default();
        if let Err(error) = thread_result {
            ui.error = Some(format!("Could not start restoration worker: {error}"));
        }
        Self {
            controller: Controller::new(Model::default()),
            sessions: SessionManager::new(ResourcePolicy::default(), launch.diagnostics),
            renders: BTreeMap::new(),
            config: Config::default(),
            config_path,
            state_path,
            writer: None,
            state_writable: false,
            startup: Some(startup),
            deferred_actions: Vec::new(),
            initial_cwd: launch.cwd,
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
            diagnostics: diagnostics::Diagnostics::new(launch.diagnostics),
        }
    }
    fn poll(&mut self, ctx: &egui::Context) {
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
                        set_session_palette(session, Palette::new(self.config.theme));
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
                        pane.cwd().to_path_buf(),
                        pane.lifecycle().clone(),
                        session.metadata(),
                    )
                })
            })
            .collect();
        for (pane, generation, cwd, lifecycle, metadata) in metadata {
            if metadata.cwd != cwd {
                self.dispatch(
                    ctx,
                    Command::PaneCwdChanged {
                        pane,
                        generation,
                        cwd: metadata.cwd,
                    },
                );
            }
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
        theme::apply(ctx, self.config.theme);
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
            let cwd = self
                .initial_cwd
                .take()
                .or_else(|| directories::BaseDirs::new().map(|dirs| dirs.home_dir().into()))
                .or_else(|| std::env::current_dir().ok())
                .unwrap_or_else(|| PathBuf::from("."));
            self.action(ctx, Action::Create(cwd, None));
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
    fn views(&self) -> Vec<WorkspaceView> {
        self.controller
            .model()
            .workspaces()
            .iter()
            .map(|w| WorkspaceView {
                id: w.id(),
                name: w.name().into(),
                cwd: w.cwd().into(),
                running: w
                    .panes()
                    .iter()
                    .any(|p| p.lifecycle() == &Lifecycle::Running),
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
                    }
                } else {
                    let title = match pane.lifecycle() {
                        Lifecycle::Failed(error) => format!("Failed: {error}"),
                        _ => "Starting shell…".into(),
                    };
                    PanePresentation {
                        metadata: SessionMetadata {
                            title,
                            shell: self.config.shell.clone().unwrap_or_else(|| "shell".into()),
                            cwd: pane.cwd().into(),
                            process_id: None,
                            status: match pane.lifecycle() {
                                Lifecycle::Failed(error) => SessionStatus::Error(error.clone()),
                                _ => SessionStatus::Running,
                            },
                            bell_count: 0,
                        },
                        snapshot: ViewportSnapshot::blank(80, 24),
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
    #[cfg(target_os = "linux")]
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array()
    }
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let tick = Instant::now();
        let ctx = ui.ctx().clone();
        self.poll(&ctx);
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
        let p = Palette::new(self.config.theme);
        let bounds = ui.max_rect();
        let radius = if cfg!(target_os = "linux")
            && ctx.input(|i| {
                !i.viewport().maximized.unwrap_or(false)
                    && !i.viewport().fullscreen.unwrap_or(false)
            }) {
            12
        } else {
            0
        };
        ui.painter().rect_filled(bounds, radius, p.bg);
        let title = Rect::from_min_size(bounds.min, Vec2::new(bounds.width(), 46.0));
        let content = Rect::from_min_max(title.left_bottom(), bounds.max);
        let mut actions = Vec::new();
        let views = self.views();
        let active = self.controller.model().active_workspace();
        ui::chrome::titlebar(&views, active, ui, title, p, radius, &mut actions);
        let terminal = if self.controller.model().sidebar() && bounds.width() >= 850.0 {
            let side = Rect::from_min_size(
                content.min,
                Vec2::new(self.config.sidebar_width, content.height()),
            );
            ui::chrome::sidebar_ui(&views, active, ui, side, p, radius, &mut actions);
            Rect::from_min_max(Pos2::new(side.right(), content.top()), content.max)
        } else {
            content
        };
        let mut terminals = terminal.shrink(8.0);
        if self.ui.search_open {
            let find = Rect::from_min_size(terminals.min, Vec2::new(terminals.width(), 38.0));
            ui.scope_builder(
                UiBuilder::new()
                    .id_salt("terminal-search")
                    .max_rect(find)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
                |ui| {
                    ui.add_space(12.0);
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.ui.search)
                            .id(egui::Id::new("terminal-search-input"))
                            .hint_text("Find in scrollback…")
                            .desired_width(240.0),
                    );
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::TextEdit,
                            true,
                            "Terminal search",
                        )
                    });
                    if self.ui.search_focus {
                        response.request_focus();
                        self.ui.search_focus = false;
                    }
                    if response.changed() {
                        self.search_point = None;
                        self.find_next(false);
                    }
                    if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.find_next(ui.input(|i| i.modifiers.shift));
                    }
                    if let Some(error) = &self.ui.search_error {
                        ui.label(egui::RichText::new(error).size(10.0).color(p.muted));
                    }
                    if crate::icons::button(ui, crate::icons::Icon::ArrowUp, "Previous match")
                        .clicked()
                    {
                        self.find_next(true);
                    }
                    if crate::icons::button(ui, crate::icons::Icon::ArrowDown, "Next match")
                        .clicked()
                    {
                        self.find_next(false);
                    }
                    if crate::icons::button(ui, crate::icons::Icon::Close, "Close search").clicked()
                    {
                        self.ui.search_open = false;
                        self.search_task = None;
                    }
                },
            );
            terminals.min.y += 46.0;
        }
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
        let presentations = self.presentations();
        let mut active_rect = None;
        if let Some(workspace) = active.and_then(|id| self.controller.model().workspace(id)) {
            let layout = if self.ui.zoomed {
                pace_model::Layout::Leaf(workspace.active())
            } else {
                workspace.layout().clone()
            };
            ui::workspace::draw_node(
                ui,
                &layout,
                terminals,
                &mut self.renders,
                &presentations,
                workspace.active(),
                &self.config,
                p,
                if self.ui.search_open {
                    &self.ui.search
                } else {
                    ""
                },
                &mut actions,
                &mut active_rect,
            );
        } else {
            ui.scope_builder(
                UiBuilder::new().max_rect(Rect::from_center_size(
                    terminals.center(),
                    Vec2::new(160.0, 32.0),
                )),
                |ui| {
                    if self.startup.is_some() {
                        ui.label("Restoring workspaces…");
                    } else if ui.button("New workspace").clicked() {
                        actions.push(Action::New);
                    }
                },
            );
        }
        for action in actions.drain(..) {
            self.action(&ctx, action);
        }
        let context = if self.ui.overlay != OverlayState::None || self.ui.error.is_some() {
            crate::input::RoutingContext::Overlay
        } else if self.ui.search_open {
            crate::input::RoutingContext::TerminalSearch
        } else if ctx.memory(|m| m.focused().is_some()) {
            crate::input::RoutingContext::TextField
        } else if let Some(pane) = self.controller.model().active_pane() {
            crate::input::RoutingContext::TerminalPane(pane.get())
        } else {
            crate::input::RoutingContext::Overlay
        };
        if let Some(rect) = active_rect {
            self.terminal_input(&ctx, rect, context);
        }
        match self.ui.overlay {
            OverlayState::Settings => {
                let mut open = true;
                ui::preferences::show(&ctx, &self.config, &mut open, &mut actions);
                if !open {
                    self.ui.overlay = OverlayState::None;
                }
            }
            OverlayState::Palette => {
                let mut open = true;
                ui::palette::show(
                    &ctx,
                    &mut self.ui.palette_query,
                    &mut open,
                    self.controller.model().active_pane(),
                    p,
                    &mut actions,
                );
                if !open {
                    self.ui.overlay = OverlayState::None;
                }
            }
            _ => {}
        }
        ui::dialogs::show(&ctx, &mut self.ui, &mut actions);
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
    }
    fn on_exit(&mut self) {
        self.save_state();
        if let Some(writer) = &self.writer
            && let Err(error) = writer.flush()
        {
            eprintln!("Could not flush latest workspace state: {error}");
        }
        let complete = self.sessions.shutdown(Duration::from_secs(2));
        self.diagnostics.shutdown(complete);
    }
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

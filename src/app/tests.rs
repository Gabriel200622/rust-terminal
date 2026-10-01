use super::*;
use pace_model::WorkspaceId;

fn fixture(root: &std::path::Path) -> (App, mpsc::SyncSender<Startup>) {
    let (sender, receiver) = mpsc::sync_channel(1);
    let config = Config {
        shell: Some("/nonexistent/pace-startup-test-shell".into()),
        ..Config::default()
    };
    let app = App {
        controller: Controller::new(Model::default()),
        sessions: SessionManager::new(ResourcePolicy::default(), false),
        renders: BTreeMap::new(),
        config,
        config_path: root.join("config.toml"),
        state_path: root.join("workspaces.json"),
        window_path: root.join("window.json"),
        window_state: window_state::WindowState::default(),
        window_writable: true,
        window_generation: 0,
        restore_maximized: false,
        writer: None,
        state_writable: false,
        startup: Some(receiver),
        deferred_actions: Vec::new(),
        initial_cwd: Some(root.into()),
        initial_remote: None,
        ssh_client: "/nonexistent/pace-startup-test-ssh".into(),
        ui: UiState::default(),
        search_point: None,
        search_query: None,
        search_task: None,
        started: Instant::now() - Duration::from_secs(1),
        command: Some("printf startup".into()),
        command_target: None,
        screenshot: None,
        capture_sent: false,
        exit_approved: false,
        ephemeral: true,
        preference_generation: 0,
        ime_composing: false,
        terminal_focus: None,
        overlay_was_open: false,
        diagnostics: diagnostics::Diagnostics::new(false),
    };
    (app, sender)
}

fn loaded(config: Config, model: Model) -> Startup {
    Startup {
        config,
        report: LoadReport {
            model: Some(model),
            diagnostics: Vec::new(),
            can_write: true,
            migrated: false,
        },
        error: None,
    }
}

#[test]
fn maximized_restoration_waits_for_the_os_without_overwriting_normal_geometry() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    app.ephemeral = false;
    app.restore_maximized = true;
    app.window_state = window_state::WindowState {
        inner_size: [900.0, 640.0],
        maximized: true,
        ..Default::default()
    };
    let ctx = egui::Context::default();
    for maximized in [false, false, true] {
        let mut input = egui::RawInput::default();
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .maximized = Some(maximized);
        ctx.begin_pass(input);
        app.observe_window(&ctx, egui::vec2(1920.0, 1080.0));
        let mut output = ctx.end_pass();
        if !maximized {
            assert!(
                output.viewport_output[&egui::ViewportId::ROOT]
                    .commands
                    .contains(&egui::ViewportCommand::Maximized(true))
            );
        }
        output.textures_delta.clear();
        assert_eq!(app.window_state.inner_size, [900.0, 640.0]);
        assert!(app.window_state.maximized);
    }
    assert!(!app.restore_maximized);
}

#[test]
fn window_saves_flush_on_exit_even_when_workspace_writes_are_protected() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    app.ephemeral = false;
    let ctx = egui::Context::default();
    let mut startup = loaded(app.config.clone(), Model::default());
    startup.report.can_write = false;
    app.complete_startup(&ctx, startup);
    let mut input = egui::RawInput::default();
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .maximized = Some(false);
    ctx.begin_pass(input);
    app.observe_window(&ctx, egui::vec2(900.0, 640.0));
    ctx.end_pass().textures_delta.clear();
    let mut input = egui::RawInput::default();
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .maximized = Some(true);
    ctx.begin_pass(input);
    app.observe_window(&ctx, egui::vec2(1920.0, 1080.0));
    ctx.end_pass().textures_delta.clear();
    eframe::App::on_exit(&mut app);
    let saved = window_state::load(&app.window_path);
    assert_eq!(saved.state.inner_size, [900.0, 640.0]);
    assert!(saved.state.maximized);
    assert!(!app.state_path.exists());
}

#[test]
fn protected_window_state_does_not_block_preferences_saves() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    app.ephemeral = false;
    app.window_writable = false;
    let original = br#"{"version":999,"opaque":"future window"}"#;
    std::fs::write(&app.window_path, original).unwrap();
    let ctx = egui::Context::default();
    app.complete_startup(&ctx, loaded(app.config.clone(), Model::default()));
    let config = Config {
        font_size: 20.0,
        ..app.config.clone()
    };
    app.action(&ctx, Action::Preferences(config));
    eframe::App::on_exit(&mut app);
    assert_eq!(std::fs::read(&app.window_path).unwrap(), original);
    assert_eq!(Config::load(&app.config_path).unwrap().font_size, 20.0);
}

#[test]
fn startup_actions_wait_then_replay_on_restored_state_without_retargeting_the_cli_command() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    let ctx = egui::Context::default();
    let mut restored = Controller::new(Model::default());
    restored
        .dispatch(Command::AddWorkspace {
            cwd: root.path().into(),
            name: "Restored".into(),
            remote: None,
        })
        .unwrap();
    let original = restored.model().active_pane().unwrap();
    let sidebar = restored.model().sidebar();
    app.action(&ctx, Action::New);
    app.action(&ctx, Action::ToggleSidebar);
    assert!(app.controller.model().workspaces().is_empty());
    assert_eq!(app.sessions.usage().starting, 0);
    app.complete_startup(&ctx, loaded(app.config.clone(), restored.model().clone()));
    assert_eq!(app.controller.model().workspaces().len(), 2);
    assert_eq!(app.controller.model().workspaces()[0].name(), "Restored");
    assert_eq!(
        app.controller.model().workspaces()[1].cwd(),
        directories::BaseDirs::new().unwrap().home_dir()
    );
    assert_eq!(app.ui.overlay, OverlayState::None);
    assert_eq!(app.controller.model().sidebar(), !sidebar);
    assert_eq!(app.command_target, Some((original, 1)));
    assert_ne!(app.controller.model().active_pane(), Some(original));
    assert!(app.deferred_actions.is_empty());
}

#[test]
fn startup_queue_is_bounded_and_does_not_start_sessions_early() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    let ctx = egui::Context::default();
    for _ in 0..25 {
        app.action(&ctx, Action::New);
    }
    assert_eq!(app.deferred_actions.len(), 24);
    assert!(app.controller.model().workspaces().is_empty());
    assert_eq!(app.sessions.usage().starting, 0);
    assert!(app.ui.error.as_deref().unwrap().contains("Too many"));
}

#[test]
fn cancelling_close_preserves_state_and_confirmation_keeps_the_original_target() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    let ctx = egui::Context::default();
    app.startup = None;
    for name in ["First", "Second"] {
        app.controller
            .dispatch(Command::AddWorkspace {
                cwd: root.path().into(),
                name: name.into(),
                remote: None,
            })
            .unwrap();
    }
    let original = app.controller.model().workspaces()[0].active();
    let other = app.controller.model().active_pane().unwrap();
    let generation = app.controller.generation();
    app.action(&ctx, Action::ClosePane(original));
    assert_eq!(
        app.ui.overlay,
        OverlayState::ConfirmClose(Close::Pane(original))
    );
    app.action(&ctx, Action::CancelClose);
    assert_eq!(app.controller.generation(), generation);
    assert_eq!(app.controller.model().workspaces().len(), 2);
    assert_eq!(app.controller.model().active_pane(), Some(other));
    assert_eq!(app.ui.overlay, OverlayState::None);
    app.action(&ctx, Action::ClosePane(original));
    app.action(&ctx, Action::Confirm(Close::Pane(original)));
    assert!(app.controller.model().pane(original).is_none());
    assert!(app.controller.model().pane(other).is_some());
    assert_eq!(app.controller.model().active_pane(), Some(other));
    assert_eq!(app.controller.model().workspaces().len(), 1);
}

#[test]
fn pending_preferences_coalesce_without_closing_settings() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    let ctx = egui::Context::default();
    app.ui.overlay = OverlayState::Settings;
    let mut config = app.config.clone();
    config.font_size = 18.0;
    app.action(&ctx, Action::Preferences(config.clone()));
    config.font_size = 20.0;
    app.action(&ctx, Action::Preferences(config));
    assert_eq!(app.deferred_actions.len(), 1);
    assert_eq!(app.ui.overlay, OverlayState::Settings);
    app.complete_startup(&ctx, loaded(app.config.clone(), Model::default()));
    assert_eq!(app.config.font_size, 20.0);
    assert_eq!(app.preference_generation, 1);
}

#[test]
fn failed_restoration_keeps_storage_read_only_and_replays_user_actions() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, sender) = fixture(root.path());
    let ctx = egui::Context::default();
    app.action(
        &ctx,
        Action::Create(root.path().into(), Some("Queued".into())),
    );
    drop(sender);
    app.poll(&ctx);
    assert!(app.startup.is_none());
    assert!(!app.state_writable);
    assert_eq!(app.controller.model().workspaces().len(), 2);
    assert_eq!(app.controller.model().workspaces()[1].name(), "Queued");
    assert!(app.deferred_actions.is_empty());
    assert!(!app.state_path.exists());
}

#[test]
fn startup_command_is_cancelled_if_its_original_pane_is_closed_restarted_or_failed() {
    let root = tempfile::tempdir().unwrap();
    for operation in 0..3 {
        let (mut app, _sender) = fixture(root.path());
        let ctx = egui::Context::default();
        app.complete_startup(&ctx, loaded(app.config.clone(), Model::default()));
        let (pane, _) = app.command_target.unwrap();
        app.dispatch(
            &ctx,
            match operation {
                0 => Command::ClosePane(pane),
                1 => Command::RestartPane(pane),
                _ => Command::SessionFailed {
                    pane,
                    generation: app.command_target.unwrap().1,
                    error: "Missing shell".into(),
                },
            },
        );
        app.send_startup_command();
        assert!(app.command.is_none());
        assert!(
            app.ui
                .error
                .as_deref()
                .unwrap()
                .contains("Startup command cancelled")
        );
    }
}

#[cfg(unix)]
#[test]
fn startup_command_reaches_the_original_pty_once_after_focus_changes() {
    let root = tempfile::tempdir().unwrap();
    let first = root.path().join("first");
    let second = root.path().join("second");
    std::fs::create_dir(&first).unwrap();
    std::fs::create_dir(&second).unwrap();
    let (mut app, _sender) = fixture(root.path());
    let ctx = egui::Context::default();
    app.config.shell = Some("/bin/sh".into());
    app.initial_cwd = Some(first.clone());
    // A shell can create a redirection target before writing its contents.
    app.command = Some(": >> launch-marker; sleep 0.1; printf x >> launch-marker".into());
    app.action(&ctx, Action::Create(second.clone(), Some("Second".into())));
    app.complete_startup(&ctx, loaded(app.config.clone(), Model::default()));
    let (target, _) = app.command_target.unwrap();
    assert_ne!(app.controller.model().active_pane(), Some(target));
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.sessions.usage().running != 2 {
        app.poll(&ctx);
        assert!(
            Instant::now() < deadline,
            "sessions failed to start: {:?}",
            app.ui.error
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    app.send_startup_command();
    app.send_startup_command();
    while !std::fs::read(first.join("launch-marker")).is_ok_and(|contents| !contents.is_empty()) {
        assert!(
            Instant::now() < deadline,
            "startup command did not reach its target"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(std::fs::read(first.join("launch-marker")).unwrap(), b"x");
    assert!(!second.join("launch-marker").exists());
    assert!(app.command.is_none());
}

fn press(app: &mut App, ctx: &egui::Context, event: egui::Event) -> bool {
    let mut consumed = false;
    let key = match &event {
        egui::Event::Key { key, .. } => *key,
        _ => unreachable!("key events only"),
    };
    let mut output = ctx.run_ui(
        egui::RawInput {
            events: vec![event],
            ..Default::default()
        },
        |ui| {
            app.shortcuts(ui.ctx());
            consumed = !ui.input(|input| input.key_pressed(key));
        },
    );
    output.textures_delta.clear();
    consumed
}

fn key(key: egui::Key, physical_key: Option<egui::Key>, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn navigation_fixture(root: &std::path::Path) -> (App, [PaneId; 4]) {
    let (mut app, _sender) = fixture(root);
    app.startup = None;
    app.controller
        .dispatch(Command::AddWorkspace {
            cwd: root.into(),
            name: "Navigation".into(),
            remote: None,
        })
        .unwrap();
    let workspace = app.controller.model().active_workspace().unwrap();
    let top_left = app.controller.model().active_pane().unwrap();
    app.controller
        .dispatch(Command::SplitPane {
            workspace,
            pane: top_left,
            axis: pace_model::Axis::Vertical,
            cwd: root.into(),
        })
        .unwrap();
    let top_right = app.controller.model().active_pane().unwrap();
    let mut bottoms = Vec::new();
    for pane in [top_left, top_right] {
        app.controller
            .dispatch(Command::SplitPane {
                workspace,
                pane,
                axis: pace_model::Axis::Horizontal,
                cwd: root.into(),
            })
            .unwrap();
        bottoms.push(app.controller.model().active_pane().unwrap());
    }
    app.controller
        .dispatch(Command::FocusPane {
            workspace,
            pane: top_left,
        })
        .unwrap();
    (app, [top_left, top_right, bottoms[0], bottoms[1]])
}

#[test]
fn pane_navigation_shortcuts_move_focus_in_all_directions_and_while_zoomed() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, [top_left, top_right, bottom_left, bottom_right]) =
        navigation_fixture(root.path());
    let ctx = egui::Context::default();
    app.ui.zoomed = true;
    app.ui.search_error = Some("No matches".into());
    // Navigation also reaches panes whose shell failed or exited, and ones
    // still starting. Focus does not depend on having a live session handle.
    app.controller
        .dispatch(Command::SessionFailed {
            pane: top_right,
            generation: 1,
            error: "Shell unavailable".into(),
        })
        .unwrap();
    app.controller
        .dispatch(Command::SessionExited {
            pane: bottom_right,
            generation: 1,
        })
        .unwrap();
    for (arrow, target) in [
        (egui::Key::ArrowRight, top_right),
        (egui::Key::ArrowDown, bottom_right),
        (egui::Key::ArrowLeft, bottom_left),
        (egui::Key::ArrowUp, top_left),
    ] {
        assert!(press(
            &mut app,
            &ctx,
            key(arrow, None, egui::Modifiers::CTRL | egui::Modifiers::SHIFT)
        ));
        assert_eq!(app.controller.model().active_pane(), Some(target));
    }
    assert!(app.ui.zoomed);
    assert!(app.ui.search_error.is_none());
}

#[test]
fn pane_navigation_consumes_press_and_release_at_an_outer_edge_without_mutating_state() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, [top_left, ..]) = navigation_fixture(root.path());
    let ctx = egui::Context::default();
    let generation = app.controller.generation();
    let down = key(
        egui::Key::ArrowLeft,
        None,
        egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
    );
    let mut up = down.clone();
    if let egui::Event::Key { pressed, .. } = &mut up {
        *pressed = false;
    }
    let mut output = ctx.run_ui(
        egui::RawInput {
            events: vec![down, up],
            ..Default::default()
        },
        |ui| {
            app.shortcuts(ui.ctx());
            assert!(
                ui.input(|input| input.events.is_empty()),
                "neither event reaches the shell"
            );
        },
    );
    output.textures_delta.clear();
    assert_eq!(app.controller.model().active_pane(), Some(top_left));
    assert_eq!(app.controller.generation(), generation);
}

#[test]
fn pane_navigation_preserves_overlay_and_editable_field_ownership() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, [top_left, ..]) = navigation_fixture(root.path());
    let ctx = egui::Context::default();
    for overlay in [
        OverlayState::Palette,
        OverlayState::Settings,
        OverlayState::Rename(app.controller.model().active_workspace().unwrap()),
        OverlayState::ConfirmClose(ui::Close::Pane(top_left)),
    ] {
        app.ui.overlay = overlay;
        assert!(!press(
            &mut app,
            &ctx,
            key(
                egui::Key::ArrowRight,
                None,
                egui::Modifiers::CTRL | egui::Modifiers::SHIFT
            )
        ));
        assert_eq!(app.controller.model().active_pane(), Some(top_left));
    }
    app.ui.overlay = OverlayState::None;
    for field in [ui::search::input_id(), egui::Id::new("editable-field")] {
        ctx.memory_mut(|memory| memory.request_focus(field));
        assert!(!press(
            &mut app,
            &ctx,
            key(
                egui::Key::ArrowRight,
                None,
                egui::Modifiers::CTRL | egui::Modifiers::SHIFT
            )
        ));
        assert_eq!(app.controller.model().active_pane(), Some(top_left));
    }
}

#[test]
fn pane_navigation_leaves_other_arrow_chords_for_the_shell() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, [top_left, ..]) = navigation_fixture(root.path());
    let ctx = egui::Context::default();
    for modifiers in [
        egui::Modifiers::NONE,
        egui::Modifiers::CTRL,
        egui::Modifiers::SHIFT,
        egui::Modifiers::CTRL | egui::Modifiers::SHIFT | egui::Modifiers::ALT,
    ] {
        assert!(!press(
            &mut app,
            &ctx,
            key(egui::Key::ArrowRight, None, modifiers)
        ));
        assert_eq!(app.controller.model().active_pane(), Some(top_left));
    }
}

#[test]
fn pane_navigation_repeated_keys_in_one_frame_advance_from_the_new_focus() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, [top_left, _, _, bottom_right]) = navigation_fixture(root.path());
    let ctx = egui::Context::default();
    let mut output = ctx.run_ui(
        egui::RawInput {
            events: vec![
                key(
                    egui::Key::ArrowRight,
                    None,
                    egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                ),
                key(
                    egui::Key::ArrowDown,
                    None,
                    egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                ),
            ],
            ..Default::default()
        },
        |ui| app.shortcuts(ui.ctx()),
    );
    output.textures_delta.clear();
    assert_ne!(app.controller.model().active_pane(), Some(top_left));
    assert_eq!(app.controller.model().active_pane(), Some(bottom_right));
}

#[test]
fn app_zoom_shortcuts_scale_the_ui_without_changing_terminal_preferences() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    let ctx = egui::Context::default();
    app.startup = None;
    app.config.font_size = 19.0;
    for modifiers in [
        egui::Modifiers::CTRL,
        egui::Modifiers::MAC_CMD,
        egui::Modifiers::MAC_CMD | egui::Modifiers::SHIFT,
    ] {
        for overlay in [
            OverlayState::None,
            OverlayState::Settings,
            OverlayState::Palette,
        ] {
            app.ui.overlay = overlay;
            let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
            output.textures_delta.clear();
            assert!(press(&mut app, &ctx, key(egui::Key::Plus, None, modifiers)));
            let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
            output.textures_delta.clear();
            assert_eq!(ctx.zoom_factor(), 1.1);
            assert_eq!(app.config.font_size, 19.0);
            assert_eq!(app.preference_generation, 0);

            let primary = egui::Modifiers {
                shift: false,
                ..modifiers
            };
            assert!(press(&mut app, &ctx, key(egui::Key::Minus, None, primary)));
            let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
            output.textures_delta.clear();
            assert_eq!(ctx.zoom_factor(), 1.0);

            assert!(press(&mut app, &ctx, key(egui::Key::Equals, None, primary)));
            let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
            output.textures_delta.clear();
            assert_eq!(ctx.zoom_factor(), 1.1);
            assert!(press(&mut app, &ctx, key(egui::Key::Num0, None, primary)));
            let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
            output.textures_delta.clear();
            assert_eq!(ctx.zoom_factor(), 1.0);
        }
    }
}

#[test]
fn terminal_font_shortcuts_use_ctrl_shift_in_preferences_and_respect_limits() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    let ctx = egui::Context::default();
    app.startup = None;
    app.ui.overlay = OverlayState::Settings;
    let modifiers = egui::Modifiers::CTRL | egui::Modifiers::SHIFT;
    for (key_code, size) in [
        (egui::Key::Plus, 15.0),
        (egui::Key::Equals, 16.0),
        (egui::Key::Minus, 15.0),
    ] {
        assert!(press(&mut app, &ctx, key(key_code, None, modifiers)));
        assert_eq!(app.config.font_size, size);
        assert_eq!(ctx.zoom_factor(), 1.0);
    }
    for (size, key_code) in [(32.0, egui::Key::Plus), (9.0, egui::Key::Minus)] {
        app.config.font_size = size;
        assert!(press(&mut app, &ctx, key(key_code, None, modifiers)));
        assert_eq!(app.config.font_size, size);
    }
    ctx.set_zoom_factor(1.4);
    let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
    output.textures_delta.clear();
    assert!(press(&mut app, &ctx, key(egui::Key::Num0, None, modifiers)));
    assert_eq!(app.config.font_size, Config::default().font_size);
    assert_eq!(ctx.zoom_factor(), 1.4);
}

#[test]
fn zoom_shortcuts_consume_key_and_text_events_before_terminal_input() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    let ctx = egui::Context::default();
    app.startup = None;
    for (key_code, modifiers, text) in [
        (egui::Key::Plus, egui::Modifiers::CTRL, "+"),
        (egui::Key::Equals, egui::Modifiers::CTRL, "="),
        (egui::Key::Minus, egui::Modifiers::CTRL, "-"),
        (egui::Key::Num0, egui::Modifiers::CTRL, "0"),
        (
            egui::Key::Plus,
            egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
            "+",
        ),
        (
            egui::Key::Minus,
            egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
            "_",
        ),
        (
            egui::Key::Num0,
            egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
            ")",
        ),
    ] {
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![
                    key(key_code, None, modifiers),
                    egui::Event::Text(text.into()),
                    egui::Event::Text("other input".into()),
                ],
                ..Default::default()
            },
            |ui| {
                app.shortcuts(ui.ctx());
                assert_eq!(
                    ui.input(|input| input.events.clone()),
                    [egui::Event::Text("other input".into())]
                );
            },
        );
        output.textures_delta.clear();
    }
}

#[test]
fn ordinary_and_alt_modified_keys_keep_their_input_owner() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    let ctx = egui::Context::default();
    ctx.options_mut(|options| options.zoom_with_keyboard = false);
    app.startup = None;
    for modifiers in [
        egui::Modifiers::NONE,
        egui::Modifiers::CTRL | egui::Modifiers::ALT,
    ] {
        for key_code in [
            egui::Key::Plus,
            egui::Key::Equals,
            egui::Key::Minus,
            egui::Key::Num0,
        ] {
            assert!(!press(&mut app, &ctx, key(key_code, None, modifiers)));
        }
    }
    assert_eq!(ctx.zoom_factor(), 1.0);
    assert_eq!(app.config.font_size, 14.0);
    assert_eq!(app.preference_generation, 0);
}

#[test]
fn escape_leaves_one_surface_at_a_time_and_otherwise_belongs_to_the_shell() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    let ctx = egui::Context::default();
    let escape = || key(egui::Key::Escape, None, egui::Modifiers::NONE);
    app.startup = None;
    app.ui.overlay = OverlayState::Settings;
    app.ui.search_open = true;
    app.ui.error = Some("Storage is read-only".into());

    assert!(press(&mut app, &ctx, escape()));
    assert_eq!(app.ui.overlay, OverlayState::None);
    assert!(app.ui.search_open && app.ui.error.is_some());

    // With no terminal to receive it, Escape dismisses the message; search,
    // whose field is not focused, stays.
    assert!(press(&mut app, &ctx, escape()));
    assert!(app.ui.error.is_none() && app.ui.search_open);

    app.controller
        .dispatch(Command::AddWorkspace {
            cwd: root.path().into(),
            name: "Shell".into(),
            remote: None,
        })
        .unwrap();
    assert!(app.controller.model().active_pane().is_some());

    // A message never takes a key the shell is waiting for.
    app.ui.error = Some("Storage is read-only".into());
    assert!(!press(&mut app, &ctx, escape()));
    assert!(app.ui.error.is_some() && app.ui.search_open);

    ctx.memory_mut(|memory| memory.request_focus(ui::search::input_id()));
    assert!(press(&mut app, &ctx, escape()));
    assert!(!app.ui.search_open);
    assert!(app.ui.error.is_some(), "one surface per Escape");
}

#[test]
fn escape_cancels_a_terminal_drag_instead_of_reaching_the_shell() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    let ctx = egui::Context::default();
    let escape = || key(egui::Key::Escape, None, egui::Modifiers::NONE);
    app.startup = None;
    app.controller
        .dispatch(Command::AddWorkspace {
            cwd: root.path().into(),
            name: "Shell".into(),
            remote: None,
        })
        .unwrap();
    let generation = app.controller.generation();
    app.ui.pane_drag = app.controller.model().active_pane();

    assert!(press(&mut app, &ctx, escape()));
    assert!(app.ui.pane_drag.is_none());
    assert_eq!(app.controller.generation(), generation);
    // With nothing left to cancel, the key is the shell's again.
    assert!(!press(&mut app, &ctx, escape()));
}

#[cfg(unix)]
#[test]
fn a_terminal_moved_to_another_workspace_keeps_its_running_shell() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    let ctx = egui::Context::default();
    app.startup = None;
    app.config.shell = Some("/bin/sh".into());
    let add = |app: &mut App, name: &str| {
        app.dispatch(
            &ctx,
            Command::AddWorkspace {
                cwd: root.path().into(),
                name: name.into(),
                remote: None,
            },
        );
        app.controller.model().active_workspace().unwrap()
    };
    let home = add(&mut app, "Home");
    let stays = app.controller.model().active_pane().unwrap();
    app.action(&ctx, Action::Split(stays, pace_model::Axis::Vertical));
    let moved = app.controller.model().active_pane().unwrap();
    let other = add(&mut app, "Other");
    app.action(&ctx, Action::SelectWorkspace(home));
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.sessions.usage().running != 3 {
        app.poll(&ctx);
        assert!(
            Instant::now() < deadline,
            "sessions failed to start: {:?}",
            app.ui.error
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let process = app.sessions.get(moved).unwrap().metadata().process_id;
    assert!(process.is_some());

    app.action(
        &ctx,
        Action::MovePane(moved, pace_model::Destination::Workspace(other)),
    );
    let model = app.controller.model();
    assert_eq!(model.workspace_for_pane(moved), Some(other));
    assert_eq!(model.active_workspace(), Some(home));
    assert_eq!(model.active_pane(), Some(stays));
    assert_eq!(model.pane(moved).unwrap().generation(), 1);
    assert!(app.ui.error.is_none(), "{:?}", app.ui.error);

    // The same process, still accepting input while its workspace is hidden.
    let usage = app.sessions.usage();
    assert_eq!((usage.running, usage.starting, usage.closing), (3, 0, 0));
    assert!(app.renders.contains_key(&moved));
    let session = app.sessions.get(moved).unwrap();
    assert_eq!(session.metadata().process_id, process);
    session.write(b": > moved-marker\r").unwrap();
    while !root.path().join("moved-marker").exists() {
        assert!(
            Instant::now() < deadline,
            "the moved shell stopped accepting input"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn find_returns_to_an_open_search_field_before_it_closes() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    let ctx = egui::Context::default();
    app.startup = None;
    app.action(&ctx, Action::Find);
    assert!(app.ui.search_open && app.ui.search_focus);

    // The field took focus and the user then clicked back into the terminal.
    app.ui.search_focus = false;
    app.action(&ctx, Action::Find);
    assert!(app.ui.search_open, "search stays open");
    assert!(app.ui.search_focus, "the field is focused again");

    ctx.memory_mut(|memory| memory.request_focus(ui::search::input_id()));
    app.action(&ctx, Action::Find);
    assert!(!app.ui.search_open);
}

#[test]
fn toggling_the_sidebar_starts_a_slide_from_where_it_is() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    let ctx = egui::Context::default();
    app.startup = None;
    assert!(app.controller.model().sidebar());
    assert_eq!(
        app.ui.sidebar_slide, None,
        "a sidebar that was not toggled rests in place"
    );

    app.action(&ctx, Action::ToggleSidebar);
    assert!(!app.controller.model().sidebar());
    let hide = app.ui.sidebar_slide.unwrap();
    assert_eq!(hide.reveal(false, 0.0), Some(1.0));
    assert_eq!(hide.reveal(false, 1.0), None);

    // Toggled back before the first slide moved: it starts fully shown.
    app.action(&ctx, Action::ToggleSidebar);
    assert!(app.controller.model().sidebar());
    assert_eq!(app.ui.sidebar_slide.unwrap().reveal(true, 0.0), Some(1.0));
}

#[test]
fn sidebar_width_is_clamped_and_saved_as_one_preference_change() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    let ctx = egui::Context::default();
    app.startup = None;
    app.action(&ctx, Action::SidebarWidth(900.0));
    assert_eq!(app.config.sidebar_width, 360.0);
    app.action(&ctx, Action::SidebarWidth(f32::NAN));
    assert_eq!(
        app.config.sidebar_width, 360.0,
        "invalid widths are refused"
    );
    app.action(&ctx, Action::SidebarWidth(12.0));
    assert_eq!(app.config.sidebar_width, 170.0);
    assert_eq!(app.preference_generation, 2);
}

#[test]
fn rename_requests_field_focus_and_cancels_without_touching_workspaces() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    let ctx = egui::Context::default();
    app.startup = None;
    app.controller
        .dispatch(Command::AddWorkspace {
            cwd: root.path().into(),
            name: "Only".into(),
            remote: None,
        })
        .unwrap();
    let workspace = app.controller.model().active_workspace().unwrap();
    let generation = app.controller.generation();

    app.action(&ctx, Action::Rename(workspace));
    assert_eq!(app.ui.overlay, OverlayState::Rename(workspace));
    assert!(app.ui.overlay_focus);
    assert_eq!(app.ui.rename_name, "Only");
    app.action(&ctx, Action::CloseOverlay);
    assert_eq!(app.ui.overlay, OverlayState::None);

    app.ui.error = Some("Could not save".into());
    app.action(&ctx, Action::DismissError);
    assert!(app.ui.error.is_none());
    assert_eq!(app.controller.generation(), generation);
    assert_eq!(app.controller.model().workspaces().len(), 1);
}

#[test]
fn new_workspace_opens_at_home_without_a_dialog_and_can_be_renamed() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    let ctx = egui::Context::default();
    app.startup = None;
    app.controller
        .dispatch(Command::AddWorkspace {
            cwd: root.path().into(),
            name: "Existing".into(),
            remote: None,
        })
        .unwrap();
    let existing = app.controller.model().active_workspace().unwrap();
    app.ui.overlay = OverlayState::Palette;

    app.action(&ctx, Action::New);

    let workspace = app.controller.model().active_workspace().unwrap();
    let pane = app.controller.model().active_pane().unwrap();
    let home = directories::BaseDirs::new().unwrap();
    assert_ne!(workspace, existing);
    assert_eq!(app.controller.model().workspaces().len(), 2);
    assert_eq!(
        app.controller.model().workspace(workspace).unwrap().cwd(),
        home.home_dir()
    );
    assert_eq!(
        app.controller.model().pane(pane).unwrap().cwd(),
        home.home_dir()
    );
    assert_eq!(app.ui.overlay, OverlayState::None);
    assert_eq!(app.sessions.usage().starting, 1);

    app.action(&ctx, Action::Rename(workspace));
    assert_eq!(app.ui.overlay, OverlayState::Rename(workspace));
    assert!(app.ui.overlay_focus);
    // A later focus change cannot retarget the rename.
    app.action(&ctx, Action::SelectWorkspace(existing));
    app.action(&ctx, Action::SetName(workspace, "Renamed".into()));
    app.action(&ctx, Action::CloseOverlay);
    let renamed = app.controller.model().workspace(workspace).unwrap();
    assert_eq!(renamed.name(), "Renamed");
    assert_eq!(renamed.cwd(), home.home_dir());
    assert_eq!(renamed.active(), pane);
    assert_eq!(
        app.controller.model().workspace(existing).unwrap().name(),
        "Existing"
    );
}

#[test]
fn new_workspace_shortcut_works_with_no_existing_workspace() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    let ctx = egui::Context::default();
    app.startup = None;
    let command = if cfg!(target_os = "macos") {
        egui::Modifiers::MAC_CMD | egui::Modifiers::COMMAND
    } else {
        egui::Modifiers::CTRL | egui::Modifiers::SHIFT
    };

    assert!(press(&mut app, &ctx, key(egui::Key::T, None, command)));
    let workspace = app.controller.model().workspaces().first().unwrap();
    assert_eq!(
        workspace.cwd(),
        directories::BaseDirs::new().unwrap().home_dir()
    );
    assert_eq!(
        app.controller.model().active_workspace(),
        Some(workspace.id())
    );
    assert_eq!(app.ui.overlay, OverlayState::None);
}

#[test]
fn command_digits_select_workspaces_by_position_even_when_shift_changes_the_symbol() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    let ctx = egui::Context::default();
    app.startup = None;
    for name in ["First", "Second", "Third"] {
        app.controller
            .dispatch(Command::AddWorkspace {
                cwd: root.path().into(),
                name: name.into(),
                remote: None,
            })
            .unwrap();
    }
    let ids: Vec<_> = app
        .controller
        .model()
        .workspaces()
        .iter()
        .map(|workspace| workspace.id())
        .collect();
    assert_eq!(app.controller.model().active_workspace(), Some(ids[2]));
    let command = if cfg!(target_os = "macos") {
        egui::Modifiers::MAC_CMD | egui::Modifiers::COMMAND
    } else {
        egui::Modifiers::CTRL | egui::Modifiers::SHIFT
    };
    // On a US layout Shift+1 arrives as "!" with the digit as its physical key.
    assert!(press(
        &mut app,
        &ctx,
        key(egui::Key::Exclamationmark, Some(egui::Key::Num1), command)
    ));
    assert_eq!(app.controller.model().active_workspace(), Some(ids[0]));
    assert!(press(&mut app, &ctx, key(egui::Key::Num2, None, command)));
    assert_eq!(app.controller.model().active_workspace(), Some(ids[1]));
    // No ninth workspace: the key is left for the terminal and nothing changes.
    assert!(!press(&mut app, &ctx, key(egui::Key::Num9, None, command)));
    assert_eq!(app.controller.model().active_workspace(), Some(ids[1]));
    // A plain digit is ordinary typing.
    assert!(!press(
        &mut app,
        &ctx,
        key(egui::Key::Num1, None, egui::Modifiers::NONE)
    ));
    assert_eq!(app.controller.model().active_workspace(), Some(ids[1]));
}

#[test]
fn moving_a_workspace_keeps_focus_and_renumbers_the_position_shortcuts() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    let ctx = egui::Context::default();
    app.startup = None;
    for name in ["First", "Second", "Third"] {
        app.controller
            .dispatch(Command::AddWorkspace {
                cwd: root.path().into(),
                name: name.into(),
                remote: None,
            })
            .unwrap();
    }
    let names = |app: &App| -> Vec<String> {
        let workspaces = app.controller.model().workspaces();
        workspaces.iter().map(|w| w.name().to_owned()).collect()
    };
    let third = app.controller.model().active_workspace().unwrap();
    let pane = app.controller.model().active_pane();
    app.action(&ctx, Action::MoveWorkspace(third, 0));
    assert_eq!(names(&app), ["Third", "First", "Second"]);
    assert_eq!(app.controller.model().active_workspace(), Some(third));
    assert_eq!(app.controller.model().active_pane(), pane);
    assert!(app.ui.error.is_none());

    let command = if cfg!(target_os = "macos") {
        egui::Modifiers::MAC_CMD | egui::Modifiers::COMMAND
    } else {
        egui::Modifiers::CTRL | egui::Modifiers::SHIFT
    };
    assert!(press(&mut app, &ctx, key(egui::Key::Num2, None, command)));
    let active = app.controller.model().active_workspace().unwrap();
    assert_eq!(
        app.controller.model().workspace(active).unwrap().name(),
        "First"
    );

    // A workspace closed before its queued move arrives is reported, not moved.
    app.controller
        .dispatch(Command::CloseWorkspace(third))
        .unwrap();
    app.action(&ctx, Action::MoveWorkspace(third, 1));
    assert_eq!(names(&app), ["First", "Second"]);
    assert!(app.ui.error.is_some());
}

fn add_workspace(app: &mut App, root: &std::path::Path, remote: Option<&str>) -> WorkspaceId {
    // Dispatched on the controller alone, so no session is started.
    app.controller
        .dispatch(Command::AddWorkspace {
            cwd: root.into(),
            name: "Workspace".into(),
            remote: remote.map(str::to_owned),
        })
        .unwrap();
    app.controller.model().active_workspace().unwrap()
}

fn remote_of(app: &App, workspace: WorkspaceId) -> Option<&str> {
    app.controller
        .model()
        .workspace(workspace)
        .unwrap()
        .remote()
        .map(|remote| remote.destination())
}

#[test]
fn a_remote_terminal_runs_the_ssh_client_with_a_pty_and_a_quoted_bootstrap() {
    let config = Config {
        shell: Some("/bin/zsh".into()),
        scrollback: 500,
        ..Config::default()
    };
    let remote = Remote::parse("me@devbox").unwrap();
    let options =
        coordinator::session_options(&config, "ssh", "/srv/app".into(), Some(&remote), None);
    assert_eq!(options.shell.as_deref(), Some("ssh"));
    // `--` ends option parsing, so the destination can only be a host.
    assert_eq!(options.args[..3], ["-t", "--", "me@devbox"]);
    assert_eq!(options.args.len(), 4);
    assert!(options.args[3].starts_with("sh -c '"));
    assert_eq!(options.cwd, std::path::Path::new("/srv/app"));
    assert_eq!(options.scrollback, 500);

    let local = coordinator::session_options(&config, "ssh", "/srv/app".into(), None, None);
    assert_eq!(local.shell.as_deref(), Some("/bin/zsh"));
    assert!(local.args.is_empty());
}

#[test]
fn the_ssh_sheet_opens_for_local_workspaces_and_connecting_replaces_their_terminals() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    let ctx = egui::Context::default();
    app.startup = None;
    app.action(&ctx, Action::Create(root.path().into(), None));
    let workspace = app.controller.model().active_workspace().unwrap();
    let pane = app.controller.model().active_pane().unwrap();
    app.ui.ssh_host = "stale".into();

    app.action(&ctx, Action::Ssh(Some(workspace)));
    assert_eq!(app.ui.overlay, OverlayState::Ssh(Some(workspace)));
    assert!(app.ui.overlay_focus);
    assert!(app.ui.ssh_host.is_empty());

    app.action(
        &ctx,
        Action::Connect {
            workspace: Some(workspace),
            destination: "me@devbox".into(),
        },
    );
    assert_eq!(app.ui.overlay, OverlayState::None);
    assert_eq!(remote_of(&app, workspace), Some("me@devbox"));
    assert_eq!(app.controller.model().pane(pane).unwrap().generation(), 2);
    assert_eq!(app.sessions.generation(pane), Some(2));
    assert_eq!(app.controller.model().workspaces().len(), 1);

    // A connected workspace is disconnected before it can be pointed elsewhere.
    app.action(&ctx, Action::Ssh(Some(workspace)));
    assert_eq!(app.ui.overlay, OverlayState::None);
    // A workspace that no longer exists has nothing to connect.
    app.action(&ctx, Action::Ssh(Some(WorkspaceId::new(99))));
    assert_eq!(app.ui.overlay, OverlayState::None);

    app.action(&ctx, Action::Ssh(None));
    assert_eq!(app.ui.overlay, OverlayState::Ssh(None));
    app.action(
        &ctx,
        Action::Connect {
            workspace: None,
            destination: "ssh://me@buildbox".into(),
        },
    );
    assert_eq!(app.ui.overlay, OverlayState::None);
    let created = app.controller.model().workspaces().last().unwrap();
    assert_eq!(created.name(), "buildbox");
    assert_eq!(
        created.remote().map(|remote| remote.destination()),
        Some("ssh://me@buildbox")
    );
    assert_eq!(
        app.controller.model().active_workspace(),
        Some(created.id())
    );
}

#[test]
fn disconnecting_asks_first_and_cancelling_keeps_the_connection() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    let ctx = egui::Context::default();
    app.startup = None;
    let connect = |app: &mut App| {
        app.action(
            &ctx,
            Action::Connect {
                workspace: None,
                destination: "me@devbox".into(),
            },
        );
        app.controller.model().active_workspace().unwrap()
    };
    let workspace = connect(&mut app);
    let pane = app.controller.model().active_pane().unwrap();
    let generation = app.controller.generation();

    app.action(&ctx, Action::Disconnect(workspace));
    assert_eq!(
        app.ui.overlay,
        OverlayState::ConfirmClose(Close::Connection(workspace))
    );
    app.action(&ctx, Action::CancelClose);
    assert_eq!(app.ui.overlay, OverlayState::None);
    assert_eq!(remote_of(&app, workspace), Some("me@devbox"));
    assert_eq!(app.controller.generation(), generation);
    assert_eq!(app.controller.model().pane(pane).unwrap().generation(), 1);

    app.action(&ctx, Action::Disconnect(workspace));
    app.action(&ctx, Action::Confirm(Close::Connection(workspace)));
    assert_eq!(app.ui.overlay, OverlayState::None);
    assert_eq!(remote_of(&app, workspace), None);
    assert_eq!(app.controller.model().pane(pane).unwrap().generation(), 2);
    assert_eq!(app.sessions.generation(pane), Some(2));
    assert_eq!(app.controller.model().workspaces().len(), 1);

    // Without confirmation the same action applies at once.
    let workspace = connect(&mut app);
    app.config.confirm_close = false;
    app.action(&ctx, Action::Disconnect(workspace));
    assert_eq!(app.ui.overlay, OverlayState::None);
    assert_eq!(remote_of(&app, workspace), None);
}

#[test]
fn a_connection_requested_during_restoration_waits_and_the_cli_host_opens_first() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    let ctx = egui::Context::default();
    app.command = None;
    app.initial_remote = Some("me@devbox".into());
    app.ui.overlay = OverlayState::Ssh(None);
    app.action(
        &ctx,
        Action::Connect {
            workspace: None,
            destination: "buildbox".into(),
        },
    );
    assert_eq!(app.ui.overlay, OverlayState::None);
    assert!(app.controller.model().workspaces().is_empty());
    assert_eq!(app.sessions.usage().starting, 0);

    app.complete_startup(&ctx, loaded(app.config.clone(), Model::default()));
    let workspaces = app.controller.model().workspaces();
    assert_eq!(workspaces.len(), 2);
    assert_eq!(workspaces[0].name(), "devbox");
    assert_eq!(workspaces[0].cwd(), root.path());
    assert_eq!(remote_of(&app, workspaces[0].id()), Some("me@devbox"));
    assert_eq!(workspaces[1].name(), "buildbox");
    assert_eq!(remote_of(&app, workspaces[1].id()), Some("buildbox"));
    assert!(app.initial_remote.is_none() && app.deferred_actions.is_empty());
}

#[test]
fn a_startup_command_is_never_typed_into_an_ssh_connection() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    app.startup = None;
    add_workspace(&mut app, root.path(), Some("me@devbox"));
    let pane = app.controller.model().active_pane().unwrap();
    app.command_target = Some((pane, 1));
    app.send_startup_command();
    assert!(app.command.is_none());
    assert!(
        app.ui
            .error
            .as_deref()
            .unwrap()
            .contains("connected over SSH")
    );
}

#[test]
fn remote_workspaces_are_presented_by_host_instead_of_a_local_folder() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    app.startup = None;
    add_workspace(&mut app, root.path(), None);
    add_workspace(&mut app, root.path(), Some("me@devbox"));
    let pane = app.controller.model().active_pane().unwrap();
    let views = app.views();
    assert_eq!(views[0].remote, None);
    assert_eq!(views[1].remote.as_deref(), Some("me@devbox"));
    let presentation = &app.presentations()[&pane];
    assert_eq!(presentation.location(), "me@devbox");
    assert_eq!(
        presentation.metadata.shell,
        "/nonexistent/pace-startup-test-ssh"
    );
    assert!(presentation.starting);
}

#[cfg(unix)]
#[test]
fn a_remote_split_inherits_the_reported_host_directory_and_keeps_its_local_directory() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let cwd = root.path().join("local");
    std::fs::create_dir(&cwd).unwrap();
    // Stands in for the SSH client: records how it was started, then reports
    // a directory on the "host" the way a remote shell does.
    let client = root.path().join("fake-ssh");
    std::fs::write(
        &client,
        "#!/bin/sh\nprintf '%s %s %s\\n' \"$1\" \"$2\" \"$3\" >> \"$0.args\"\n\
         printf '%s\\0' \"$4\" >> \"$0.commands\"\npwd >> \"$0.cwd\"\n\
         printf '\\033]7;file://devbox/srv/on-the-host\\007'\nexec sleep 30\n",
    )
    .unwrap();
    std::fs::set_permissions(&client, std::fs::Permissions::from_mode(0o755)).unwrap();
    let lines = |suffix: &str| {
        std::fs::read_to_string(format!("{}.{suffix}", client.display())).unwrap_or_default()
    };

    let (mut app, _sender) = fixture(root.path());
    let ctx = egui::Context::default();
    app.command = None;
    app.ssh_client = client.to_string_lossy().into_owned();
    app.initial_cwd = Some(cwd.clone());
    app.initial_remote = Some("me@devbox".into());
    app.complete_startup(&ctx, loaded(app.config.clone(), Model::default()));
    let pane = app.controller.model().active_pane().unwrap();
    let generation = app.controller.generation();

    let deadline = Instant::now() + Duration::from_secs(10);
    let reported = std::path::Path::new("/srv/on-the-host");
    let mut seen = false;
    // The host's directory reaches the session; the model must not adopt it.
    while !seen {
        app.poll(&ctx);
        seen = app
            .sessions
            .get(pane)
            .is_some_and(|session| session.metadata().cwd == reported);
        assert!(
            Instant::now() < deadline,
            "the client never reported its directory: {:?}",
            app.ui.error
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    app.poll(&ctx);
    assert_eq!(lines("args"), "-t -- me@devbox\n");
    assert_eq!(
        std::path::Path::new(lines("cwd").trim_end())
            .canonicalize()
            .unwrap(),
        cwd.canonicalize().unwrap()
    );
    assert_eq!(app.controller.model().pane(pane).unwrap().cwd(), cwd);
    assert_eq!(
        app.controller.generation(),
        generation,
        "a remote directory was saved as a local one"
    );

    // The SSH client's local process polling must not become the remote path.
    std::thread::sleep(Duration::from_millis(1200));
    app.poll(&ctx);
    // A split opens another connection from the same local directory while
    // passing the host's reported path in the remote bootstrap.
    app.action(&ctx, Action::Split(pane, pace_model::Axis::Vertical));
    let second = app.controller.model().active_pane().unwrap();
    assert_ne!(second, pane);
    assert_eq!(app.controller.model().pane(second).unwrap().cwd(), cwd);
    while app.sessions.usage().running != 2 || lines("args").lines().count() != 2 {
        app.poll(&ctx);
        assert!(
            Instant::now() < deadline,
            "the split did not connect: {:?}",
            app.ui.error
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        lines("commands").contains("/srv/on-the-host"),
        "the split starts in the host's home directory instead of /srv/on-the-host: {:?}",
        lines("commands")
    );
}

#[cfg(unix)]
#[test]
fn remote_splits_follow_their_source_pane_after_focus_and_directory_changes() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let local = root.path().join("local");
    let home = root.path().join("fake-ssh.home");
    let first = home.join("first project");
    let next = home.join("next ' % λ project");
    for directory in [&local, &first, &next] {
        std::fs::create_dir_all(directory).unwrap();
    }
    std::fs::write(home.join(".zshrc"), "PROMPT='PACE> '\n").unwrap();
    let client = root.path().join("fake-ssh");
    std::fs::write(&client, "#!/bin/sh\nexport HOME=\"$0.home\" SHELL=zsh ZDOTDIR=\"$0.home\" TMPDIR=\"$0.home\"\ncd \"$HOME\"\nexec /bin/sh -c \"$4\"\n").unwrap();
    std::fs::set_permissions(&client, std::fs::Permissions::from_mode(0o755)).unwrap();
    let (mut app, _sender) = fixture(root.path());
    let ctx = egui::Context::default();
    app.command = None;
    app.ssh_client = client.to_str().unwrap().into();
    app.initial_cwd = Some(local.clone());
    app.initial_remote = Some("devbox".into());
    app.complete_startup(&ctx, loaded(app.config.clone(), Model::default()));
    let source = app.controller.model().active_pane().unwrap();
    let wait_for_directory = |app: &mut App, pane, directory: &std::path::Path| {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            app.poll(&ctx);
            if app
                .sessions
                .get(pane)
                .is_some_and(|s| s.metadata().reported_cwd.as_deref() == Some(directory))
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "pane did not enter its remote directory: {:?}",
                app.ui.error
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    wait_for_directory(&mut app, source, &home);
    app.sessions
        .get(source)
        .unwrap()
        .write(b"cd 'first project'\r")
        .unwrap();
    wait_for_directory(&mut app, source, &first);
    app.action(&ctx, Action::Split(source, pace_model::Axis::Vertical));
    let right = app.controller.model().active_pane().unwrap();
    wait_for_directory(&mut app, right, &first);
    // The source is now unfocused, but its parser and directory hooks stay live.
    app.sessions
        .get(source)
        .unwrap()
        .write(b"cd ../next*\r")
        .unwrap();
    wait_for_directory(&mut app, source, &next);
    app.action(&ctx, Action::Split(source, pace_model::Axis::Horizontal));
    let below = app.controller.model().active_pane().unwrap();
    wait_for_directory(&mut app, below, &next);
    assert_eq!(
        app.sessions
            .get(right)
            .unwrap()
            .metadata()
            .reported_cwd
            .as_deref(),
        Some(first.as_path())
    );
    for pane in [source, right, below] {
        assert_eq!(app.controller.model().pane(pane).unwrap().cwd(), local);
    }
}

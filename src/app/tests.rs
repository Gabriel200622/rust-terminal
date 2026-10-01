use super::*;

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
        writer: None,
        state_writable: false,
        startup: Some(receiver),
        deferred_actions: Vec::new(),
        initial_cwd: Some(root.into()),
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
fn startup_actions_wait_then_replay_on_restored_state_without_retargeting_the_cli_command() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _sender) = fixture(root.path());
    let ctx = egui::Context::default();
    let mut restored = Controller::new(Model::default());
    restored
        .dispatch(Command::AddWorkspace {
            cwd: root.path().into(),
            name: "Restored".into(),
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

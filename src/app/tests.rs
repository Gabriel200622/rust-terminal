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
    app.action(
        &ctx,
        Action::Create(root.path().into(), Some("Queued".into())),
    );
    app.action(&ctx, Action::ToggleSidebar);
    assert!(app.controller.model().workspaces().is_empty());
    assert_eq!(app.sessions.usage().starting, 0);
    app.complete_startup(&ctx, loaded(app.config.clone(), restored.model().clone()));
    assert_eq!(app.controller.model().workspaces().len(), 2);
    assert_eq!(app.controller.model().workspaces()[0].name(), "Restored");
    assert_eq!(app.controller.model().workspaces()[1].name(), "Queued");
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
        app.action(
            &ctx,
            Action::Create(root.path().into(), Some("Queued".into())),
        );
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
    app.command = Some("printf x >> launch-marker".into());
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
    while !first.join("launch-marker").exists() {
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
fn dialogs_request_field_focus_and_close_without_touching_workspaces() {
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

    app.action(&ctx, Action::New);
    assert_eq!(app.ui.overlay, OverlayState::NewWorkspace);
    assert!(app.ui.overlay_focus);
    assert_eq!(app.ui.new_cwd, root.path().display().to_string());
    app.action(&ctx, Action::CloseOverlay);
    assert_eq!(app.ui.overlay, OverlayState::None);

    app.ui.overlay_focus = false;
    app.action(&ctx, Action::Rename(workspace));
    assert_eq!(app.ui.overlay, OverlayState::Rename(workspace));
    assert!(app.ui.overlay_focus);
    assert_eq!(app.ui.rename_name, "Only");
    app.action(&ctx, Action::CloseOverlay);

    app.ui.error = Some("Could not save".into());
    app.action(&ctx, Action::DismissError);
    assert!(app.ui.error.is_none());
    assert_eq!(app.controller.generation(), generation);
    assert_eq!(app.controller.model().workspaces().len(), 1);
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

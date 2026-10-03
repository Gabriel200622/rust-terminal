//! Workspace group directory drafts and one bounded, off-frame filesystem check.
use super::*;
use neptune_model::WorkspaceGroupId;

pub(super) struct PendingDirectory {
    group: WorkspaceGroupId,
    draft: String,
    receiver: mpsc::Receiver<Result<Option<PathBuf>, String>>,
    browsing: bool,
    cancelled: bool,
}

fn directory_path(draft: &str) -> Result<PathBuf, String> {
    let directory =
        if draft == "~" || draft.starts_with("~/") || cfg!(windows) && draft.starts_with("~\\") {
            let home =
                directories::BaseDirs::new().ok_or("Could not determine your home directory")?;
            home.home_dir().join(draft.get(2..).unwrap_or_default())
        } else {
            PathBuf::from(draft)
        };
    if !directory.is_absolute() || directory.as_os_str().as_encoded_bytes().contains(&0) {
        return Err("Enter an absolute directory path or ~/folder".into());
    }
    Ok(directory)
}

fn local_directory(draft: &str) -> Result<PathBuf, String> {
    validate_directory(directory_path(draft)?)
}

fn validate_directory(directory: PathBuf) -> Result<PathBuf, String> {
    match std::fs::metadata(&directory) {
        Ok(metadata) if metadata.is_dir() => Ok(directory),
        Ok(_) => Err("This path is a file. Choose a directory".into()),
        Err(error) => Err(format!("Cannot open this directory: {error}")),
    }
}

impl App {
    pub(super) fn cancel_directory_check(&mut self) {
        if let Some(check) = &mut self.directory_check {
            check.cancelled = true;
        }
    }

    pub(super) fn edit_directory(&mut self, group: WorkspaceGroupId) {
        self.cancel_directory_check();
        let Some(target) = self.controller.model().group(group) else {
            return;
        };
        self.ui.directory_path = target
            .default_directory()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.ui.directory_selected = target.default_directory().map(std::path::Path::to_path_buf);
        self.ui.directory_group = target.name().into();
        self.ui.directory_error = None;
        self.ui.overlay = OverlayState::GroupDefaultDirectory(group);
        self.ui.overlay_focus = true;
    }

    pub(super) fn browse_directory(&mut self, ctx: &egui::Context, group: WorkspaceGroupId) {
        if self.ui.overlay != OverlayState::GroupDefaultDirectory(group)
            || self.directory_check.is_some()
            || self.controller.model().group(group).is_none()
        {
            return;
        }
        let directory = self
            .ui
            .directory_selected
            .clone()
            .or_else(|| directory_path(&self.ui.directory_path).ok())
            .or_else(|| directories::BaseDirs::new().map(|dirs| dirs.home_dir().into()));
        match self.folder_picker.open(directory, ctx.clone()) {
            Ok(receiver) => {
                self.directory_check = Some(PendingDirectory {
                    group,
                    draft: self.ui.directory_path.clone(),
                    receiver,
                    browsing: true,
                    cancelled: false,
                });
                self.ui.directory_browsing = true;
                self.ui.directory_error = None;
            }
            Err(error) => self.ui.directory_error = Some(error),
        }
    }

    pub(super) fn set_directory(
        &mut self,
        ctx: &egui::Context,
        group: WorkspaceGroupId,
        draft: Option<String>,
    ) {
        if self.ui.overlay != OverlayState::GroupDefaultDirectory(group)
            || self.directory_check.is_some()
        {
            return;
        }
        if self.controller.model().group(group).is_none() {
            self.ui.directory_error = Some("This workspace group was removed".into());
            return;
        }
        let Some(draft) = draft else {
            self.commit_directory(ctx, group, None);
            return;
        };
        let (sender, receiver) = mpsc::sync_channel(1);
        let input = draft.clone();
        let selected = self
            .ui
            .directory_selected
            .clone()
            .filter(|path| path.to_string_lossy() == draft);
        let wake = ctx.clone();
        match std::thread::Builder::new()
            .name("neptune-directory".into())
            .spawn(move || {
                let result = match selected {
                    Some(directory) => validate_directory(directory),
                    None => local_directory(&input),
                };
                let _ = sender.send(result.map(Some));
                wake.request_repaint();
            }) {
            Ok(_) => {
                self.directory_check = Some(PendingDirectory {
                    group,
                    draft,
                    receiver,
                    browsing: false,
                    cancelled: false,
                });
                self.ui.directory_pending = true;
                self.ui.directory_error = None;
            }
            Err(error) => {
                self.ui.directory_error = Some(format!("Cannot check this directory: {error}"))
            }
        }
    }

    fn commit_directory(
        &mut self,
        ctx: &egui::Context,
        group: WorkspaceGroupId,
        directory: Option<PathBuf>,
    ) {
        match self
            .controller
            .dispatch(Command::SetWorkspaceGroupDefaultDirectory { group, directory })
        {
            Ok(effects) => {
                self.execute(ctx, effects);
                self.ui.overlay = OverlayState::None;
            }
            Err(error) => self.ui.directory_error = Some(error.to_string()),
        }
    }

    pub(super) fn poll_directory(&mut self, ctx: &egui::Context) {
        let Some(check) = &self.directory_check else {
            return;
        };
        let result = match check.receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("Directory check stopped; try again".into())
            }
        };
        let Some(check) = self.directory_check.take() else {
            return;
        };
        self.ui.directory_pending = false;
        self.ui.directory_browsing = false;
        if check.cancelled
            || self.ui.overlay != OverlayState::GroupDefaultDirectory(check.group)
            || self.ui.directory_path != check.draft
        {
            return;
        }
        if self.controller.model().group(check.group).is_none() {
            self.ui.directory_error = Some("This workspace group was removed".into());
            return;
        }
        match result {
            Ok(Some(directory)) if check.browsing => {
                self.ui.directory_path = directory.to_string_lossy().into_owned();
                self.ui.directory_selected = Some(directory);
                self.ui.directory_error = None;
                self.ui.overlay_focus = true;
            }
            Ok(Some(directory)) => self.commit_directory(ctx, check.group, Some(directory)),
            Ok(None) => self.ui.overlay_focus = true,
            Err(error) => self.ui.directory_error = Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(root: &std::path::Path) -> (App, WorkspaceGroupId) {
        let (mut app, _) = super::super::tests::fixture(root);
        app.startup = None;
        app.controller
            .dispatch(Command::AddWorkspaceGroup {
                name: "Projects".into(),
            })
            .unwrap();
        let group = app.controller.model().groups()[0].id();
        (app, group)
    }

    fn wait(app: &mut App, ctx: &egui::Context) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.directory_check.is_some() {
            assert!(Instant::now() < deadline, "directory check did not finish");
            app.poll_directory(ctx);
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn picker_request(
        app: &mut App,
        group: WorkspaceGroupId,
    ) -> mpsc::SyncSender<Result<Option<PathBuf>, String>> {
        let (sender, receiver) = mpsc::sync_channel(1);
        app.directory_check = Some(PendingDirectory {
            group,
            draft: app.ui.directory_path.clone(),
            receiver,
            browsing: true,
            cancelled: false,
        });
        app.ui.directory_browsing = true;
        sender
    }

    #[test]
    fn choosing_a_folder_only_edits_the_draft_until_saved_and_allows_manual_edits() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("Project");
        std::fs::create_dir(&project).unwrap();
        let (mut app, group) = fixture(root.path());
        let ctx = egui::Context::default();
        app.edit_directory(group);
        let generation = app.controller.generation();
        picker_request(&mut app, group)
            .send(Ok(Some(project.clone())))
            .unwrap();
        app.poll_directory(&ctx);
        assert_eq!(app.controller.generation(), generation);
        assert_eq!(app.ui.overlay, OverlayState::GroupDefaultDirectory(group));
        assert_eq!(app.ui.directory_path, project.to_string_lossy());
        assert_eq!(app.ui.directory_selected.as_ref(), Some(&project));
        assert!(!app.ui.directory_browsing);
        app.set_directory(&ctx, group, Some(app.ui.directory_path.clone()));
        wait(&mut app, &ctx);
        assert_eq!(
            app.controller
                .model()
                .group(group)
                .unwrap()
                .default_directory(),
            Some(project.as_path())
        );
        app.edit_directory(group);
        app.ui.directory_path = root.path().to_string_lossy().into_owned();
        app.set_directory(&ctx, group, Some(app.ui.directory_path.clone()));
        wait(&mut app, &ctx);
        assert_eq!(
            app.controller
                .model()
                .group(group)
                .unwrap()
                .default_directory(),
            Some(root.path())
        );
    }

    #[test]
    fn cancelling_the_native_picker_keeps_the_path_and_saved_default() {
        let root = tempfile::tempdir().unwrap();
        let (mut app, group) = fixture(root.path());
        let ctx = egui::Context::default();
        app.controller
            .dispatch(Command::SetWorkspaceGroupDefaultDirectory {
                group,
                directory: Some(root.path().into()),
            })
            .unwrap();
        app.edit_directory(group);
        let generation = app.controller.generation();
        let path = app.ui.directory_path.clone();
        picker_request(&mut app, group).send(Ok(None)).unwrap();
        app.poll_directory(&ctx);
        assert_eq!(app.ui.directory_path, path);
        assert_eq!(app.ui.directory_selected.as_deref(), Some(root.path()));
        assert_eq!(app.controller.generation(), generation);
        assert!(app.ui.directory_error.is_none());
        assert!(!app.ui.directory_browsing);
    }

    #[test]
    fn picker_results_cannot_change_cancelled_removed_reopened_or_edited_drafts() {
        let root = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        for scenario in ["cancel", "remove", "reopen", "edit"] {
            let (mut app, group) = fixture(root.path());
            app.edit_directory(group);
            let sender = picker_request(&mut app, group);
            // Save and repeated Browse cannot start a second operation.
            app.set_directory(
                &ctx,
                group,
                Some(root.path().to_string_lossy().into_owned()),
            );
            app.browse_directory(&ctx, group);
            assert!(app.directory_check.as_ref().unwrap().browsing);
            match scenario {
                "cancel" => app.action(&ctx, Action::CloseOverlay),
                "remove" => {
                    app.controller
                        .dispatch(Command::RemoveWorkspaceGroup(group))
                        .unwrap();
                }
                "reopen" => app.edit_directory(group),
                "edit" => app.ui.directory_path = "edited".into(),
                _ => unreachable!(),
            }
            let draft = app.ui.directory_path.clone();
            let generation = app.controller.generation();
            sender.send(Ok(Some(root.path().into()))).unwrap();
            app.poll_directory(&ctx);
            assert_eq!(app.ui.directory_path, draft, "{scenario}");
            assert_eq!(app.controller.generation(), generation, "{scenario}");
            assert!(!app.ui.directory_browsing);
        }
    }

    #[test]
    fn local_directory_validates_paths_and_expands_home_without_shell_evaluation() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("project space ' λ $(touch injected)");
        std::fs::create_dir(&path).unwrap();
        assert_eq!(local_directory(path.to_str().unwrap()), Ok(path));
        #[cfg(unix)]
        {
            let trailing_space = root.path().join("folder with trailing space ");
            std::fs::create_dir(&trailing_space).unwrap();
            assert_eq!(
                local_directory(trailing_space.to_str().unwrap()),
                Ok(trailing_space)
            );
        }
        assert_eq!(
            local_directory("~").unwrap(),
            directories::BaseDirs::new().unwrap().home_dir()
        );
        let file = root.path().join("file");
        std::fs::write(&file, "fixture").unwrap();
        for invalid in [
            "",
            "relative",
            "/bad\0path",
            file.to_str().unwrap(),
            root.path().join("missing").to_str().unwrap(),
        ] {
            assert!(local_directory(invalid).is_err(), "{invalid:?}");
        }
        assert!(!root.path().join("injected").exists());
    }

    #[test]
    fn directory_save_targets_the_group_even_after_selection_and_name_changes() {
        let root = tempfile::tempdir().unwrap();
        let (mut app, group) = fixture(root.path());
        let ctx = egui::Context::default();
        app.edit_directory(group);
        app.ui.directory_path = root.path().to_str().unwrap().into();
        app.set_directory(&ctx, group, Some(app.ui.directory_path.clone()));
        app.controller
            .dispatch(Command::AddWorkspaceGroup {
                name: "Other".into(),
            })
            .unwrap();
        let other = app.controller.model().groups()[1].id();
        app.controller
            .dispatch(Command::AddWorkspace {
                cwd: root.path().into(),
                name: "other".into(),
                group: Some(other),
                remote: None,
            })
            .unwrap();
        app.controller
            .dispatch(Command::RenameWorkspaceGroup {
                group,
                name: "Renamed".into(),
            })
            .unwrap();
        let workspace = app.controller.model().active_workspace();
        wait(&mut app, &ctx);
        assert_eq!(
            app.controller
                .model()
                .group(group)
                .unwrap()
                .default_directory(),
            Some(root.path())
        );
        assert!(
            app.controller
                .model()
                .group(other)
                .unwrap()
                .default_directory()
                .is_none()
        );
        assert_eq!(app.controller.model().active_workspace(), workspace);
        assert_eq!(app.ui.overlay, OverlayState::None);
        app.edit_directory(group);
        app.set_directory(&ctx, group, None);
        assert!(
            app.controller
                .model()
                .group(group)
                .unwrap()
                .default_directory()
                .is_none()
        );
    }

    #[test]
    fn directory_checks_are_bounded_and_cancelled_or_stale_results_cannot_save() {
        let root = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        for scenario in ["cancel", "remove", "reopen", "edit"] {
            let (mut app, group) = fixture(root.path());
            app.edit_directory(group);
            app.ui.directory_path = root.path().to_str().unwrap().into();
            app.set_directory(&ctx, group, Some(app.ui.directory_path.clone()));
            app.set_directory(&ctx, group, Some("another".into()));
            assert_eq!(
                app.directory_check.as_ref().unwrap().draft,
                app.ui.directory_path
            );
            match scenario {
                "cancel" => app.action(&ctx, Action::CloseOverlay),
                "remove" => {
                    app.controller
                        .dispatch(Command::RemoveWorkspaceGroup(group))
                        .unwrap();
                }
                "reopen" => app.edit_directory(group),
                "edit" => app.ui.directory_path = "changed".into(),
                _ => unreachable!(),
            }
            let generation = app.controller.generation();
            wait(&mut app, &ctx);
            assert_eq!(app.controller.generation(), generation, "{scenario}");
            assert!(!app.ui.directory_pending);
            assert!(
                app.controller
                    .model()
                    .group(group)
                    .is_none_or(|target| target.default_directory().is_none())
            );
        }
    }

    #[test]
    fn invalid_directory_preserves_the_saved_default_and_keeps_the_draft_open() {
        let root = tempfile::tempdir().unwrap();
        let (mut app, group) = fixture(root.path());
        let ctx = egui::Context::default();
        app.controller
            .dispatch(Command::SetWorkspaceGroupDefaultDirectory {
                group,
                directory: Some(root.path().into()),
            })
            .unwrap();
        app.edit_directory(group);
        app.ui.directory_path = root.path().join("missing").to_str().unwrap().into();
        app.set_directory(&ctx, group, Some(app.ui.directory_path.clone()));
        wait(&mut app, &ctx);
        assert_eq!(app.ui.overlay, OverlayState::GroupDefaultDirectory(group));
        assert!(app.ui.directory_error.is_some());
        assert_eq!(
            app.controller
                .model()
                .group(group)
                .unwrap()
                .default_directory(),
            Some(root.path())
        );
    }

    #[test]
    fn new_workspace_is_named_after_the_group_directory() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("Project");
        std::fs::create_dir(&project).unwrap();
        let (mut app, group) = fixture(root.path());
        let ctx = egui::Context::default();
        app.controller
            .dispatch(Command::SetWorkspaceGroupDefaultDirectory {
                group,
                directory: Some(project.clone()),
            })
            .unwrap();
        app.action(&ctx, Action::NewInGroup(group));
        let workspace = app.controller.model().workspaces().last().unwrap();
        assert_eq!(workspace.name(), "Project");
        assert_eq!(workspace.cwd(), project);
    }
}

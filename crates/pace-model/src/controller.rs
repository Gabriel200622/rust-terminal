use crate::{Axis, Error, Layout, Lifecycle, Model, Pane, PaneId, SplitId, Workspace, WorkspaceId};
use std::path::PathBuf;

/// All durable UI mutations use this path. Targets are captured when commands
/// are created rather than resolved against whichever pane is active later.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    AddWorkspace {
        cwd: PathBuf,
        name: String,
    },
    SelectWorkspace(WorkspaceId),
    SplitPane {
        workspace: WorkspaceId,
        pane: PaneId,
        axis: Axis,
        cwd: PathBuf,
    },
    FocusPane {
        workspace: WorkspaceId,
        pane: PaneId,
    },
    ClosePane(PaneId),
    CloseWorkspace(WorkspaceId),
    RenameWorkspace {
        workspace: WorkspaceId,
        name: String,
    },
    SetSplitRatio {
        split: SplitId,
        ratio: f32,
    },
    RestartPane(PaneId),
    SetSidebar(bool),
    /// The composition layer owns the typed configuration and supplies its
    /// immutable replacement to the preference writer when this effect runs.
    UpdatePreferences,
    AcknowledgeSave {
        generation: u64,
    },
    SessionStarted {
        pane: PaneId,
        generation: u64,
    },
    SessionFailed {
        pane: PaneId,
        generation: u64,
        error: String,
    },
    SessionExited {
        pane: PaneId,
        generation: u64,
    },
    PaneCwdChanged {
        pane: PaneId,
        generation: u64,
        cwd: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    StartSession {
        pane: PaneId,
        generation: u64,
        cwd: PathBuf,
        replacement: bool,
    },
    StopSession {
        pane: PaneId,
        generation: u64,
    },
    Focus {
        old: Option<PaneId>,
        new: Option<PaneId>,
    },
    ResetSearch,
    Persist {
        generation: u64,
    },
    SavePreferences,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Completion {
    Started {
        pane: PaneId,
        generation: u64,
    },
    Failed {
        pane: PaneId,
        generation: u64,
        error: String,
    },
    Exited {
        pane: PaneId,
        generation: u64,
    },
    Saved {
        generation: u64,
    },
}

#[derive(Debug, Clone)]
pub struct Controller {
    model: Model,
    generation: u64,
    saved_generation: u64,
}

impl Controller {
    pub fn new(model: Model) -> Self {
        Self {
            model,
            generation: 0,
            saved_generation: 0,
        }
    }
    pub fn model(&self) -> &Model {
        &self.model
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn saved_generation(&self) -> u64 {
        self.saved_generation
    }
    pub fn is_dirty(&self) -> bool {
        self.saved_generation < self.generation
    }

    /// Restoration can populate panes immediately before bounded workers start
    /// their shells. It does not serialize or touch a filesystem.
    pub fn start_effects(&self) -> Vec<Effect> {
        self.model
            .workspaces
            .iter()
            .flat_map(|workspace| workspace.panes.iter())
            .map(|pane| Effect::StartSession {
                pane: pane.id,
                generation: pane.generation,
                cwd: pane.cwd.clone(),
                replacement: false,
            })
            .collect()
    }

    pub fn complete(&mut self, completion: Completion) -> Result<Vec<Effect>, Error> {
        self.dispatch(match completion {
            Completion::Started { pane, generation } => {
                Command::SessionStarted { pane, generation }
            }
            Completion::Failed {
                pane,
                generation,
                error,
            } => Command::SessionFailed {
                pane,
                generation,
                error,
            },
            Completion::Exited { pane, generation } => Command::SessionExited { pane, generation },
            Completion::Saved { generation } => Command::AcknowledgeSave { generation },
        })
    }

    pub fn dispatch(&mut self, command: Command) -> Result<Vec<Effect>, Error> {
        let old_focus = self.model.active_pane();
        let mut effects = Vec::new();
        let mut dirty = false;
        match command {
            Command::AddWorkspace { cwd, name } => {
                if name.trim().is_empty() {
                    return Err(Error::InvalidName);
                }
                if self.model.workspaces.len() >= self.model.limits.workspaces {
                    return Err(Error::WorkspaceLimit);
                }
                self.check_pane_capacity(0)?;
                let id = WorkspaceId::new(self.model.next_workspace);
                let pane_id = PaneId::new(self.model.next_pane);
                let next_workspace = self
                    .model
                    .next_workspace
                    .checked_add(1)
                    .ok_or(Error::IdentityExhausted)?;
                let next_pane = self
                    .model
                    .next_pane
                    .checked_add(1)
                    .ok_or(Error::IdentityExhausted)?;
                self.model.next_workspace = next_workspace;
                self.model.next_pane = next_pane;
                self.model.workspaces.push(Workspace {
                    id,
                    name,
                    cwd: cwd.clone(),
                    panes: vec![Pane {
                        id: pane_id,
                        cwd: cwd.clone(),
                        generation: 1,
                        lifecycle: Lifecycle::Starting,
                    }],
                    layout: Layout::Leaf(pane_id),
                    active: pane_id,
                });
                self.model.active = Some(id);
                effects.push(Effect::StartSession {
                    pane: pane_id,
                    generation: 1,
                    cwd,
                    replacement: false,
                });
                dirty = true;
            }
            Command::SelectWorkspace(id) => {
                if self.model.workspace(id).is_none() {
                    return Err(Error::UnknownWorkspace(id));
                }
                if self.model.active != Some(id) {
                    self.model.active = Some(id);
                    dirty = true;
                }
            }
            Command::SplitPane {
                workspace,
                pane,
                axis,
                cwd,
            } => {
                let ws = self
                    .model
                    .workspace(workspace)
                    .ok_or(Error::UnknownWorkspace(workspace))?;
                if ws.pane(pane).is_none() {
                    return Err(Error::UnknownPane(pane));
                }
                self.check_pane_capacity(ws.panes.len())?;
                let id = PaneId::new(self.model.next_pane);
                let split = SplitId::new(self.model.next_split);
                let next_pane = self
                    .model
                    .next_pane
                    .checked_add(1)
                    .ok_or(Error::IdentityExhausted)?;
                let next_split = self
                    .model
                    .next_split
                    .checked_add(1)
                    .ok_or(Error::IdentityExhausted)?;
                let ws = self.model.workspace_mut(workspace)?;
                if !ws.layout.split(pane, id, split, axis) {
                    return Err(Error::UnknownPane(pane));
                }
                ws.panes.push(Pane {
                    id,
                    cwd: cwd.clone(),
                    generation: 1,
                    lifecycle: Lifecycle::Starting,
                });
                ws.active = id;
                self.model.active = Some(workspace);
                self.model.next_pane = next_pane;
                self.model.next_split = next_split;
                effects.push(Effect::StartSession {
                    pane: id,
                    generation: 1,
                    cwd,
                    replacement: false,
                });
                dirty = true;
            }
            Command::FocusPane { workspace, pane } => {
                let was_active = self.model.active == Some(workspace);
                let ws = self.model.workspace_mut(workspace)?;
                if ws.pane(pane).is_none() {
                    return Err(Error::UnknownPane(pane));
                }
                dirty = ws.active != pane || !was_active;
                ws.active = pane;
                self.model.active = Some(workspace);
            }
            Command::ClosePane(pane) => {
                let workspace = self
                    .model
                    .workspace_for_pane(pane)
                    .ok_or(Error::UnknownPane(pane))?;
                let ws = self.model.workspace_mut(workspace)?;
                if ws.panes.len() == 1 {
                    self.close_workspace(workspace, &mut effects)?;
                } else {
                    let position = ws
                        .panes
                        .iter()
                        .position(|item| item.id == pane)
                        .ok_or(Error::UnknownPane(pane))?;
                    let removed = ws.panes.remove(position);
                    let layout = ws
                        .layout
                        .clone()
                        .remove(pane)
                        .ok_or(Error::InvalidLayout("close removed every leaf"))?;
                    ws.layout = layout;
                    if ws.active == pane {
                        ws.active = ws.panes[position.min(ws.panes.len() - 1)].id;
                    }
                    effects.push(Effect::StopSession {
                        pane,
                        generation: removed.generation,
                    });
                }
                dirty = true;
            }
            Command::CloseWorkspace(workspace) => {
                self.close_workspace(workspace, &mut effects)?;
                dirty = true;
            }
            Command::RenameWorkspace { workspace, name } => {
                if name.trim().is_empty() {
                    return Err(Error::InvalidName);
                }
                let ws = self.model.workspace_mut(workspace)?;
                if ws.name != name {
                    ws.name = name;
                    dirty = true;
                }
            }
            Command::SetSplitRatio { split, ratio } => {
                if !ratio.is_finite() || !(0.1..=0.9).contains(&ratio) {
                    return Err(Error::InvalidRatio);
                }
                dirty = self
                    .model
                    .workspaces
                    .iter_mut()
                    .find_map(|workspace| workspace.layout.set_ratio(split, ratio))
                    .ok_or(Error::UnknownSplit(split))?;
            }
            Command::RestartPane(pane) => {
                let item = self.model.pane_mut(pane)?;
                let previous = item.generation;
                item.generation = item
                    .generation
                    .checked_add(1)
                    .ok_or(Error::IdentityExhausted)?;
                item.lifecycle = Lifecycle::Starting;
                effects.push(Effect::StopSession {
                    pane,
                    generation: previous,
                });
                effects.push(Effect::StartSession {
                    pane,
                    generation: item.generation,
                    cwd: item.cwd.clone(),
                    replacement: true,
                });
                if old_focus == Some(pane) {
                    effects.push(Effect::ResetSearch);
                }
            }
            Command::SetSidebar(sidebar) => {
                if self.model.sidebar != sidebar {
                    self.model.sidebar = sidebar;
                    dirty = true;
                }
            }
            Command::UpdatePreferences => effects.push(Effect::SavePreferences),
            Command::AcknowledgeSave { generation } => {
                if generation <= self.generation {
                    self.saved_generation = self.saved_generation.max(generation);
                }
            }
            Command::SessionStarted { pane, generation } => {
                self.lifecycle(pane, generation, Lifecycle::Running)
            }
            Command::SessionFailed {
                pane,
                generation,
                error,
            } => self.lifecycle(pane, generation, Lifecycle::Failed(error)),
            Command::SessionExited { pane, generation } => {
                self.lifecycle(pane, generation, Lifecycle::Exited)
            }
            Command::PaneCwdChanged {
                pane,
                generation,
                cwd,
            } => {
                if let Ok(item) = self.model.pane_mut(pane)
                    && item.generation == generation
                    && item.cwd != cwd
                {
                    item.cwd = cwd;
                    dirty = true;
                }
            }
        }
        let new_focus = self.model.active_pane();
        if old_focus != new_focus {
            effects.push(Effect::Focus {
                old: old_focus,
                new: new_focus,
            });
            effects.push(Effect::ResetSearch);
        }
        if dirty {
            self.generation = self.generation.saturating_add(1);
            effects.push(Effect::Persist {
                generation: self.generation,
            });
        }
        Ok(effects)
    }

    fn check_pane_capacity(&self, count: usize) -> Result<(), Error> {
        if count >= self.model.limits.panes_per_workspace {
            return Err(Error::PaneLimit);
        }
        if self.model.pane_count() >= self.model.limits.total_panes {
            return Err(Error::TotalPaneLimit);
        }
        Ok(())
    }

    fn close_workspace(
        &mut self,
        workspace: WorkspaceId,
        effects: &mut Vec<Effect>,
    ) -> Result<(), Error> {
        let position = self
            .model
            .workspaces
            .iter()
            .position(|item| item.id == workspace)
            .ok_or(Error::UnknownWorkspace(workspace))?;
        let removed = self.model.workspaces.remove(position);
        for pane in removed.panes {
            effects.push(Effect::StopSession {
                pane: pane.id,
                generation: pane.generation,
            });
        }
        if self.model.active == Some(workspace) {
            self.model.active = self
                .model
                .workspaces
                .get(position.min(self.model.workspaces.len().saturating_sub(1)))
                .map(Workspace::id);
        }
        Ok(())
    }

    fn lifecycle(&mut self, pane: PaneId, generation: u64, lifecycle: Lifecycle) {
        // A closed pane or a previous replacement cannot publish into a new one.
        if let Ok(item) = self.model.pane_mut(pane)
            && item.generation == generation
            && matches!(
                (&item.lifecycle, &lifecycle),
                (
                    Lifecycle::Starting,
                    Lifecycle::Running | Lifecycle::Failed(_)
                ) | (Lifecycle::Running, Lifecycle::Exited)
            )
        {
            item.lifecycle = lifecycle;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Limits, PaneSpec, WorkspaceSpec};

    #[derive(Default)]
    struct FakeRuntime {
        effects: Vec<Effect>,
    }
    impl FakeRuntime {
        fn run(&mut self, controller: &mut Controller, command: Command) -> Vec<Effect> {
            let effects = controller.dispatch(command).unwrap();
            self.effects.extend(effects.clone());
            for effect in &effects {
                if let Effect::StartSession {
                    pane, generation, ..
                } = effect
                {
                    controller
                        .complete(Completion::Started {
                            pane: *pane,
                            generation: *generation,
                        })
                        .unwrap();
                }
            }
            effects
        }
    }

    fn create(controller: &mut Controller, name: &str) -> WorkspaceId {
        controller
            .dispatch(Command::AddWorkspace {
                cwd: PathBuf::from("/fake"),
                name: name.into(),
            })
            .unwrap();
        controller.model().active_workspace().unwrap()
    }
    fn setup() -> (Controller, WorkspaceId, PaneId) {
        let mut controller = Controller::new(Model::default());
        let workspace = create(&mut controller, "main");
        let pane = controller.model().active_pane().unwrap();
        (controller, workspace, pane)
    }

    #[test]
    fn selection_and_focus_share_reports_search_reset_and_dirty_policy() {
        let (mut controller, first, first_pane) = setup();
        let second = create(&mut controller, "second");
        let second_pane = controller.model().active_pane().unwrap();
        let sidebar = controller
            .dispatch(Command::SelectWorkspace(first))
            .unwrap();
        assert!(sidebar.contains(&Effect::Focus {
            old: Some(second_pane),
            new: Some(first_pane)
        }));
        assert!(sidebar.contains(&Effect::ResetSearch));
        controller
            .dispatch(Command::SelectWorkspace(second))
            .unwrap();
        let direct = controller
            .dispatch(Command::FocusPane {
                workspace: first,
                pane: first_pane,
            })
            .unwrap();
        let without_save = |effects: Vec<Effect>| {
            effects
                .into_iter()
                .filter(|effect| !matches!(effect, Effect::Persist { .. }))
                .collect::<Vec<_>>()
        };
        assert_eq!(without_save(sidebar), without_save(direct));
    }

    #[test]
    fn queued_split_targets_original_workspace_after_selection_changes() {
        let (mut controller, workspace, pane) = setup();
        create(&mut controller, "another");
        controller
            .dispatch(Command::SplitPane {
                workspace,
                pane,
                axis: Axis::Horizontal,
                cwd: PathBuf::from("/fake"),
            })
            .unwrap();
        assert_eq!(
            controller
                .model()
                .workspace(workspace)
                .unwrap()
                .panes()
                .len(),
            2
        );
        assert_eq!(controller.model().workspaces()[1].panes().len(), 1);
    }

    #[test]
    fn closing_before_active_workspace_preserves_identity() {
        let (mut controller, first, _) = setup();
        let second = create(&mut controller, "second");
        create(&mut controller, "third");
        controller
            .dispatch(Command::SelectWorkspace(second))
            .unwrap();
        controller.dispatch(Command::CloseWorkspace(first)).unwrap();
        assert_eq!(controller.model().active_workspace(), Some(second));
    }

    #[test]
    fn closing_active_workspace_focuses_successor_then_predecessor() {
        let (mut controller, first, _) = setup();
        let second = create(&mut controller, "second");
        let third = create(&mut controller, "third");
        controller
            .dispatch(Command::SelectWorkspace(second))
            .unwrap();
        controller
            .dispatch(Command::CloseWorkspace(second))
            .unwrap();
        assert_eq!(controller.model().active_workspace(), Some(third));
        controller.dispatch(Command::CloseWorkspace(third)).unwrap();
        assert_eq!(controller.model().active_workspace(), Some(first));
    }

    #[test]
    fn closing_after_active_workspace_preserves_focus() {
        let (mut controller, first, pane) = setup();
        let second = create(&mut controller, "second");
        controller
            .dispatch(Command::SelectWorkspace(first))
            .unwrap();
        let effects = controller
            .dispatch(Command::CloseWorkspace(second))
            .unwrap();
        assert_eq!(controller.model().active_pane(), Some(pane));
        assert!(
            !effects
                .iter()
                .any(|effect| matches!(effect, Effect::Focus { .. }))
        );
    }

    #[test]
    fn close_pane_collapses_layout_and_keeps_unrelated_split_identity() {
        let (mut controller, workspace, first) = setup();
        controller
            .dispatch(Command::SplitPane {
                workspace,
                pane: first,
                axis: Axis::Vertical,
                cwd: PathBuf::from("/fake"),
            })
            .unwrap();
        let second = controller.model().active_pane().unwrap();
        let outer = match controller.model().workspace(workspace).unwrap().layout() {
            Layout::Split { id, .. } => *id,
            _ => panic!("expected split"),
        };
        controller
            .dispatch(Command::SplitPane {
                workspace,
                pane: second,
                axis: Axis::Horizontal,
                cwd: PathBuf::from("/fake"),
            })
            .unwrap();
        let third = controller.model().active_pane().unwrap();
        controller.dispatch(Command::ClosePane(third)).unwrap();
        assert_eq!(
            controller
                .model()
                .workspace(workspace)
                .unwrap()
                .layout()
                .leaves(),
            vec![first, second]
        );
        assert!(
            matches!(controller.model().workspace(workspace).unwrap().layout(), Layout::Split { id, .. } if *id == outer)
        );
    }

    #[test]
    fn invalid_focus_and_split_are_atomic() {
        let (mut controller, workspace, _) = setup();
        let before = controller.model().clone();
        assert!(
            controller
                .dispatch(Command::FocusPane {
                    workspace,
                    pane: PaneId::new(99)
                })
                .is_err()
        );
        assert!(
            controller
                .dispatch(Command::SplitPane {
                    workspace,
                    pane: PaneId::new(99),
                    axis: Axis::Vertical,
                    cwd: PathBuf::new()
                })
                .is_err()
        );
        assert_eq!(controller.model(), &before);
    }

    #[test]
    fn unchanged_split_ratio_does_not_schedule_storage_work() {
        let (mut controller, workspace, pane) = setup();
        controller
            .dispatch(Command::SplitPane {
                workspace,
                pane,
                axis: Axis::Vertical,
                cwd: PathBuf::from("/fake"),
            })
            .unwrap();
        let split = match controller.model().workspace(workspace).unwrap().layout() {
            Layout::Split { id, .. } => *id,
            _ => panic!("expected split"),
        };
        let generation = controller.generation();
        assert!(
            controller
                .dispatch(Command::SetSplitRatio { split, ratio: 0.5 })
                .unwrap()
                .is_empty()
        );
        assert_eq!(controller.generation(), generation);
    }

    #[test]
    fn mixed_command_sequences_always_preserve_restorable_layout_and_focus() {
        let (mut controller, _, _) = setup();
        let mut sequence = 0x1234_5678_u64;
        for step in 0..400 {
            sequence = sequence
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            if controller.model().workspaces().is_empty() {
                create(&mut controller, "replacement");
            }
            let index = (sequence as usize) % controller.model().workspaces().len();
            let ws = &controller.model().workspaces()[index];
            let workspace = ws.id();
            let pane = ws.panes()[(sequence.rotate_right(17) as usize) % ws.panes().len()].id();
            let command = match sequence % 7 {
                0 => Command::AddWorkspace {
                    cwd: PathBuf::from("/fake"),
                    name: format!("workspace {step}"),
                },
                1 => Command::SplitPane {
                    workspace,
                    pane,
                    axis: Axis::Horizontal,
                    cwd: PathBuf::from("/fake"),
                },
                2 => Command::ClosePane(pane),
                3 => Command::CloseWorkspace(workspace),
                4 => Command::FocusPane { workspace, pane },
                5 => Command::RenameWorkspace {
                    workspace,
                    name: format!("renamed {step}"),
                },
                _ => Command::SelectWorkspace(workspace),
            };
            let before = controller.model().clone();
            if controller.dispatch(command).is_err() {
                assert_eq!(controller.model(), &before);
            }
            assert!(
                Model::restore(
                    controller.model().specs(),
                    controller.model().active_workspace(),
                    controller.model().sidebar(),
                    controller.model().limits()
                )
                .is_ok()
            );
        }
    }

    #[test]
    fn restart_at_capacity_replaces_session_and_rejects_stale_completion() {
        let mut controller = Controller::new(Model::new(Limits {
            total_panes: 1,
            ..Limits::default()
        }));
        create(&mut controller, "main");
        let pane = controller.model().active_pane().unwrap();
        let effects = controller.dispatch(Command::RestartPane(pane)).unwrap();
        assert!(effects.contains(&Effect::StartSession {
            pane,
            generation: 2,
            cwd: PathBuf::from("/fake"),
            replacement: true
        }));
        controller
            .complete(Completion::Started {
                pane,
                generation: 1,
            })
            .unwrap();
        assert_eq!(
            controller.model().pane(pane).unwrap().lifecycle(),
            &Lifecycle::Starting
        );
        controller
            .complete(Completion::Failed {
                pane,
                generation: 2,
                error: "fixture failure".into(),
            })
            .unwrap();
        assert_eq!(
            controller.model().pane(pane).unwrap().lifecycle(),
            &Lifecycle::Failed("fixture failure".into())
        );
    }

    #[test]
    fn completion_after_close_is_ignored() {
        let (mut controller, _, pane) = setup();
        controller.dispatch(Command::ClosePane(pane)).unwrap();
        assert!(
            controller
                .complete(Completion::Started {
                    pane,
                    generation: 1
                })
                .unwrap()
                .is_empty()
        );
        assert!(controller.model().pane(pane).is_none());
    }

    #[test]
    fn saved_generation_cannot_acknowledge_newer_changes() {
        let (mut controller, _, _) = setup();
        let first = controller.generation();
        controller.dispatch(Command::SetSidebar(false)).unwrap();
        controller
            .complete(Completion::Saved { generation: first })
            .unwrap();
        assert!(controller.is_dirty());
        controller
            .complete(Completion::Saved {
                generation: u64::MAX,
            })
            .unwrap();
        assert!(controller.is_dirty());
        controller
            .complete(Completion::Saved {
                generation: controller.generation(),
            })
            .unwrap();
        assert!(!controller.is_dirty());
    }

    #[test]
    fn fake_runtime_drives_real_model_without_shells_fonts_or_gpu() {
        let mut controller = Controller::new(Model::default());
        let mut runtime = FakeRuntime::default();
        runtime.run(
            &mut controller,
            Command::AddWorkspace {
                cwd: PathBuf::from("/fake"),
                name: "fixture".into(),
            },
        );
        let pane = controller.model().active_pane().unwrap();
        assert_eq!(
            controller.model().pane(pane).unwrap().lifecycle(),
            &Lifecycle::Running
        );
        runtime.run(&mut controller, Command::RestartPane(pane));
        assert_eq!(controller.model().pane(pane).unwrap().generation(), 2);
        runtime.run(&mut controller, Command::ClosePane(pane));
        assert_eq!(controller.model().pane_count(), 0);
    }

    #[test]
    fn restoration_rejects_duplicate_missing_leaves_and_invalid_ratios() {
        let mut spec = WorkspaceSpec {
            id: WorkspaceId::new(1),
            name: "fixture".into(),
            cwd: PathBuf::new(),
            panes: vec![
                PaneSpec {
                    id: PaneId::new(1),
                    cwd: PathBuf::new(),
                },
                PaneSpec {
                    id: PaneId::new(2),
                    cwd: PathBuf::new(),
                },
            ],
            layout: Layout::Leaf(PaneId::new(1)),
            active: PaneId::new(1),
        };
        assert!(Model::restore(vec![spec.clone()], None, true, Limits::default()).is_err());
        spec.layout = Layout::Split {
            id: SplitId::new(1),
            axis: Axis::Vertical,
            ratio: 0.5,
            first: Box::new(Layout::Leaf(PaneId::new(1))),
            second: Box::new(Layout::Leaf(PaneId::new(1))),
        };
        assert!(Model::restore(vec![spec.clone()], None, true, Limits::default()).is_err());
        if let Layout::Split { ratio, second, .. } = &mut spec.layout {
            *ratio = f32::NAN;
            **second = Layout::Leaf(PaneId::new(2));
        }
        assert_eq!(
            Model::restore(vec![spec], None, true, Limits::default()),
            Err(Error::InvalidRatio)
        );
    }
}

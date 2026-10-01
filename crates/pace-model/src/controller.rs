use crate::{
    Axis, Edge, Error, Layout, Lifecycle, Model, Pane, PaneId, SplitId, Workspace, WorkspaceId,
};
use std::path::PathBuf;

/// Where a moved pane lands. The pane keeps its session and identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    /// Against one edge of another pane, in that pane's workspace.
    Beside { pane: PaneId, edge: Edge },
    /// In the place of another pane of the same workspace, which takes its place.
    Swap(PaneId),
    /// In another workspace, beside its roomiest pane.
    Workspace(WorkspaceId),
}

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
    /// Moving the last pane out of a workspace removes that workspace.
    MovePane {
        pane: PaneId,
        destination: Destination,
    },
    ClosePane(PaneId),
    CloseWorkspace(WorkspaceId),
    /// Moves a workspace to `index` in the ordered list. A position past the
    /// end means last.
    MoveWorkspace {
        workspace: WorkspaceId,
        index: usize,
    },
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
                let edge = match axis {
                    Axis::Vertical => Edge::Right,
                    Axis::Horizontal => Edge::Bottom,
                };
                if !ws.layout.split(pane, id, split, edge) {
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
            Command::MovePane { pane, destination } => dirty = self.move_pane(pane, destination)?,
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
            Command::MoveWorkspace { workspace, index } => {
                let position = self
                    .model
                    .workspaces
                    .iter()
                    .position(|item| item.id == workspace)
                    .ok_or(Error::UnknownWorkspace(workspace))?;
                let index = index.min(self.model.workspaces.len() - 1);
                if index != position {
                    let moved = self.model.workspaces.remove(position);
                    self.model.workspaces.insert(index, moved);
                    dirty = true;
                }
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

    /// Reports whether anything changed. Sessions are untouched: a pane's
    /// identity and generation do not depend on where it is shown.
    fn move_pane(&mut self, pane: PaneId, destination: Destination) -> Result<bool, Error> {
        let source = self
            .model
            .workspace_for_pane(pane)
            .ok_or(Error::UnknownPane(pane))?;
        let (target, edge) = match destination {
            Destination::Beside { pane: target, edge } => (target, edge),
            Destination::Swap(other) => {
                let ws = self.model.workspace_mut(source)?;
                if ws.pane(other).is_none() {
                    return Err(Error::UnknownPane(other));
                }
                let changed = other != pane || ws.active != pane;
                ws.layout.swap(pane, other);
                ws.active = pane;
                return Ok(changed);
            }
            Destination::Workspace(workspace) => {
                let ws = self
                    .model
                    .workspace(workspace)
                    .ok_or(Error::UnknownWorkspace(workspace))?;
                if workspace == source {
                    return Ok(false);
                }
                ws.layout.roomiest(ws.active)
            }
        };
        let destination = self
            .model
            .workspace_for_pane(target)
            .ok_or(Error::UnknownPane(target))?;
        if target == pane {
            return Ok(false);
        }
        let split = SplitId::new(self.model.next_split);
        let next_split = self
            .model
            .next_split
            .checked_add(1)
            .ok_or(Error::IdentityExhausted)?;
        // The target is another pane, so the source keeps a leaf unless the
        // moved pane was alone in a different workspace.
        let remaining = self
            .model
            .workspace(source)
            .and_then(|ws| ws.layout.clone().remove(pane));
        if destination == source {
            let mut layout = remaining.ok_or(Error::InvalidLayout("move removed every leaf"))?;
            layout.split(target, pane, split, edge);
            let ws = self.model.workspace_mut(source)?;
            if layout.same_arrangement(&ws.layout) {
                // Dropped where it already was: keep the split and its ratio.
                let changed = ws.active != pane;
                ws.active = pane;
                return Ok(changed);
            }
            ws.layout = layout;
            ws.active = pane;
        } else {
            let limit = self.model.limits.panes_per_workspace;
            if self
                .model
                .workspace(destination)
                .is_some_and(|ws| ws.panes.len() >= limit)
            {
                return Err(Error::PaneLimit);
            }
            let ws = self.model.workspace_mut(source)?;
            let position = ws
                .panes
                .iter()
                .position(|item| item.id == pane)
                .ok_or(Error::UnknownPane(pane))?;
            let moved = ws.panes.remove(position);
            if let Some(layout) = remaining {
                ws.layout = layout;
                if ws.active == pane {
                    ws.active = ws.panes[position.min(ws.panes.len() - 1)].id;
                }
            } else {
                self.model.workspaces.retain(|item| item.id != source);
                if self.model.active == Some(source) {
                    self.model.active = Some(destination);
                }
            }
            let ws = self.model.workspace_mut(destination)?;
            ws.layout.split(target, pane, split, edge);
            ws.panes.push(moved);
            ws.active = pane;
        }
        self.model.next_split = next_split;
        Ok(true)
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
    fn moving_a_workspace_reorders_without_changing_identity_or_focus() {
        let (mut controller, first, _) = setup();
        let second = create(&mut controller, "second");
        let third = create(&mut controller, "third");
        controller
            .dispatch(Command::SelectWorkspace(second))
            .unwrap();
        let pane = controller.model().active_pane();
        let order = |controller: &Controller| {
            controller
                .model()
                .workspaces()
                .iter()
                .map(Workspace::id)
                .collect::<Vec<_>>()
        };
        let generation = controller.generation();
        let effects = controller
            .dispatch(Command::MoveWorkspace {
                workspace: first,
                index: 2,
            })
            .unwrap();
        assert_eq!(order(&controller), [second, third, first]);
        assert_eq!(
            effects,
            [Effect::Persist {
                generation: generation + 1
            }],
            "sessions, focus and search are untouched"
        );
        assert_eq!(controller.model().active_workspace(), Some(second));
        assert_eq!(controller.model().active_pane(), pane);
        // A position past the end means last.
        controller
            .dispatch(Command::MoveWorkspace {
                workspace: second,
                index: usize::MAX,
            })
            .unwrap();
        assert_eq!(order(&controller), [third, first, second]);
        controller
            .dispatch(Command::MoveWorkspace {
                workspace: second,
                index: 0,
            })
            .unwrap();
        assert_eq!(order(&controller), [second, third, first]);
    }

    #[test]
    fn moving_to_the_same_position_or_an_unknown_workspace_changes_nothing() {
        let (mut controller, first, _) = setup();
        create(&mut controller, "second");
        let before = controller.model().clone();
        let generation = controller.generation();
        assert!(
            controller
                .dispatch(Command::MoveWorkspace {
                    workspace: first,
                    index: 0
                })
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            controller.dispatch(Command::MoveWorkspace {
                workspace: WorkspaceId::new(99),
                index: 0
            }),
            Err(Error::UnknownWorkspace(WorkspaceId::new(99)))
        );
        assert_eq!(controller.model(), &before);
        assert_eq!(controller.generation(), generation);
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

    fn split(controller: &mut Controller, pane: PaneId, axis: Axis) -> PaneId {
        let workspace = controller.model().workspace_for_pane(pane).unwrap();
        controller
            .dispatch(Command::SplitPane {
                workspace,
                pane,
                axis,
                cwd: PathBuf::from("/fake"),
            })
            .unwrap();
        controller.model().workspace(workspace).unwrap().active()
    }
    fn split_ids(layout: &Layout) -> Vec<SplitId> {
        match layout {
            Layout::Leaf(_) => Vec::new(),
            Layout::Split {
                id, first, second, ..
            } => [vec![*id], split_ids(first), split_ids(second)].concat(),
        }
    }
    fn without_save(effects: Vec<Effect>) -> Vec<Effect> {
        effects
            .into_iter()
            .filter(|effect| !matches!(effect, Effect::Persist { .. }))
            .collect()
    }

    #[test]
    fn moving_a_pane_rearranges_the_layout_without_touching_sessions() {
        let (mut controller, workspace, first) = setup();
        let second = split(&mut controller, first, Axis::Vertical);
        let third = split(&mut controller, second, Axis::Horizontal);
        let inner = split_ids(controller.model().workspace(workspace).unwrap().layout())[1];
        let effects = controller
            .dispatch(Command::MovePane {
                pane: first,
                destination: Destination::Beside {
                    pane: third,
                    edge: Edge::Bottom,
                },
            })
            .unwrap();
        // The moved pane takes focus; no session starts or stops.
        assert_eq!(
            without_save(effects),
            [
                Effect::Focus {
                    old: Some(third),
                    new: Some(first)
                },
                Effect::ResetSearch
            ]
        );
        let ws = controller.model().workspace(workspace).unwrap();
        assert_eq!(ws.layout().leaves(), [second, third, first]);
        assert_eq!(ws.panes().len(), 3);
        assert_eq!(ws.pane(first).unwrap().generation(), 1);
        assert!(
            split_ids(ws.layout()).contains(&inner),
            "unrelated split kept"
        );
        assert!(matches!(
            ws.layout(),
            Layout::Split { axis: Axis::Horizontal, second: below, .. }
                if matches!(&**below, Layout::Split { axis: Axis::Horizontal, .. })
        ));
    }

    #[test]
    fn moving_a_pane_to_where_it_already_is_keeps_the_split_and_its_ratio() {
        let (mut controller, workspace, first) = setup();
        let second = split(&mut controller, first, Axis::Vertical);
        let id = split_ids(controller.model().workspace(workspace).unwrap().layout())[0];
        controller
            .dispatch(Command::SetSplitRatio {
                split: id,
                ratio: 0.3,
            })
            .unwrap();
        let before = controller.model().clone();
        let generation = controller.generation();
        for destination in [
            Destination::Beside {
                pane: first,
                edge: Edge::Right,
            },
            Destination::Beside {
                pane: second,
                edge: Edge::Left,
            },
            Destination::Swap(second),
            Destination::Workspace(workspace),
        ] {
            let effects = controller
                .dispatch(Command::MovePane {
                    pane: second,
                    destination,
                })
                .unwrap();
            assert!(effects.is_empty(), "{destination:?}");
        }
        assert_eq!(controller.model(), &before);
        assert_eq!(controller.generation(), generation);
    }

    #[test]
    fn swapping_panes_exchanges_their_places_and_keeps_every_split() {
        let (mut controller, workspace, first) = setup();
        let second = split(&mut controller, first, Axis::Vertical);
        let third = split(&mut controller, second, Axis::Horizontal);
        let splits = split_ids(controller.model().workspace(workspace).unwrap().layout());
        controller
            .dispatch(Command::MovePane {
                pane: third,
                destination: Destination::Swap(first),
            })
            .unwrap();
        let ws = controller.model().workspace(workspace).unwrap();
        assert_eq!(ws.layout().leaves(), [third, second, first]);
        assert_eq!(split_ids(ws.layout()), splits);
        assert_eq!(ws.active(), third);
        assert!(controller.is_dirty());
    }

    #[test]
    fn moving_a_pane_to_another_workspace_keeps_the_view_and_the_session() {
        let (mut controller, home, first) = setup();
        let second = split(&mut controller, first, Axis::Vertical);
        let other = create(&mut controller, "other");
        let resident = controller.model().active_pane().unwrap();
        controller.dispatch(Command::SelectWorkspace(home)).unwrap();
        let effects = controller
            .dispatch(Command::MovePane {
                pane: second,
                destination: Destination::Workspace(other),
            })
            .unwrap();
        // The view stays on the source workspace, whose neighbour takes focus.
        assert_eq!(
            without_save(effects),
            [
                Effect::Focus {
                    old: Some(second),
                    new: Some(first)
                },
                Effect::ResetSearch
            ]
        );
        let model = controller.model();
        assert_eq!(model.active_workspace(), Some(home));
        assert_eq!(
            model.workspace(home).unwrap().layout(),
            &Layout::Leaf(first)
        );
        let destination = model.workspace(other).unwrap();
        assert_eq!(destination.layout().leaves(), [resident, second]);
        assert_eq!(destination.active(), second);
        assert_eq!(model.workspace_for_pane(second), Some(other));
        assert_eq!(model.pane_count(), 3);
        // The session it carried still reports into the same pane.
        controller
            .complete(Completion::Started {
                pane: second,
                generation: 1,
            })
            .unwrap();
        assert_eq!(
            controller.model().pane(second).unwrap().lifecycle(),
            &Lifecycle::Running
        );
    }

    #[test]
    fn panes_sent_to_a_workspace_fill_its_roomiest_pane_instead_of_stacking() {
        let (mut controller, home, resident) = setup();
        let mut arrivals = Vec::new();
        for _ in 0..3 {
            create(&mut controller, "source");
            arrivals.push(controller.model().active_pane().unwrap());
            controller
                .dispatch(Command::MovePane {
                    pane: *arrivals.last().unwrap(),
                    destination: Destination::Workspace(home),
                })
                .unwrap();
        }
        // An even grid: each arrival halves the largest pane along its longer
        // side, starting with the focused one when several are as large.
        let quarter = |layout: &Layout, top: PaneId, bottom: PaneId| {
            matches!(
                layout,
                Layout::Split { axis: Axis::Horizontal, first, second, .. }
                    if **first == Layout::Leaf(top) && **second == Layout::Leaf(bottom)
            )
        };
        let ws = controller.model().workspace(home).unwrap();
        assert!(matches!(
            ws.layout(),
            Layout::Split { axis: Axis::Vertical, first, second, .. }
                if quarter(first, resident, arrivals[2]) && quarter(second, arrivals[0], arrivals[1])
        ));
        assert_eq!(ws.active(), arrivals[2]);
        assert_eq!(controller.model().workspaces().len(), 1);
    }

    #[test]
    fn moving_the_last_pane_out_removes_its_workspace_and_follows_the_pane() {
        let (mut controller, home, first) = setup();
        let other = create(&mut controller, "other");
        let alone = controller.model().active_pane().unwrap();
        let effects = controller
            .dispatch(Command::MovePane {
                pane: alone,
                destination: Destination::Beside {
                    pane: first,
                    edge: Edge::Left,
                },
            })
            .unwrap();
        assert!(
            !effects
                .iter()
                .any(|effect| matches!(effect, Effect::StopSession { .. } | Effect::Focus { .. })),
            "the same pane stays focused in its new workspace"
        );
        let model = controller.model();
        assert!(model.workspace(other).is_none());
        assert_eq!(model.active_workspace(), Some(home));
        assert_eq!(model.active_pane(), Some(alone));
        assert_eq!(
            model.workspace(home).unwrap().layout().leaves(),
            [alone, first]
        );
    }

    #[test]
    fn rejected_moves_are_atomic() {
        let mut controller = Controller::new(Model::new(Limits {
            panes_per_workspace: 2,
            ..Limits::default()
        }));
        let home = create(&mut controller, "home");
        let first = controller.model().active_pane().unwrap();
        let second = split(&mut controller, first, Axis::Vertical);
        let other = create(&mut controller, "other");
        let third = controller.model().active_pane().unwrap();
        let before = controller.model().clone();
        let generation = controller.generation();
        for (pane, destination, error) in [
            (third, Destination::Workspace(home), Error::PaneLimit),
            (
                third,
                Destination::Beside {
                    pane: second,
                    edge: Edge::Top,
                },
                Error::PaneLimit,
            ),
            // Positions are exchanged only inside one workspace.
            (third, Destination::Swap(first), Error::UnknownPane(first)),
            (
                third,
                Destination::Workspace(WorkspaceId::new(99)),
                Error::UnknownWorkspace(WorkspaceId::new(99)),
            ),
            (
                PaneId::new(99),
                Destination::Workspace(other),
                Error::UnknownPane(PaneId::new(99)),
            ),
        ] {
            assert_eq!(
                controller.dispatch(Command::MovePane { pane, destination }),
                Err(error)
            );
        }
        assert_eq!(controller.model(), &before);
        assert_eq!(controller.generation(), generation);
        // Rearranging a full workspace needs no spare capacity.
        controller
            .dispatch(Command::MovePane {
                pane: first,
                destination: Destination::Beside {
                    pane: second,
                    edge: Edge::Bottom,
                },
            })
            .unwrap();
        assert_eq!(
            controller
                .model()
                .workspace(home)
                .unwrap()
                .layout()
                .leaves(),
            [second, first]
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
            let other = controller.model().workspaces()
                [(sequence.rotate_right(29) as usize) % controller.model().workspaces().len()]
            .active();
            let command = match sequence % 11 {
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
                6 => Command::MoveWorkspace {
                    workspace,
                    index: (sequence.rotate_right(29) as usize) % 30,
                },
                7 => Command::MovePane {
                    pane,
                    destination: Destination::Beside {
                        pane: other,
                        edge: [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom]
                            [(sequence.rotate_right(41) as usize) % 4],
                    },
                },
                8 => Command::MovePane {
                    pane,
                    destination: Destination::Swap(other),
                },
                9 => Command::MovePane {
                    pane: other,
                    destination: Destination::Workspace(workspace),
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

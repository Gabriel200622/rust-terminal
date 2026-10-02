use crate::{
    Axis, Edge, Error, Layout, Lifecycle, Model, Pane, PaneId, Remote, SidebarItem, SplitId,
    Workspace, WorkspaceGroup, WorkspaceGroupId, WorkspaceId,
};
use std::path::PathBuf;

/// Where a moved pane lands. The pane keeps its session and identity, so it
/// can only enter a workspace on the same machine as its own.
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
        group: Option<WorkspaceGroupId>,
        /// An SSH destination: every terminal of the workspace opens there.
        remote: Option<String>,
    },
    AddWorkspaceGroup {
        name: String,
    },
    RenameWorkspaceGroup {
        group: WorkspaceGroupId,
        name: String,
    },
    SetWorkspaceGroupCollapsed {
        group: WorkspaceGroupId,
        collapsed: bool,
    },
    SetWorkspaceGroup {
        workspace: WorkspaceId,
        group: Option<WorkspaceGroupId>,
    },
    /// Removes the folder, retaining its workspaces and every running session.
    RemoveWorkspaceGroup(WorkspaceGroupId),
    /// Moves a folder to `index` in the group order; a position past the end
    /// means last. Membership, order within each folder and sessions stay intact.
    MoveWorkspaceGroup {
        group: WorkspaceGroupId,
        index: usize,
    },
    /// Moves a group (with all its children) or ungrouped workspace to a
    /// top-level sidebar index. A position past the end means last.
    MoveSidebarItem {
        item: SidebarItem,
        index: usize,
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
    /// Connect a workspace to an SSH destination, or return it to local
    /// shells with `None`. Every terminal it holds is replaced, because a
    /// running session cannot move to another machine.
    SetWorkspaceRemote {
        workspace: WorkspaceId,
        remote: Option<String>,
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
    /// A directory explicitly reported by the remote shell, never a local path.
    PaneRemoteCwdChanged {
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
        /// Where the session runs; `None` is a local shell.
        remote: Option<Remote>,
        remote_cwd: Option<PathBuf>,
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
            .flat_map(|workspace| {
                workspace.panes.iter().map(|pane| Effect::StartSession {
                    pane: pane.id,
                    generation: pane.generation,
                    cwd: pane.cwd.clone(),
                    remote: workspace.remote.clone(),
                    remote_cwd: pane.remote_cwd.clone(),
                    replacement: false,
                })
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
            Command::AddWorkspace {
                cwd,
                name,
                remote,
                group,
            } => {
                if name.trim().is_empty() {
                    return Err(Error::InvalidName);
                }
                let remote = remote.as_deref().map(Remote::parse).transpose()?;
                if let Some(group) = group
                    && self.model.group(group).is_none()
                {
                    return Err(Error::UnknownWorkspaceGroup(group));
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
                    group,
                    name,
                    cwd: cwd.clone(),
                    remote: remote.clone(),
                    panes: vec![Pane {
                        id: pane_id,
                        cwd: cwd.clone(),
                        remote_cwd: None,
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
                    remote,
                    remote_cwd: None,
                    replacement: false,
                });
                dirty = true;
            }
            Command::AddWorkspaceGroup { name } => {
                if name.trim().is_empty() {
                    return Err(Error::InvalidName);
                }
                if self.model.groups.len() >= self.model.limits.workspaces {
                    return Err(Error::WorkspaceGroupLimit);
                }
                let id = WorkspaceGroupId::new(self.model.next_group);
                self.model.next_group = self
                    .model
                    .next_group
                    .checked_add(1)
                    .ok_or(Error::IdentityExhausted)?;
                self.model.groups.push(WorkspaceGroup {
                    id,
                    name,
                    collapsed: false,
                });
                dirty = true;
            }
            Command::RenameWorkspaceGroup { group, name } => {
                if name.trim().is_empty() {
                    return Err(Error::InvalidName);
                }
                let folder = self.model.group_mut(group)?;
                if folder.name != name {
                    folder.name = name;
                    dirty = true;
                }
            }
            Command::SetWorkspaceGroupCollapsed { group, collapsed } => {
                let folder = self.model.group_mut(group)?;
                if folder.collapsed != collapsed {
                    folder.collapsed = collapsed;
                    dirty = true;
                }
            }
            Command::SetWorkspaceGroup { workspace, group } => {
                if let Some(group) = group
                    && self.model.group(group).is_none()
                {
                    return Err(Error::UnknownWorkspaceGroup(group));
                }
                let ws = self.model.workspace_mut(workspace)?;
                if ws.group != group {
                    ws.group = group;
                    dirty = true;
                }
                if let Some(group) = group {
                    let folder = self.model.group_mut(group)?;
                    dirty |= folder.collapsed;
                    folder.collapsed = false;
                }
            }
            Command::RemoveWorkspaceGroup(group) => {
                let position = self
                    .model
                    .groups
                    .iter()
                    .position(|folder| folder.id == group)
                    .ok_or(Error::UnknownWorkspaceGroup(group))?;
                if let Some(index) = self
                    .model
                    .sidebar_order
                    .iter()
                    .position(|item| *item == SidebarItem::Group(group))
                {
                    let children = self
                        .model
                        .workspaces
                        .iter()
                        .filter(|w| w.group == Some(group))
                        .map(|w| SidebarItem::Workspace(w.id));
                    self.model.sidebar_order.splice(index..=index, children);
                }
                self.model.groups.remove(position);
                for workspace in &mut self.model.workspaces {
                    if workspace.group == Some(group) {
                        workspace.group = None;
                    }
                }
                dirty = true;
            }
            Command::MoveWorkspaceGroup { group, index } => {
                let position = self
                    .model
                    .groups
                    .iter()
                    .position(|folder| folder.id == group)
                    .ok_or(Error::UnknownWorkspaceGroup(group))?;
                let index = index.min(self.model.groups.len() - 1);
                if index != position {
                    let target = SidebarItem::Group(self.model.groups[index].id);
                    let target_index = self
                        .model
                        .sidebar_order
                        .iter()
                        .position(|item| *item == target)
                        .ok_or(Error::InvalidSidebarOrder)?;
                    self.move_sidebar_item(SidebarItem::Group(group), target_index)?;
                    dirty = true;
                }
            }
            Command::MoveSidebarItem { item, index } => {
                dirty = self.move_sidebar_item(item, index)?;
            }
            Command::SelectWorkspace(id) => {
                if self.model.workspace(id).is_none() {
                    return Err(Error::UnknownWorkspace(id));
                }
                if self.model.active != Some(id) {
                    self.model.active = Some(id);
                    dirty = true;
                }
                if let Some(group) = self.model.workspace(id).and_then(Workspace::group) {
                    let folder = self.model.group_mut(group)?;
                    dirty |= folder.collapsed;
                    folder.collapsed = false;
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
                let remote_cwd = ws
                    .pane(pane)
                    .ok_or(Error::UnknownPane(pane))?
                    .remote_cwd
                    .clone();
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
                    remote_cwd: remote_cwd.clone(),
                    generation: 1,
                    lifecycle: Lifecycle::Starting,
                });
                ws.active = id;
                let remote = ws.remote.clone();
                self.model.active = Some(workspace);
                self.model.next_pane = next_pane;
                self.model.next_split = next_split;
                effects.push(Effect::StartSession {
                    pane: id,
                    generation: 1,
                    cwd,
                    remote,
                    remote_cwd,
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
                    if self.model.workspaces[position].group.is_none() {
                        let target = &self.model.workspaces[index];
                        let target = target
                            .group
                            .map_or(SidebarItem::Workspace(target.id), SidebarItem::Group);
                        let target_index = self
                            .model
                            .sidebar_order
                            .iter()
                            .position(|item| *item == target)
                            .ok_or(Error::InvalidSidebarOrder)?;
                        self.move_sidebar_item(SidebarItem::Workspace(workspace), target_index)?;
                    }
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
            Command::SetWorkspaceRemote { workspace, remote } => {
                let remote = remote.as_deref().map(Remote::parse).transpose()?;
                let ws = self.model.workspace_mut(workspace)?;
                if ws.remote != remote {
                    if ws
                        .panes
                        .iter()
                        .any(|pane| pane.generation.checked_add(1).is_none())
                    {
                        return Err(Error::IdentityExhausted);
                    }
                    ws.remote.clone_from(&remote);
                    for pane in &mut ws.panes {
                        let previous = pane.generation;
                        pane.generation += 1;
                        pane.lifecycle = Lifecycle::Starting;
                        pane.remote_cwd = None;
                        effects.push(Effect::StopSession {
                            pane: pane.id,
                            generation: previous,
                        });
                        effects.push(Effect::StartSession {
                            pane: pane.id,
                            generation: pane.generation,
                            cwd: pane.cwd.clone(),
                            remote: remote.clone(),
                            remote_cwd: None,
                            replacement: true,
                        });
                    }
                    if old_focus.is_some_and(|pane| ws.pane(pane).is_some()) {
                        effects.push(Effect::ResetSearch);
                    }
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
                let remote = self
                    .model
                    .workspace_for_pane(pane)
                    .and_then(|id| self.model.workspace(id))
                    .and_then(|workspace| workspace.remote.clone());
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
                    remote,
                    remote_cwd: item.remote_cwd.clone(),
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
            Command::PaneRemoteCwdChanged {
                pane,
                generation,
                cwd,
            } => {
                let remote = self
                    .model
                    .workspace_for_pane(pane)
                    .and_then(|id| self.model.workspace(id))
                    .is_some_and(|workspace| workspace.remote.is_some());
                if remote
                    && let Ok(item) = self.model.pane_mut(pane)
                    && item.generation == generation
                    && item.remote_cwd.as_ref() != Some(&cwd)
                {
                    item.remote_cwd = Some(cwd);
                    dirty = true;
                }
            }
        }
        if dirty {
            self.model.order_workspaces();
        }
        let new_focus = self.model.active_pane();
        if old_focus != new_focus {
            if let Some(group) = self
                .model
                .active
                .and_then(|id| self.model.workspace(id))
                .and_then(Workspace::group)
            {
                let folder = self.model.group_mut(group)?;
                dirty |= folder.collapsed;
                folder.collapsed = false;
            }
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
            // A local shell would be shown as running on the host, and a
            // connection as a local shell, until the next restart.
            if self.model.workspace(source).map(|ws| &ws.remote)
                != self.model.workspace(destination).map(|ws| &ws.remote)
            {
                return Err(Error::RemoteMismatch);
            }
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

    fn move_sidebar_item(&mut self, item: SidebarItem, index: usize) -> Result<bool, Error> {
        match item {
            SidebarItem::Workspace(id) => {
                let workspace = self
                    .model
                    .workspace(id)
                    .ok_or(Error::UnknownWorkspace(id))?;
                if workspace.group().is_some() {
                    return Err(Error::InvalidSidebarOrder);
                }
            }
            SidebarItem::Group(id) => {
                self.model
                    .group(id)
                    .ok_or(Error::UnknownWorkspaceGroup(id))?;
            }
        }
        let position = self
            .model
            .sidebar_order
            .iter()
            .position(|entry| *entry == item)
            .ok_or(Error::InvalidSidebarOrder)?;
        let index = index.min(self.model.sidebar_order.len() - 1);
        if position == index {
            return Ok(false);
        }
        self.model.sidebar_order.remove(position);
        self.model.sidebar_order.insert(index, item);
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
                group: None,
                cwd: PathBuf::from("/fake"),
                name: name.into(),
                remote: None,
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
            let command = match sequence % 12 {
                0 => Command::AddWorkspace {
                    group: None,
                    cwd: PathBuf::from("/fake"),
                    name: format!("workspace {step}"),
                    remote: (step % 3 == 0).then(|| "me@devbox".to_owned()),
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
                6 => Command::SetWorkspaceRemote {
                    workspace,
                    remote: (step % 2 == 0).then(|| format!("host{step}")),
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
                10 => Command::MoveWorkspace {
                    workspace,
                    index: (sequence.rotate_right(29) as usize) % 30,
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
            remote: None,
            remote_cwd: None,
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
                group: None,
                cwd: PathBuf::from("/fake"),
                name: "fixture".into(),
                remote: None,
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

    fn remote(destination: &str) -> Option<Remote> {
        Some(Remote::parse(destination).unwrap())
    }
    fn starts(effects: &[Effect]) -> Vec<(PaneId, u64, Option<Remote>, bool)> {
        effects
            .iter()
            .filter_map(|effect| match effect {
                Effect::StartSession {
                    pane,
                    generation,
                    remote,
                    replacement,
                    ..
                } => Some((*pane, *generation, remote.clone(), *replacement)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn remote_directories_follow_each_pane_through_split_restart_and_restore() {
        let (mut controller, workspace, first) = setup();
        controller
            .dispatch(Command::SetWorkspaceRemote {
                workspace,
                remote: Some("devbox".into()),
            })
            .unwrap();
        let directory = PathBuf::from("/srv/project ' % λ");
        let generation = controller.model().pane(first).unwrap().generation();
        controller
            .dispatch(Command::PaneRemoteCwdChanged {
                pane: first,
                generation,
                cwd: directory.clone(),
            })
            .unwrap();
        let split = controller
            .dispatch(Command::SplitPane {
                workspace,
                pane: first,
                axis: Axis::Vertical,
                cwd: PathBuf::from("/fake"),
            })
            .unwrap();
        let second = controller.model().active_pane().unwrap();
        assert!(split.iter().any(|effect| matches!(effect, Effect::StartSession { pane, remote_cwd: Some(cwd), .. } if *pane == second && *cwd == directory)));
        controller
            .dispatch(Command::PaneRemoteCwdChanged {
                pane: second,
                generation: 1,
                cwd: PathBuf::from("/srv/other"),
            })
            .unwrap();
        let restarted = controller.dispatch(Command::RestartPane(first)).unwrap();
        assert!(restarted.iter().any(|effect| matches!(effect, Effect::StartSession { pane, remote_cwd: Some(cwd), .. } if *pane == first && *cwd == directory)));
        let restored = Controller::new(
            Model::restore(
                controller.model().specs(),
                Some(workspace),
                true,
                Limits::default(),
            )
            .unwrap(),
        );
        let directories: Vec<_> = restored
            .start_effects()
            .into_iter()
            .filter_map(|effect| match effect {
                Effect::StartSession { remote_cwd, .. } => remote_cwd,
                _ => None,
            })
            .collect();
        assert_eq!(directories, [directory, PathBuf::from("/srv/other")]);
        for pane in [first, second] {
            assert_eq!(
                restored.model().pane(pane).unwrap().cwd(),
                std::path::Path::new("/fake")
            );
        }
    }

    #[test]
    fn remote_directory_reports_ignore_stale_closed_and_local_sessions() {
        let (mut controller, workspace, pane) = setup();
        let report = |generation| Command::PaneRemoteCwdChanged {
            pane,
            generation,
            cwd: PathBuf::from("/srv/project"),
        };
        let generation = controller.generation();
        assert!(controller.dispatch(report(1)).unwrap().is_empty());
        assert_eq!(controller.generation(), generation);
        controller
            .dispatch(Command::SetWorkspaceRemote {
                workspace,
                remote: Some("devbox".into()),
            })
            .unwrap();
        assert!(controller.dispatch(report(1)).unwrap().is_empty());
        assert!(
            controller
                .dispatch(report(2))
                .unwrap()
                .iter()
                .any(|effect| matches!(effect, Effect::Persist { .. }))
        );
        assert!(controller.dispatch(report(2)).unwrap().is_empty());
        controller.dispatch(Command::RestartPane(pane)).unwrap();
        assert!(controller.dispatch(report(2)).unwrap().is_empty());
        assert_eq!(
            controller.model().pane(pane).unwrap().remote_cwd(),
            Some(std::path::Path::new("/srv/project"))
        );
        controller.dispatch(Command::ClosePane(pane)).unwrap();
        assert!(controller.dispatch(report(3)).unwrap().is_empty());
    }

    #[test]
    fn changing_ssh_hosts_and_disconnecting_clear_remote_directories() {
        let (mut controller, workspace, pane) = setup();
        for destination in [Some("devbox"), Some("otherbox"), None] {
            let effects = controller
                .dispatch(Command::SetWorkspaceRemote {
                    workspace,
                    remote: destination.map(str::to_owned),
                })
                .unwrap();
            assert_eq!(controller.model().pane(pane).unwrap().remote_cwd(), None);
            assert!(effects.iter().any(|effect| matches!(
                effect,
                Effect::StartSession {
                    remote_cwd: None,
                    ..
                }
            )));
            if destination.is_some() {
                let generation = controller.model().pane(pane).unwrap().generation();
                controller
                    .dispatch(Command::PaneRemoteCwdChanged {
                        pane,
                        generation,
                        cwd: PathBuf::from("/srv/project"),
                    })
                    .unwrap();
            }
        }
    }

    #[test]
    fn every_terminal_of_a_remote_workspace_starts_on_its_host() {
        let mut controller = Controller::new(Model::default());
        let created = controller
            .dispatch(Command::AddWorkspace {
                group: None,
                cwd: PathBuf::from("/fake"),
                name: "devbox".into(),
                remote: Some(" me@devbox ".into()),
            })
            .unwrap();
        let workspace = controller.model().active_workspace().unwrap();
        let first = controller.model().active_pane().unwrap();
        assert_eq!(
            controller.model().workspace(workspace).unwrap().remote(),
            remote("me@devbox").as_ref()
        );
        assert_eq!(starts(&created), [(first, 1, remote("me@devbox"), false)]);

        let split = controller
            .dispatch(Command::SplitPane {
                workspace,
                pane: first,
                axis: Axis::Vertical,
                cwd: PathBuf::from("/fake"),
            })
            .unwrap();
        let second = controller.model().active_pane().unwrap();
        assert_eq!(starts(&split), [(second, 1, remote("me@devbox"), false)]);

        let restarted = controller.dispatch(Command::RestartPane(first)).unwrap();
        assert_eq!(starts(&restarted), [(first, 2, remote("me@devbox"), true)]);

        // A local neighbour is unaffected, and restoration reconnects.
        create(&mut controller, "local");
        let restored = Controller::new(
            Model::restore(
                controller.model().specs(),
                None,
                true,
                controller.model().limits(),
            )
            .unwrap(),
        );
        let remotes: Vec<_> = starts(&restored.start_effects())
            .into_iter()
            .map(|(_, _, remote, _)| remote)
            .collect();
        assert_eq!(remotes, [remote("me@devbox"), remote("me@devbox"), None]);
    }

    #[test]
    fn connecting_a_workspace_replaces_all_of_its_terminals_and_only_those() {
        let (mut controller, workspace, first) = setup();
        controller
            .dispatch(Command::SplitPane {
                workspace,
                pane: first,
                axis: Axis::Horizontal,
                cwd: PathBuf::from("/fake"),
            })
            .unwrap();
        let second = controller.model().active_pane().unwrap();
        let other = create(&mut controller, "other");
        let other_pane = controller.model().active_pane().unwrap();
        for pane in [first, second, other_pane] {
            controller
                .complete(Completion::Started {
                    pane,
                    generation: 1,
                })
                .unwrap();
        }
        let saved = controller.generation();

        let effects = controller
            .dispatch(Command::SetWorkspaceRemote {
                workspace,
                remote: Some("me@devbox".into()),
            })
            .unwrap();
        assert_eq!(
            starts(&effects),
            [
                (first, 2, remote("me@devbox"), true),
                (second, 2, remote("me@devbox"), true)
            ]
        );
        for pane in [first, second] {
            assert!(effects.contains(&Effect::StopSession {
                pane,
                generation: 1
            }));
            assert_eq!(
                controller.model().pane(pane).unwrap().lifecycle(),
                &Lifecycle::Starting
            );
        }
        assert!(effects.contains(&Effect::Persist {
            generation: saved + 1
        }));
        // The focused pane is elsewhere: its search and session are untouched.
        assert!(!effects.contains(&Effect::ResetSearch));
        assert_eq!(controller.model().pane(other_pane).unwrap().generation(), 1);
        assert_eq!(controller.model().workspace(other).unwrap().remote(), None);
        // The local shell's exit cannot reach its replacement.
        controller
            .complete(Completion::Exited {
                pane: first,
                generation: 1,
            })
            .unwrap();
        assert_eq!(
            controller.model().pane(first).unwrap().lifecycle(),
            &Lifecycle::Starting
        );

        // The same host again is not a reason to drop the connections.
        assert!(
            controller
                .dispatch(Command::SetWorkspaceRemote {
                    workspace,
                    remote: Some("me@devbox".into()),
                })
                .unwrap()
                .is_empty()
        );

        // Disconnecting is the reverse action.
        controller
            .dispatch(Command::SelectWorkspace(workspace))
            .unwrap();
        let effects = controller
            .dispatch(Command::SetWorkspaceRemote {
                workspace,
                remote: None,
            })
            .unwrap();
        assert_eq!(
            starts(&effects),
            [(first, 3, None, true), (second, 3, None, true)]
        );
        assert!(effects.contains(&Effect::ResetSearch));
        assert_eq!(
            controller.model().workspace(workspace).unwrap().remote(),
            None
        );
    }

    #[test]
    fn a_terminal_moves_only_between_workspaces_on_the_same_machine() {
        let mut controller = Controller::new(Model::default());
        let add = |controller: &mut Controller, remote: Option<&str>| {
            controller
                .dispatch(Command::AddWorkspace {
                    group: None,
                    cwd: PathBuf::from("/fake"),
                    name: "workspace".into(),
                    remote: remote.map(str::to_owned),
                })
                .unwrap();
            (
                controller.model().active_workspace().unwrap(),
                controller.model().active_pane().unwrap(),
            )
        };
        let (local, local_pane) = add(&mut controller, None);
        let (devbox, devbox_pane) = add(&mut controller, Some("me@devbox"));
        let (same_host, same_host_pane) = add(&mut controller, Some("me@devbox"));
        let (_, other_host_pane) = add(&mut controller, Some("me@buildbox"));
        let before = controller.model().clone();
        for (pane, destination) in [
            (local_pane, Destination::Workspace(devbox)),
            (devbox_pane, Destination::Workspace(local)),
            (
                other_host_pane,
                Destination::Beside {
                    pane: devbox_pane,
                    edge: Edge::Right,
                },
            ),
        ] {
            assert_eq!(
                controller.dispatch(Command::MovePane { pane, destination }),
                Err(Error::RemoteMismatch)
            );
        }
        assert_eq!(controller.model(), &before);

        // The same host is the same machine: the session moves untouched.
        let effects = controller
            .dispatch(Command::MovePane {
                pane: same_host_pane,
                destination: Destination::Workspace(devbox),
            })
            .unwrap();
        assert!(starts(&effects).is_empty());
        assert!(controller.model().workspace(same_host).is_none());
        assert_eq!(
            controller.model().workspace(devbox).unwrap().panes().len(),
            2
        );
    }

    #[test]
    fn an_unusable_ssh_destination_is_refused_without_changing_anything() {
        let (mut controller, workspace, _) = setup();
        let before = controller.model().clone();
        let generation = controller.generation();
        for command in [
            Command::AddWorkspace {
                group: None,
                cwd: PathBuf::from("/fake"),
                name: "bad".into(),
                remote: Some("-oProxyCommand=id".into()),
            },
            Command::SetWorkspaceRemote {
                workspace,
                remote: Some("two words".into()),
            },
        ] {
            assert_eq!(controller.dispatch(command), Err(Error::InvalidRemote));
        }
        assert_eq!(
            controller.dispatch(Command::SetWorkspaceRemote {
                workspace: WorkspaceId::new(99),
                remote: None,
            }),
            Err(Error::UnknownWorkspace(WorkspaceId::new(99)))
        );
        assert_eq!(controller.model(), &before);
        assert_eq!(controller.generation(), generation);

        let mut specs = before.specs();
        specs[0].remote = Some("-oProxyCommand=id".into());
        assert_eq!(
            Model::restore(specs, None, true, Limits::default()),
            Err(Error::InvalidRemote)
        );
    }

    #[test]
    fn restoration_rejects_duplicate_missing_leaves_and_invalid_ratios() {
        let mut spec = WorkspaceSpec {
            group: None,
            id: WorkspaceId::new(1),
            name: "fixture".into(),
            cwd: PathBuf::new(),
            remote: None,
            panes: vec![
                PaneSpec {
                    id: PaneId::new(1),
                    cwd: PathBuf::new(),
                    remote_cwd: None,
                },
                PaneSpec {
                    id: PaneId::new(2),
                    cwd: PathBuf::new(),
                    remote_cwd: None,
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

#[cfg(test)]
mod group_tests {
    use super::*;
    use crate::Limits;

    #[test]
    fn mixed_sidebar_moves_preserve_sessions_and_drive_navigation_and_restoration() {
        let mut controller = Controller::new(Model::default());
        for name in ["first", "second"] {
            controller
                .dispatch(Command::AddWorkspace {
                    cwd: "/fake".into(),
                    name: name.into(),
                    remote: None,
                    group: None,
                })
                .unwrap();
        }
        controller
            .dispatch(Command::AddWorkspaceGroup {
                name: "Projects".into(),
            })
            .unwrap();
        let group = controller.model().groups()[0].id();
        for name in ["client", "server"] {
            controller
                .dispatch(Command::AddWorkspace {
                    cwd: "/fake".into(),
                    name: name.into(),
                    remote: None,
                    group: Some(group),
                })
                .unwrap();
        }
        controller
            .dispatch(Command::SetWorkspaceGroupCollapsed {
                group,
                collapsed: true,
            })
            .unwrap();
        let before = controller.model().workspaces().to_owned();
        let active = controller.model().active_workspace();
        for (index, names) in [
            (0, ["client", "server", "first", "second"]),
            (1, ["first", "client", "server", "second"]),
            (usize::MAX, ["first", "second", "client", "server"]),
        ] {
            assert!(matches!(
                controller
                    .dispatch(Command::MoveSidebarItem {
                        item: SidebarItem::Group(group),
                        index
                    })
                    .unwrap()
                    .as_slice(),
                [Effect::Persist { .. }]
            ));
            assert_eq!(
                controller
                    .model()
                    .workspaces()
                    .iter()
                    .map(Workspace::name)
                    .collect::<Vec<_>>(),
                names
            );
            for workspace in &before {
                assert_eq!(
                    controller.model().workspace(workspace.id()),
                    Some(workspace)
                );
            }
            assert_eq!(controller.model().active_workspace(), active);
            let restored = Model::restore_ordered(
                controller.model().specs(),
                controller.model().group_specs(),
                Some(controller.model().sidebar_order().to_owned()),
                active,
                true,
                Limits::default(),
            )
            .unwrap();
            assert_eq!(restored.sidebar_order(), controller.model().sidebar_order());
            assert_eq!(restored.workspaces(), controller.model().workspaces());
        }
        let before = controller.model().clone();
        let generation = controller.generation();
        assert!(
            controller
                .dispatch(Command::MoveSidebarItem {
                    item: SidebarItem::Group(group),
                    index: usize::MAX
                })
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            controller.dispatch(Command::MoveSidebarItem {
                item: SidebarItem::Workspace(before.workspaces()[2].id()),
                index: 0
            }),
            Err(Error::InvalidSidebarOrder)
        );
        assert_eq!(
            controller.dispatch(Command::MoveSidebarItem {
                item: SidebarItem::Group(WorkspaceGroupId::new(99)),
                index: 0
            }),
            Err(Error::UnknownWorkspaceGroup(WorkspaceGroupId::new(99)))
        );
        assert_eq!(controller.model(), &before);
        assert_eq!(controller.generation(), generation);
        let order = before.sidebar_order().to_owned();
        for invalid in [
            vec![],
            vec![order[0]; order.len()],
            vec![
                SidebarItem::Workspace(before.workspaces()[2].id()),
                order[1],
                order[2],
            ],
        ] {
            assert_eq!(
                Model::restore_ordered(
                    before.specs(),
                    before.group_specs(),
                    Some(invalid),
                    active,
                    true,
                    Limits::default()
                ),
                Err(Error::InvalidSidebarOrder)
            );
        }
    }

    #[test]
    fn removing_a_mixed_group_replaces_it_with_its_children_in_place() {
        let mut controller = Controller::new(Model::default());
        for name in ["first", "second"] {
            controller
                .dispatch(Command::AddWorkspace {
                    cwd: "/fake".into(),
                    name: name.into(),
                    remote: None,
                    group: None,
                })
                .unwrap();
        }
        controller
            .dispatch(Command::AddWorkspaceGroup {
                name: "Projects".into(),
            })
            .unwrap();
        let group = controller.model().groups()[0].id();
        controller
            .dispatch(Command::AddWorkspace {
                cwd: "/fake".into(),
                name: "child".into(),
                remote: None,
                group: Some(group),
            })
            .unwrap();
        controller
            .dispatch(Command::MoveSidebarItem {
                item: SidebarItem::Group(group),
                index: 1,
            })
            .unwrap();
        let pane = controller.model().active_pane();
        controller
            .dispatch(Command::RemoveWorkspaceGroup(group))
            .unwrap();
        assert_eq!(
            controller
                .model()
                .workspaces()
                .iter()
                .map(Workspace::name)
                .collect::<Vec<_>>(),
            ["first", "child", "second"]
        );
        assert_eq!(controller.model().active_pane(), pane);
        let first = controller.model().workspaces()[0].id();
        controller
            .dispatch(Command::MoveWorkspace {
                workspace: first,
                index: 2,
            })
            .unwrap();
        assert_eq!(
            controller
                .model()
                .workspaces()
                .iter()
                .map(Workspace::name)
                .collect::<Vec<_>>(),
            ["child", "second", "first"]
        );
        controller.dispatch(Command::CloseWorkspace(first)).unwrap();
        assert_eq!(controller.model().sidebar_order().len(), 2);
    }

    #[test]
    fn moving_folders_only_persists_order_and_preserves_their_workspaces() {
        let mut controller = Controller::new(Model::default());
        for name in ["Projects", "Servers", "Empty"] {
            controller
                .dispatch(Command::AddWorkspaceGroup { name: name.into() })
                .unwrap();
        }
        let ids: Vec<_> = controller
            .model()
            .groups()
            .iter()
            .map(WorkspaceGroup::id)
            .collect();
        for (name, group) in [
            ("root", None),
            ("client", Some(ids[0])),
            ("server", Some(ids[0])),
            ("remote", Some(ids[1])),
        ] {
            controller
                .dispatch(Command::AddWorkspace {
                    cwd: "/fake".into(),
                    name: name.into(),
                    remote: None,
                    group,
                })
                .unwrap();
        }
        controller
            .dispatch(Command::SetWorkspaceGroupCollapsed {
                group: ids[0],
                collapsed: true,
            })
            .unwrap();
        let workspaces = controller.model().workspaces().to_owned();
        let active = controller.model().active_workspace();
        let generation = controller.generation();
        assert_eq!(
            controller
                .dispatch(Command::MoveWorkspaceGroup {
                    group: ids[0],
                    index: 2
                })
                .unwrap(),
            [Effect::Persist {
                generation: generation + 1
            }]
        );
        assert_eq!(
            controller
                .model()
                .groups()
                .iter()
                .map(WorkspaceGroup::id)
                .collect::<Vec<_>>(),
            [ids[1], ids[2], ids[0]]
        );
        assert!(controller.model().group(ids[0]).unwrap().collapsed());
        assert_eq!(
            controller
                .model()
                .workspaces()
                .iter()
                .map(Workspace::name)
                .collect::<Vec<_>>(),
            ["root", "remote", "client", "server"]
        );
        for workspace in &workspaces {
            assert_eq!(
                controller.model().workspace(workspace.id()),
                Some(workspace)
            );
        }
        assert_eq!(controller.model().active_workspace(), active);
        controller
            .dispatch(Command::MoveWorkspaceGroup {
                group: ids[0],
                index: 0,
            })
            .unwrap();
        assert_eq!(
            controller
                .model()
                .groups()
                .iter()
                .map(WorkspaceGroup::id)
                .collect::<Vec<_>>(),
            ids
        );
        controller
            .dispatch(Command::MoveWorkspaceGroup {
                group: ids[0],
                index: usize::MAX,
            })
            .unwrap();
        let before = controller.model().clone();
        let generation = controller.generation();
        assert!(
            controller
                .dispatch(Command::MoveWorkspaceGroup {
                    group: ids[0],
                    index: 2
                })
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            controller.dispatch(Command::MoveWorkspaceGroup {
                group: WorkspaceGroupId::new(99),
                index: 0
            }),
            Err(Error::UnknownWorkspaceGroup(WorkspaceGroupId::new(99)))
        );
        assert_eq!(controller.model(), &before);
        assert_eq!(controller.generation(), generation);
        assert_eq!(
            Controller::new(Model::default()).dispatch(Command::MoveWorkspaceGroup {
                group: ids[0],
                index: 0
            }),
            Err(Error::UnknownWorkspaceGroup(ids[0]))
        );
    }

    #[test]
    fn folder_changes_preserve_terminal_identity_and_only_persist() {
        let mut controller = Controller::new(Model::default());
        controller
            .dispatch(Command::AddWorkspaceGroup {
                name: "Projects".into(),
            })
            .unwrap();
        let group = controller.model().groups()[0].id();
        controller
            .dispatch(Command::AddWorkspace {
                cwd: "/fake".into(),
                name: "app".into(),
                remote: None,
                group: Some(group),
            })
            .unwrap();
        let workspace = controller.model().active_workspace().unwrap();
        let pane = controller.model().active_pane().unwrap();
        let before = controller.model().pane(pane).unwrap().clone();
        for command in [
            Command::SetWorkspaceGroupCollapsed {
                group,
                collapsed: true,
            },
            Command::RenameWorkspaceGroup {
                group,
                name: "Work".into(),
            },
            Command::SetWorkspaceGroup {
                workspace,
                group: None,
            },
            Command::SetWorkspaceGroup {
                workspace,
                group: Some(group),
            },
            Command::RemoveWorkspaceGroup(group),
        ] {
            assert!(matches!(
                controller.dispatch(command).unwrap().as_slice(),
                [Effect::Persist { .. }]
            ));
            assert_eq!(controller.model().active_pane(), Some(pane));
            assert_eq!(controller.model().pane(pane), Some(&before));
        }
        assert!(controller.model().groups().is_empty());
        assert_eq!(
            controller.model().workspace(workspace).unwrap().group(),
            None
        );
    }

    #[test]
    fn selecting_or_creating_a_workspace_reveals_its_folder() {
        let mut controller = Controller::new(Model::default());
        controller
            .dispatch(Command::AddWorkspaceGroup {
                name: "Projects".into(),
            })
            .unwrap();
        let group = controller.model().groups()[0].id();
        controller
            .dispatch(Command::AddWorkspace {
                cwd: "/fake".into(),
                name: "app".into(),
                remote: None,
                group: Some(group),
            })
            .unwrap();
        let workspace = controller.model().active_workspace().unwrap();
        controller
            .dispatch(Command::SetWorkspaceGroupCollapsed {
                group,
                collapsed: true,
            })
            .unwrap();
        controller
            .dispatch(Command::SelectWorkspace(workspace))
            .unwrap();
        assert!(!controller.model().group(group).unwrap().collapsed());
        controller
            .dispatch(Command::SetWorkspaceGroupCollapsed {
                group,
                collapsed: true,
            })
            .unwrap();
        controller
            .dispatch(Command::AddWorkspace {
                cwd: "/fake".into(),
                name: "remote".into(),
                remote: Some("me@host".into()),
                group: Some(group),
            })
            .unwrap();
        assert!(!controller.model().group(group).unwrap().collapsed());
        assert_eq!(controller.model().workspaces()[1].group(), Some(group));
        controller
            .dispatch(Command::CloseWorkspace(workspace))
            .unwrap();
        let last = controller.model().active_workspace().unwrap();
        controller.dispatch(Command::CloseWorkspace(last)).unwrap();
        assert!(controller.model().workspaces().is_empty());
        assert_eq!(
            controller.model().groups().len(),
            1,
            "empty folders survive the last workspace closing"
        );
    }

    #[test]
    fn invalid_group_commands_are_atomic_and_empty_groups_are_bounded() {
        let mut controller = Controller::new(Model::new(crate::Limits {
            workspaces: 1,
            ..Default::default()
        }));
        controller
            .dispatch(Command::AddWorkspaceGroup {
                name: "Projects".into(),
            })
            .unwrap();
        let before = controller.model().clone();
        let generation = controller.generation();
        for command in [
            Command::AddWorkspaceGroup {
                name: "Second".into(),
            },
            Command::AddWorkspaceGroup { name: "  ".into() },
            Command::AddWorkspace {
                cwd: "/fake".into(),
                name: "app".into(),
                remote: None,
                group: Some(WorkspaceGroupId::new(99)),
            },
            Command::RemoveWorkspaceGroup(WorkspaceGroupId::new(99)),
        ] {
            assert!(controller.dispatch(command).is_err());
            assert_eq!(controller.model(), &before);
            assert_eq!(controller.generation(), generation);
        }
        controller
            .dispatch(Command::RemoveWorkspaceGroup(before.groups()[0].id()))
            .unwrap();
        controller
            .dispatch(Command::AddWorkspaceGroup {
                name: "Replacement".into(),
            })
            .unwrap();
        assert_ne!(controller.model().groups()[0].id(), before.groups()[0].id());
    }

    #[test]
    fn grouped_restore_validates_ids_membership_and_retains_collapsed_state() {
        let mut controller = Controller::new(Model::default());
        controller
            .dispatch(Command::AddWorkspaceGroup {
                name: "Projects".into(),
            })
            .unwrap();
        let group = controller.model().groups()[0].id();
        controller
            .dispatch(Command::AddWorkspace {
                cwd: "/fake".into(),
                name: "app".into(),
                remote: None,
                group: Some(group),
            })
            .unwrap();
        controller
            .dispatch(Command::SetWorkspaceGroupCollapsed {
                group,
                collapsed: true,
            })
            .unwrap();
        let specs = controller.model().specs();
        let groups = controller.model().group_specs();
        let restored = Model::restore_grouped(
            specs.clone(),
            groups.clone(),
            controller.model().active_workspace(),
            false,
            crate::Limits::default(),
        )
        .unwrap();
        assert_eq!(restored.group_specs(), groups);
        assert_eq!(restored.workspaces()[0].group(), Some(group));
        assert_eq!(
            Model::restore_grouped(
                specs.clone(),
                Vec::new(),
                None,
                true,
                crate::Limits::default()
            ),
            Err(Error::UnknownWorkspaceGroup(group))
        );
        let mut invalid = groups.clone();
        invalid.push(groups[0].clone());
        assert_eq!(
            Model::restore_grouped(specs.clone(), invalid, None, true, crate::Limits::default()),
            Err(Error::InvalidIdentity)
        );
        let mut invalid = groups;
        invalid[0].id = WorkspaceGroupId::new(0);
        assert_eq!(
            Model::restore_grouped(specs, invalid, None, true, crate::Limits::default()),
            Err(Error::InvalidIdentity)
        );
    }
}

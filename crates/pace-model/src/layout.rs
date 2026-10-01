use crate::{Error, PaneId, SplitId};
use std::collections::HashSet;

/// Vertical divides left/right; horizontal divides top/bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Vertical,
    Horizontal,
}

/// A read-only view of a workspace's validated split tree. Mutable construction
/// is accepted only through `WorkspaceSpec` and validated before model adoption.
#[derive(Debug, Clone, PartialEq)]
pub enum Layout {
    Leaf(PaneId),
    Split {
        id: SplitId,
        axis: Axis,
        ratio: f32,
        first: Box<Layout>,
        second: Box<Layout>,
    },
}

impl Layout {
    pub fn leaves(&self) -> Vec<PaneId> {
        let mut leaves = Vec::new();
        self.visit(&mut |pane| leaves.push(pane));
        leaves
    }

    pub fn contains(&self, pane: PaneId) -> bool {
        match self {
            Self::Leaf(id) => *id == pane,
            Self::Split { first, second, .. } => first.contains(pane) || second.contains(pane),
        }
    }

    pub(crate) fn validate(
        &self,
        panes: &HashSet<PaneId>,
        splits: &mut HashSet<SplitId>,
    ) -> Result<(), Error> {
        let mut seen = HashSet::new();
        self.validate_node(panes, &mut seen, splits, 0)?;
        if seen != *panes {
            return Err(Error::InvalidLayout(
                "layout must contain every pane exactly once",
            ));
        }
        Ok(())
    }

    fn validate_node(
        &self,
        panes: &HashSet<PaneId>,
        seen: &mut HashSet<PaneId>,
        splits: &mut HashSet<SplitId>,
        depth: usize,
    ) -> Result<(), Error> {
        if depth > 64 {
            return Err(Error::InvalidLayout("layout is too deeply nested"));
        }
        match self {
            Self::Leaf(id) => {
                if !panes.contains(id) || !seen.insert(*id) {
                    return Err(Error::InvalidLayout("layout leaf is missing or duplicated"));
                }
            }
            Self::Split {
                id,
                ratio,
                first,
                second,
                ..
            } => {
                if id.get() == 0 || !splits.insert(*id) {
                    return Err(Error::InvalidLayout("split identity is zero or duplicated"));
                }
                if !ratio.is_finite() || !(0.1..=0.9).contains(ratio) {
                    return Err(Error::InvalidRatio);
                }
                first.validate_node(panes, seen, splits, depth + 1)?;
                second.validate_node(panes, seen, splits, depth + 1)?;
            }
        }
        Ok(())
    }

    pub(crate) fn visit(&self, visit: &mut impl FnMut(PaneId)) {
        match self {
            Self::Leaf(pane) => visit(*pane),
            Self::Split { first, second, .. } => {
                first.visit(visit);
                second.visit(visit);
            }
        }
    }

    pub(crate) fn max_split_id(&self) -> u64 {
        match self {
            Self::Leaf(_) => 0,
            Self::Split {
                id, first, second, ..
            } => id
                .get()
                .max(first.max_split_id())
                .max(second.max_split_id()),
        }
    }

    pub(crate) fn split(&mut self, target: PaneId, pane: PaneId, id: SplitId, axis: Axis) -> bool {
        match self {
            Self::Leaf(existing) if *existing == target => {
                *self = Self::Split {
                    id,
                    axis,
                    ratio: 0.5,
                    first: Box::new(Self::Leaf(target)),
                    second: Box::new(Self::Leaf(pane)),
                };
                true
            }
            Self::Split { first, second, .. } => {
                first.split(target, pane, id, axis) || second.split(target, pane, id, axis)
            }
            _ => false,
        }
    }

    pub(crate) fn remove(self, pane: PaneId) -> Option<Self> {
        match self {
            Self::Leaf(id) => (id != pane).then_some(Self::Leaf(id)),
            Self::Split {
                id,
                axis,
                ratio,
                first,
                second,
            } => match (first.remove(pane), second.remove(pane)) {
                (Some(first), Some(second)) => Some(Self::Split {
                    id,
                    axis,
                    ratio,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (Some(remaining), None) | (None, Some(remaining)) => Some(remaining),
                (None, None) => None,
            },
        }
    }

    pub(crate) fn set_ratio(&mut self, target: SplitId, value: f32) -> Option<bool> {
        match self {
            Self::Split {
                id,
                ratio,
                first,
                second,
                ..
            } => {
                if *id == target {
                    let changed = *ratio != value;
                    *ratio = value;
                    Some(changed)
                } else {
                    first
                        .set_ratio(target, value)
                        .or_else(|| second.set_ratio(target, value))
                }
            }
            Self::Leaf(_) => None,
        }
    }
}

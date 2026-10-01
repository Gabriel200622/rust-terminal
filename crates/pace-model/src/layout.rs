use crate::{Error, PaneId, SplitId};
use std::collections::HashSet;

/// Vertical divides left/right; horizontal divides top/bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Vertical,
    Horizontal,
}

/// The side of a pane that another pane is placed against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

impl Edge {
    /// The split that puts a pane on this side of another.
    pub fn axis(self) -> Axis {
        match self {
            Self::Left | Self::Right => Axis::Vertical,
            Self::Top | Self::Bottom => Axis::Horizontal,
        }
    }

    /// The placed pane comes first in its split.
    fn leading(self) -> bool {
        matches!(self, Self::Left | Self::Top)
    }
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

    /// Splits `target`, placing `pane` against the given edge of it.
    pub(crate) fn split(&mut self, target: PaneId, pane: PaneId, id: SplitId, edge: Edge) -> bool {
        match self {
            Self::Leaf(existing) if *existing == target => {
                let (first, second) = if edge.leading() {
                    (pane, target)
                } else {
                    (target, pane)
                };
                *self = Self::Split {
                    id,
                    axis: edge.axis(),
                    ratio: 0.5,
                    first: Box::new(Self::Leaf(first)),
                    second: Box::new(Self::Leaf(second)),
                };
                true
            }
            Self::Split { first, second, .. } => {
                first.split(target, pane, id, edge) || second.split(target, pane, id, edge)
            }
            _ => false,
        }
    }

    /// Where to put a pane that nobody positioned by hand: against the roomiest
    /// pane, dividing its longer side. Sizes are fractions of the workspace,
    /// which is taken to be wider than tall; among equals `preferred` wins.
    pub(crate) fn roomiest(&self, preferred: PaneId) -> (PaneId, Edge) {
        const ASPECT: f32 = 1.6;
        let mut best = (preferred, 0.0, 0.0);
        self.visit_sizes(ASPECT, 1.0, &mut |pane, width, height| {
            let (_, best_width, best_height) = best;
            let gain = width * height - best_width * best_height;
            if gain > 1e-4 || (gain > -1e-4 && pane == preferred) {
                best = (pane, width, height);
            }
        });
        let (pane, width, height) = best;
        let edge = if width >= height {
            Edge::Right
        } else {
            Edge::Bottom
        };
        (pane, edge)
    }

    fn visit_sizes(&self, width: f32, height: f32, visit: &mut impl FnMut(PaneId, f32, f32)) {
        match self {
            Self::Leaf(pane) => visit(*pane, width, height),
            Self::Split {
                axis,
                ratio,
                first,
                second,
                ..
            } => {
                let (a, b) = match axis {
                    Axis::Vertical => ((width * ratio, height), (width * (1.0 - ratio), height)),
                    Axis::Horizontal => ((width, height * ratio), (width, height * (1.0 - ratio))),
                };
                first.visit_sizes(a.0, a.1, visit);
                second.visit_sizes(b.0, b.1, visit);
            }
        }
    }

    /// Exchanges the positions of two panes.
    pub(crate) fn swap(&mut self, a: PaneId, b: PaneId) {
        match self {
            Self::Leaf(pane) if *pane == a => *pane = b,
            Self::Leaf(pane) if *pane == b => *pane = a,
            Self::Leaf(_) => {}
            Self::Split { first, second, .. } => {
                first.swap(a, b);
                second.swap(a, b);
            }
        }
    }

    /// The same panes in the same places, whatever the split identities and
    /// ratios are.
    pub(crate) fn same_arrangement(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Leaf(a), Self::Leaf(b)) => a == b,
            (
                Self::Split {
                    axis: a,
                    first: a_first,
                    second: a_second,
                    ..
                },
                Self::Split {
                    axis: b,
                    first: b_first,
                    second: b_second,
                    ..
                },
            ) => a == b && a_first.same_arrangement(b_first) && a_second.same_arrangement(b_second),
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

use crate::{Error, PaneId, SplitId};
use std::collections::HashSet;

/// Vertical divides left/right; horizontal divides top/bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Vertical,
    Horizontal,
}

/// A spatial direction within a workspace's split layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDirection {
    Left,
    Right,
    Up,
    Down,
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

    /// Finds a pane sharing the requested edge, without wrapping. When several
    /// panes share that edge, prefer the closest perpendicular centre; ties use
    /// layout order. Split ratios determine positions independently of pixels.
    pub fn adjacent(&self, pane: PaneId, direction: FocusDirection) -> Option<PaneId> {
        let mut regions = Vec::new();
        self.visit_regions(
            Bounds {
                left: 0.0,
                top: 0.0,
                right: 1.0,
                bottom: 1.0,
            },
            &mut regions,
        );
        let (_, origin) = regions.iter().find(|(id, _)| *id == pane)?;
        let mut nearest = None;
        let mut distance = f64::INFINITY;
        for (id, bounds) in &regions {
            if *id == pane {
                continue;
            }
            let (touches, start, end, origin_start, origin_end) = match direction {
                FocusDirection::Left => (
                    bounds.right == origin.left,
                    bounds.top,
                    bounds.bottom,
                    origin.top,
                    origin.bottom,
                ),
                FocusDirection::Right => (
                    bounds.left == origin.right,
                    bounds.top,
                    bounds.bottom,
                    origin.top,
                    origin.bottom,
                ),
                FocusDirection::Up => (
                    bounds.bottom == origin.top,
                    bounds.left,
                    bounds.right,
                    origin.left,
                    origin.right,
                ),
                FocusDirection::Down => (
                    bounds.top == origin.bottom,
                    bounds.left,
                    bounds.right,
                    origin.left,
                    origin.right,
                ),
            };
            if touches && start < origin_end && end > origin_start {
                let offset = ((start + end) - (origin_start + origin_end)).abs();
                if offset < distance {
                    nearest = Some(*id);
                    distance = offset;
                }
            }
        }
        nearest
    }

    fn visit_regions(&self, bounds: Bounds, regions: &mut Vec<(PaneId, Bounds)>) {
        match self {
            Self::Leaf(pane) => regions.push((*pane, bounds)),
            Self::Split {
                axis,
                ratio,
                first,
                second,
                ..
            } => {
                let mut a = bounds;
                let mut b = bounds;
                match axis {
                    Axis::Vertical => {
                        let cut = bounds.left + (bounds.right - bounds.left) * f64::from(*ratio);
                        a.right = cut;
                        b.left = cut;
                    }
                    Axis::Horizontal => {
                        let cut = bounds.top + (bounds.bottom - bounds.top) * f64::from(*ratio);
                        a.bottom = cut;
                        b.top = cut;
                    }
                }
                first.visit_regions(a, regions);
                second.visit_regions(b, regions);
            }
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

#[derive(Clone, Copy)]
struct Bounds {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(id: u64) -> Layout {
        Layout::Leaf(PaneId::new(id))
    }

    fn split(axis: Axis, ratio: f32, first: Layout, second: Layout) -> Layout {
        Layout::Split {
            id: SplitId::new(1),
            axis,
            ratio,
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    #[test]
    fn adjacent_follows_all_four_directions_in_a_grid() {
        let layout = split(
            Axis::Vertical,
            0.5,
            split(Axis::Horizontal, 0.5, leaf(1), leaf(2)),
            split(Axis::Horizontal, 0.5, leaf(3), leaf(4)),
        );
        for (from, direction, to) in [
            (1, FocusDirection::Right, 3),
            (3, FocusDirection::Left, 1),
            (1, FocusDirection::Down, 2),
            (2, FocusDirection::Up, 1),
            (2, FocusDirection::Right, 4),
            (4, FocusDirection::Up, 3),
        ] {
            assert_eq!(
                layout.adjacent(PaneId::new(from), direction),
                Some(PaneId::new(to))
            );
        }
    }

    #[test]
    fn adjacent_does_not_wrap_or_choose_diagonal_panes() {
        let layout = split(
            Axis::Vertical,
            0.5,
            split(Axis::Horizontal, 0.5, leaf(1), leaf(2)),
            leaf(3),
        );
        assert_eq!(layout.adjacent(PaneId::new(1), FocusDirection::Up), None);
        assert_eq!(layout.adjacent(PaneId::new(2), FocusDirection::Down), None);
        assert_eq!(layout.adjacent(PaneId::new(3), FocusDirection::Down), None);
        assert_eq!(layout.adjacent(PaneId::new(99), FocusDirection::Left), None);
        assert_eq!(
            leaf(1).adjacent(PaneId::new(1), FocusDirection::Right),
            None
        );
    }

    #[test]
    fn adjacent_uses_ratios_and_layout_order_for_shared_edges() {
        let layout = split(
            Axis::Vertical,
            0.5,
            leaf(1),
            split(Axis::Horizontal, 0.3, leaf(2), leaf(3)),
        );
        assert_eq!(
            layout.adjacent(PaneId::new(1), FocusDirection::Right),
            Some(PaneId::new(3))
        );
        let even = split(
            Axis::Vertical,
            0.5,
            leaf(1),
            split(Axis::Horizontal, 0.5, leaf(2), leaf(3)),
        );
        assert_eq!(
            even.adjacent(PaneId::new(1), FocusDirection::Right),
            Some(PaneId::new(2))
        );
    }

    #[test]
    fn adjacent_skips_nearer_diagonals_and_panes_beyond_a_neighbour() {
        let layout = split(
            Axis::Vertical,
            0.4,
            split(Axis::Horizontal, 0.2, leaf(1), leaf(2)),
            split(
                Axis::Horizontal,
                0.2,
                split(Axis::Vertical, 0.1, leaf(3), leaf(4)),
                leaf(5),
            ),
        );
        assert_eq!(
            layout.adjacent(PaneId::new(1), FocusDirection::Right),
            Some(PaneId::new(3))
        );
    }
}

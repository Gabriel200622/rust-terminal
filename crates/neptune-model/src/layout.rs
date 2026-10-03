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
    /// Terminals sharing one place as tabs. Only `shown` is in view.
    Tabs { panes: Vec<PaneId>, shown: PaneId },
    Split {
        id: SplitId,
        axis: Axis,
        ratio: f32,
        first: Box<Layout>,
        second: Box<Layout>,
    },
}

impl Layout {
    /// One terminal in a place of its own.
    pub fn pane(pane: PaneId) -> Self {
        Self::Tabs {
            panes: vec![pane],
            shown: pane,
        }
    }

    /// Every pane, including tabs that are not in view.
    pub fn panes(&self) -> Vec<PaneId> {
        let mut panes = Vec::new();
        self.visit(&mut |tabs, _| panes.extend(tabs));
        panes
    }

    /// The pane in view in each tab group, in layout order.
    pub fn shown(&self) -> Vec<PaneId> {
        let mut shown = Vec::new();
        self.visit(&mut |_, pane| shown.push(pane));
        shown
    }

    /// The tab group holding `pane`, and which of its tabs is in view.
    pub fn tabs(&self, pane: PaneId) -> Option<(&[PaneId], PaneId)> {
        match self {
            Self::Tabs { panes, shown } => panes.contains(&pane).then_some((panes, *shown)),
            Self::Split { first, second, .. } => first.tabs(pane).or_else(|| second.tabs(pane)),
        }
    }

    pub fn contains(&self, pane: PaneId) -> bool {
        self.tabs(pane).is_some()
    }

    /// The tab after or before `pane` in its group, wrapping around. A pane
    /// alone in its place has none.
    pub fn next_tab(&self, pane: PaneId, forward: bool) -> Option<PaneId> {
        let (tabs, _) = self.tabs(pane)?;
        let position = tabs.iter().position(|id| *id == pane)?;
        let step = if forward { 1 } else { tabs.len() - 1 };
        Some(tabs[(position + step) % tabs.len()]).filter(|next| *next != pane)
    }

    /// Finds the pane in view across the requested edge of `pane`'s tab group,
    /// without wrapping. When several groups share that edge, prefer the
    /// closest perpendicular centre; ties use layout order. Split ratios
    /// determine positions independently of pixels.
    pub fn adjacent(&self, pane: PaneId, direction: FocusDirection) -> Option<PaneId> {
        let pane = self.tabs(pane)?.1;
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
            Self::Tabs { shown, .. } => regions.push((*shown, bounds)),
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
            Self::Tabs { panes: tabs, shown } => {
                if !tabs.contains(shown) {
                    return Err(Error::InvalidLayout("tab in view is not in its group"));
                }
                for id in tabs {
                    if !panes.contains(id) || !seen.insert(*id) {
                        return Err(Error::InvalidLayout("layout leaf is missing or duplicated"));
                    }
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

    fn visit(&self, visit: &mut impl FnMut(&[PaneId], PaneId)) {
        match self {
            Self::Tabs { panes, shown } => visit(panes, *shown),
            Self::Split { first, second, .. } => {
                first.visit(visit);
                second.visit(visit);
            }
        }
    }

    pub(crate) fn max_split_id(&self) -> u64 {
        match self {
            Self::Tabs { .. } => 0,
            Self::Split {
                id, first, second, ..
            } => id
                .get()
                .max(first.max_split_id())
                .max(second.max_split_id()),
        }
    }

    /// Splits `target`'s tab group, placing `pane` against the given edge of it.
    pub(crate) fn split(&mut self, target: PaneId, pane: PaneId, id: SplitId, edge: Edge) -> bool {
        match self {
            Self::Tabs { panes, .. } if panes.contains(&target) => {
                let existing = std::mem::replace(self, Self::pane(pane));
                let (first, second) = if edge.leading() {
                    (Self::pane(pane), existing)
                } else {
                    (existing, Self::pane(pane))
                };
                *self = Self::Split {
                    id,
                    axis: edge.axis(),
                    ratio: 0.5,
                    first: Box::new(first),
                    second: Box::new(second),
                };
                true
            }
            Self::Split { first, second, .. } => {
                first.split(target, pane, id, edge) || second.split(target, pane, id, edge)
            }
            Self::Tabs { .. } => false,
        }
    }

    /// Adds `pane` to `target`'s tab group and brings it into view. Without an
    /// index it follows `target`; an index past the end means last.
    pub(crate) fn add_tab(&mut self, target: PaneId, pane: PaneId, index: Option<usize>) -> bool {
        match self {
            Self::Tabs { panes, shown } => {
                let Some(position) = panes.iter().position(|id| *id == target) else {
                    return false;
                };
                let index = index.unwrap_or(position + 1).min(panes.len());
                panes.insert(index, pane);
                *shown = pane;
                true
            }
            Self::Split { first, second, .. } => {
                first.add_tab(target, pane, index) || second.add_tab(target, pane, index)
            }
        }
    }

    /// Brings `pane` into view in its tab group.
    pub(crate) fn show(&mut self, pane: PaneId) {
        match self {
            Self::Tabs { panes, shown } => {
                if panes.contains(&pane) {
                    *shown = pane;
                }
            }
            Self::Split { first, second, .. } => {
                first.show(pane);
                second.show(pane);
            }
        }
    }

    /// The same panes in the same places, whatever the split identities and
    /// ratios and whichever tabs are in view.
    pub(crate) fn same_arrangement(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Tabs { panes: a, .. }, Self::Tabs { panes: b, .. }) => a == b,
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

    /// A tab group that loses the tab in view shows the one that followed it,
    /// or the new last tab. A group that loses its only tab gives up its place.
    pub(crate) fn remove(self, pane: PaneId) -> Option<Self> {
        match self {
            Self::Tabs { mut panes, shown } => {
                let Some(position) = panes.iter().position(|id| *id == pane) else {
                    return Some(Self::Tabs { panes, shown });
                };
                panes.remove(position);
                let shown = if shown == pane {
                    *panes.get(position).or(panes.last())?
                } else {
                    shown
                };
                Some(Self::Tabs { panes, shown })
            }
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
            Self::Tabs { .. } => None,
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
        Layout::pane(PaneId::new(id))
    }

    fn tabs(ids: &[u64], shown: u64) -> Layout {
        Layout::Tabs {
            panes: ids.iter().copied().map(PaneId::new).collect(),
            shown: PaneId::new(shown),
        }
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

    #[test]
    fn adjacent_moves_between_tab_groups_and_lands_on_the_tab_in_view() {
        let layout = split(Axis::Vertical, 0.5, tabs(&[1, 2], 2), tabs(&[3, 4, 5], 4));
        for from in [1, 2] {
            assert_eq!(
                layout.adjacent(PaneId::new(from), FocusDirection::Right),
                Some(PaneId::new(4))
            );
        }
        assert_eq!(
            layout.adjacent(PaneId::new(5), FocusDirection::Left),
            Some(PaneId::new(2))
        );
        assert_eq!(layout.adjacent(PaneId::new(1), FocusDirection::Left), None);
        // Tabs are stepped through in order, around the ends of their group.
        for (from, forward, to) in [(3, true, 4), (5, true, 3), (3, false, 5)] {
            assert_eq!(
                layout.next_tab(PaneId::new(from), forward),
                Some(PaneId::new(to))
            );
        }
        assert_eq!(leaf(1).next_tab(PaneId::new(1), true), None);
        assert_eq!(layout.shown(), [PaneId::new(2), PaneId::new(4)]);
        assert_eq!(layout.panes(), [1, 2, 3, 4, 5].map(PaneId::new));
    }

    #[test]
    fn removing_the_tab_in_view_shows_its_follower_and_empties_collapse() {
        let pane = PaneId::new;
        let group = |layout: &Layout, id| {
            layout
                .tabs(pane(id))
                .map(|(panes, shown)| (panes.to_vec(), shown))
        };
        let layout = split(Axis::Vertical, 0.5, tabs(&[1, 2, 3], 2), leaf(4));
        let layout = layout.remove(pane(2)).unwrap();
        assert_eq!(group(&layout, 1), Some((vec![pane(1), pane(3)], pane(3))));
        let layout = layout.remove(pane(3)).unwrap();
        assert_eq!(group(&layout, 1), Some((vec![pane(1)], pane(1))));
        // A hidden tab leaves without changing what is in view.
        let hidden = tabs(&[5, 6], 6).remove(pane(5)).unwrap();
        assert_eq!(hidden, leaf(6));
        assert_eq!(layout.remove(pane(1)), Some(leaf(4)));
        assert_eq!(leaf(4).remove(pane(4)), None);
    }

    #[test]
    fn tabs_join_after_their_target_or_at_an_index_and_come_into_view() {
        let pane = PaneId::new;
        let mut layout = split(Axis::Vertical, 0.5, tabs(&[1, 2], 1), leaf(3));
        assert!(layout.add_tab(pane(1), pane(4), None));
        assert!(layout.add_tab(pane(2), pane(5), Some(0)));
        assert!(layout.add_tab(pane(2), pane(6), Some(usize::MAX)));
        assert!(!layout.add_tab(pane(99), pane(7), None));
        assert_eq!(
            layout.tabs(pane(1)),
            Some((&[5, 1, 4, 2, 6].map(pane)[..], pane(6)))
        );
        layout.show(pane(4));
        layout.show(pane(99));
        assert_eq!(layout.shown(), [pane(4), pane(3)]);
        // Splitting a tab divides its whole group.
        assert!(layout.split(pane(1), pane(8), SplitId::new(2), Edge::Left));
        assert_eq!(layout.shown(), [pane(8), pane(4), pane(3)]);
    }
}

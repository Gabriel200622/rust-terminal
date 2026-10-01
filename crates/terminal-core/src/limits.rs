//! Allocation guard: logical cells account for history plus both visible grids.
use super::*;

/// Logical cell budget per session, including history and both visible grids.
/// This prevents absurd dimension/history combinations before allocation. Cell
/// extras, parser buffers, and allocator capacity are additional resident memory.
pub const MAX_GRID_CELLS: usize = 16_000_000;

pub(super) fn check_grid_budget(
    cols: u16,
    rows: u16,
    history: usize,
) -> std::result::Result<(), SessionError> {
    let cells = (history + 2 * usize::from(rows)).checked_mul(usize::from(cols));
    if cells.is_none_or(|cells| cells > MAX_GRID_CELLS) {
        return Err(SessionError::new(
            SessionErrorKind::InvalidGeometry,
            format!(
                "Terminal dimensions and scrollback exceed the {MAX_GRID_CELLS} cell budget; reduce scrollback or terminal dimensions"
            ),
        ));
    }
    Ok(())
}

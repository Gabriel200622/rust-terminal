//! Cancellable search copies one row at a time; regex work never holds the VT mutex.
use crate::{
    Direction, Flags, Point, SelectionRange, SessionError, SessionErrorKind, TerminalSession,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

/// Compiled once when a query changes and shared by successive requests.
#[derive(Debug)]
pub struct SearchQuery {
    regex: regex::Regex,
}
impl SearchQuery {
    pub fn compile(query: &str) -> Result<Arc<Self>, SessionError> {
        if query.is_empty() || query.len() > 8192 {
            return Err(SessionError::new(
                SessionErrorKind::InvalidSearch,
                "Search query must contain 1 to 8192 bytes",
            ));
        }
        let regex = regex::RegexBuilder::new(query)
            .size_limit(2 * 1024 * 1024)
            .build()
            .map_err(|error| {
                SessionError::new(SessionErrorKind::InvalidSearch, error.to_string())
            })?;
        Ok(Arc::new(Self { regex }))
    }
}
#[derive(Debug, Clone, Copy)]
pub struct SearchBudget {
    pub max_rows: usize,
    pub max_duration: Duration,
}
impl Default for SearchBudget {
    fn default() -> Self {
        Self {
            max_rows: 32,
            max_duration: Duration::from_millis(2),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchProgress {
    Pending,
    Found(SelectionRange),
    NotFound,
    Cancelled,
    Stale,
    LimitExceeded,
}

/// A request carries its origin and revision; replacing a request drops its
/// bounded state. Wrapped logical lines retain at most one MiB of searchable
/// text. Results never refer to a grid revision newer than the captured one.
pub struct SearchTask {
    query: Arc<SearchQuery>,
    revision: u64,
    origin: Point,
    direction: Direction,
    next_line: i32,
    top: i32,
    bottom: i32,
    visited: usize,
    text: String,
    positions: Vec<(usize, Point)>,
    cancelled: Arc<AtomicBool>,
    done: Option<SearchProgress>,
    initial_context: bool,
    resume_line: i32,
    origin_complete: bool,
    fallback: Option<SelectionRange>,
}
impl SearchTask {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
}
impl TerminalSession {
    pub fn begin_search(
        &self,
        query: Arc<SearchQuery>,
        origin: Point,
        direction: Direction,
    ) -> SearchTask {
        let (revision, top, bottom, columns) = self.search_bounds();
        let origin = Point::new(
            origin.line.clamp(top, bottom),
            origin.column.min(columns - 1),
        );
        SearchTask {
            query,
            revision,
            origin,
            direction,
            next_line: origin.line,
            top,
            bottom,
            visited: 0,
            text: String::new(),
            positions: Vec::new(),
            cancelled: Arc::new(AtomicBool::new(false)),
            done: None,
            initial_context: true,
            resume_line: origin.line,
            origin_complete: false,
            fallback: None,
        }
    }
    pub fn search_step(&self, task: &mut SearchTask, budget: SearchBudget) -> SearchProgress {
        if task.cancelled.load(Ordering::Acquire) {
            return SearchProgress::Cancelled;
        }
        if self.revision() != task.revision {
            return SearchProgress::Stale;
        }
        if let Some(done) = task.done {
            return done;
        }
        let started = Instant::now();
        for _ in 0..budget.max_rows.min(256) {
            if started.elapsed() >= budget.max_duration || task.cancelled.load(Ordering::Acquire) {
                break;
            }
            let Some((cells, wraps, previous_wraps)) =
                self.search_row(task.next_line, task.revision)
            else {
                return SearchProgress::Stale;
            };
            let row = task.next_line;
            let mut text = String::new();
            let mut positions = Vec::new();
            for cell in cells {
                if cell
                    .flags
                    .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
                {
                    continue;
                }
                positions.push((text.len(), Point::new(row, cell.column)));
                text.push(cell.c);
                for ch in cell.extra {
                    positions.push((text.len(), Point::new(row, cell.column)));
                    text.push(ch);
                }
            }
            if !wraps {
                while text.ends_with(' ') {
                    text.pop();
                }
                positions.retain(|(offset, _)| *offset < text.len());
            }
            if task.visited == 0 {
                task.origin_complete = match task.direction {
                    Direction::Right => !wraps,
                    Direction::Left => !previous_wraps,
                };
            }
            let accumulation = if task.initial_context {
                match task.direction {
                    Direction::Right => Direction::Left,
                    Direction::Left => Direction::Right,
                }
            } else {
                task.direction
            };
            match accumulation {
                Direction::Right => {
                    let base = task.text.len();
                    task.positions.extend(
                        positions
                            .into_iter()
                            .map(|(byte, point)| (byte + base, point)),
                    );
                    task.text.push_str(&text);
                }
                Direction::Left => {
                    let base = text.len();
                    task.positions
                        .iter_mut()
                        .for_each(|(byte, _)| *byte += base);
                    positions.append(&mut task.positions);
                    task.positions = positions;
                    text.push_str(&task.text);
                    task.text = text;
                }
            }
            // A logical line may cross many rows, but neither allocation nor regex
            // work may grow without bound during a single request.
            if task.text.len() > 1024 * 1024 {
                task.done = Some(SearchProgress::LimitExceeded);
                return SearchProgress::LimitExceeded;
            }
            let context_complete = match task.direction {
                Direction::Right => !previous_wraps,
                Direction::Left => !wraps,
            };
            let complete = if task.initial_context {
                context_complete && task.origin_complete
            } else {
                match task.direction {
                    Direction::Right => !wraps,
                    Direction::Left => !previous_wraps,
                }
            };
            if complete {
                let initial = task
                    .positions
                    .iter()
                    .any(|(_, point)| point.line == task.origin.line);
                let origin_index = match task.direction {
                    Direction::Right => task
                        .positions
                        .partition_point(|(_, point)| *point < task.origin),
                    Direction::Left => task
                        .positions
                        .partition_point(|(_, point)| *point <= task.origin),
                };
                let origin_byte = task
                    .positions
                    .get(origin_index)
                    .map(|(byte, _)| *byte)
                    .unwrap_or(task.text.len());
                let mut selected = None;
                let mut fallback = None;
                // Select byte offsets first, then map only the chosen match to
                // grid points. Repeated matches must not rescan all positions.
                for found in task.query.regex.find_iter(&task.text) {
                    if found.is_empty() {
                        continue;
                    }
                    let eligible = !initial
                        || match task.direction {
                            Direction::Right => found.start() >= origin_byte,
                            Direction::Left => found.start() < origin_byte,
                        };
                    let bytes = (found.start(), found.end());
                    if eligible {
                        selected = Some(bytes);
                        if task.direction == Direction::Right {
                            break;
                        }
                    } else if fallback.is_none() || task.direction == Direction::Left {
                        fallback = Some(bytes);
                    }
                }
                let range = |start, end| {
                    let start_index = task
                        .positions
                        .partition_point(|(offset, _)| *offset <= start);
                    let end_index = task.positions.partition_point(|(offset, _)| *offset < end);
                    (start_index > 0 && end_index > 0).then(|| SelectionRange {
                        start: task.positions[start_index - 1].1,
                        end: task.positions[end_index - 1].1,
                        is_block: false,
                    })
                };
                let selected = selected.and_then(|(start, end)| range(start, end));
                let fallback = fallback.and_then(|(start, end)| range(start, end));
                if let Some(range) = selected {
                    if self.revision() != task.revision {
                        return SearchProgress::Stale;
                    }
                    task.done = Some(SearchProgress::Found(range));
                    return SearchProgress::Found(range);
                }
                if task.fallback.is_none() {
                    task.fallback = fallback;
                }
                task.text.clear();
                task.positions.clear();
            }
            task.visited += 1;
            if task.visited >= (task.bottom - task.top + 1) as usize {
                let progress = task
                    .fallback
                    .map(SearchProgress::Found)
                    .unwrap_or(SearchProgress::NotFound);
                task.done = Some(progress);
                return progress;
            }
            if task.initial_context && context_complete {
                task.initial_context = false;
                task.next_line = match task.direction {
                    Direction::Right => {
                        if task.resume_line == task.bottom {
                            task.top
                        } else {
                            task.resume_line + 1
                        }
                    }
                    Direction::Left => {
                        if task.resume_line == task.top {
                            task.bottom
                        } else {
                            task.resume_line - 1
                        }
                    }
                };
                continue;
            }
            task.next_line = match accumulation {
                Direction::Right => {
                    if row == task.bottom {
                        task.top
                    } else {
                        row + 1
                    }
                }
                Direction::Left => {
                    if row == task.top {
                        task.bottom
                    } else {
                        row - 1
                    }
                }
            };
            if (accumulation == Direction::Right && row == task.bottom)
                || (accumulation == Direction::Left && row == task.top)
            {
                task.text.clear();
                task.positions.clear();
            }
        }
        if task.cancelled.load(Ordering::Acquire) {
            SearchProgress::Cancelled
        } else {
            SearchProgress::Pending
        }
    }
}

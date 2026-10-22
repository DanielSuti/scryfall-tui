//! Searching the text of the focused panel with `/`, `n` and `N`.

use crate::tui::widgets::item_list::{ItemList, Motion, MotionOutcome};

/// A panel whose text lines `/` can search and whose cursor or scroll position can jump to a
/// line.
pub trait Searchable {
    /// Returns the lines of text in display order.
    fn search_lines(&self) -> &[String];
    /// Returns the index of the current line: the cursor row, or the top visible line.
    fn current_line(&self) -> usize;
    /// Moves to the line at `index` and scrolls it into a viewport of `viewport` rows.
    fn jump_to(&mut self, index: usize, viewport: usize);
}

/// A panel that responds to cursor motions.
pub trait Navigable {
    /// Applies `motion` with a viewport of `viewport` rows.
    fn apply_motion(&mut self, motion: Motion, viewport: usize) -> MotionOutcome;
}

/// A focusable panel with searchable text and motions.
pub trait Pane: Searchable + Navigable {}

impl<T: Searchable + Navigable> Pane for T {}

impl Searchable for ItemList {
    fn search_lines(&self) -> &[String] {
        self.rows()
    }

    fn current_line(&self) -> usize {
        self.cursor()
    }

    fn jump_to(&mut self, index: usize, viewport: usize) {
        self.select(index, viewport);
    }
}

impl Navigable for ItemList {
    fn apply_motion(&mut self, motion: Motion, viewport: usize) -> MotionOutcome {
        self.apply(motion, viewport)
    }
}

/// The direction of a search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Towards the end, as `/` and `n` search.
    Forward,
    /// Towards the start, as `N` searches.
    Backward,
}

/// A line found by [`find`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Found {
    /// The index of the matching line.
    pub index: usize,
    /// True when the search passed the end (or start) and continued from the other side.
    pub wrapped: bool,
}

/// Finds the next line containing `pattern`, starting after `from` in `direction` and wrapping
/// around. The line at `from` itself is checked last. The match ignores case unless `pattern`
/// contains an uppercase letter, like vim's `smartcase`. Returns `None` for an empty pattern or
/// when no line matches.
pub fn find(lines: &[String], pattern: &str, from: usize, direction: Direction) -> Option<Found> {
    if pattern.is_empty() || lines.is_empty() {
        return None;
    }
    let case_sensitive = pattern.chars().any(char::is_uppercase);
    let needle = if case_sensitive {
        pattern.to_owned()
    } else {
        pattern.to_lowercase()
    };
    let matches = |line: &str| {
        if case_sensitive {
            line.contains(&needle)
        } else {
            line.to_lowercase().contains(&needle)
        }
    };
    let len = lines.len();
    let from = from.min(len - 1);
    (1..=len).find_map(|step| {
        let index = match direction {
            Direction::Forward => (from + step) % len,
            Direction::Backward => (from + len - step % len) % len,
        };
        let wrapped = match direction {
            Direction::Forward => index <= from,
            Direction::Backward => index >= from,
        };
        matches(&lines[index]).then_some(Found { index, wrapped })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn finds_forward_from_after_the_cursor() {
        let lines = lines(&["Sol Ring", "Shivan Dragon", "Glorybringer", "Dragon Mage"]);
        assert_eq!(
            find(&lines, "dragon", 0, Direction::Forward),
            Some(Found {
                index: 1,
                wrapped: false
            })
        );
        assert_eq!(
            find(&lines, "dragon", 1, Direction::Forward),
            Some(Found {
                index: 3,
                wrapped: false
            })
        );
        assert_eq!(
            find(&lines, "dragon", 3, Direction::Forward),
            Some(Found {
                index: 1,
                wrapped: true
            })
        );
    }

    #[test]
    fn finds_backward_and_wraps_at_the_start() {
        let lines = lines(&["Dragon Mage", "Sol Ring", "Shivan Dragon"]);
        assert_eq!(
            find(&lines, "dragon", 2, Direction::Backward),
            Some(Found {
                index: 0,
                wrapped: false
            })
        );
        assert_eq!(
            find(&lines, "dragon", 0, Direction::Backward),
            Some(Found {
                index: 2,
                wrapped: true
            })
        );
    }

    #[test]
    fn the_current_line_is_checked_last() {
        let lines = lines(&["Sol Ring", "Dragon"]);
        assert_eq!(
            find(&lines, "dragon", 1, Direction::Forward),
            Some(Found {
                index: 1,
                wrapped: true
            })
        );
    }

    #[test]
    fn smartcase_and_misses() {
        let lines = lines(&["dragon", "Dragon"]);
        assert_eq!(
            find(&lines, "Dragon", 1, Direction::Forward).unwrap().index,
            1
        );
        assert_eq!(find(&lines, "DRAGON", 0, Direction::Forward), None);
        assert_eq!(find(&lines, "", 0, Direction::Forward), None);
        assert_eq!(find(&[], "x", 0, Direction::Forward), None);
    }
}

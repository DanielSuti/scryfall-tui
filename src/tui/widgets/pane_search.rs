//! Searching the text of the focused panel with `/`, `n` and `N`.

use std::ops::Range;

use ratatui::style::Style;
use ratatui::text::{Line, Span};

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

/// What `/` shows in the focused panel: the matches of `pattern`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneSearch<'a> {
    /// The pattern being typed, or the last one searched for.
    pub pattern: &'a str,
}

/// Returns the byte ranges of `text` where `pattern` occurs, left to right and without overlap.
/// Case is ignored unless `pattern` contains an uppercase letter, like vim's `smartcase`. An
/// empty pattern matches nowhere.
pub fn match_ranges(text: &str, pattern: &str) -> Vec<Range<usize>> {
    let pattern: Vec<char> = pattern.chars().collect();
    if pattern.is_empty() {
        return Vec::new();
    }
    let case_sensitive = pattern.iter().any(|c| c.is_uppercase());
    let same = |a: char, b: char| {
        if case_sensitive {
            a == b
        } else {
            a.to_lowercase().eq(b.to_lowercase())
        }
    };
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut ranges = Vec::new();
    let mut i = 0;
    while i + pattern.len() <= chars.len() {
        if pattern
            .iter()
            .enumerate()
            .all(|(k, p)| same(chars[i + k].1, *p))
        {
            let end = chars
                .get(i + pattern.len())
                .map_or(text.len(), |(at, _)| *at);
            ranges.push(chars[i].0..end);
            i += pattern.len();
        } else {
            i += 1;
        }
    }
    ranges
}

/// Returns `line` with every match of `pattern` in its spans drawn with `style` on top of the
/// span's own style. A match is found within one span.
pub fn highlight(line: Line<'static>, pattern: &str, style: Style) -> Line<'static> {
    let mut spans = Vec::new();
    for span in line.spans {
        let ranges = match_ranges(&span.content, pattern);
        if ranges.is_empty() {
            spans.push(span);
            continue;
        }
        let mut last = 0;
        for range in ranges {
            if range.start > last {
                spans.push(Span::styled(
                    span.content[last..range.start].to_owned(),
                    span.style,
                ));
            }
            spans.push(Span::styled(
                span.content[range.clone()].to_owned(),
                span.style.patch(style),
            ));
            last = range.end;
        }
        if last < span.content.len() {
            spans.push(Span::styled(span.content[last..].to_owned(), span.style));
        }
    }
    Line { spans, ..line }
}

/// Finds the next line containing `pattern`, as [`match_ranges`] matches, starting after `from`
/// in `direction` and wrapping around. The line at `from` itself is checked last. Returns
/// `None` for an empty pattern or when no line matches.
pub fn find(lines: &[String], pattern: &str, from: usize, direction: Direction) -> Option<Found> {
    if pattern.is_empty() || lines.is_empty() {
        return None;
    }
    let matches = |line: &str| !match_ranges(line, pattern).is_empty();
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

    use ratatui::style::{Color, Modifier};

    #[test]
    fn match_ranges_follow_smartcase() {
        assert_eq!(
            match_ranges("Dragon dragon DRAGON", "dragon"),
            [0..6, 7..13, 14..20]
        );
        assert_eq!(match_ranges("Dragon dragon", "Dragon"), vec![(0..6)]);
        assert_eq!(
            match_ranges("Lim-Dûl the Necromancer", "DÛL"),
            Vec::<Range<usize>>::new()
        );
        // `û` takes two bytes.
        assert_eq!(match_ranges("Lim-Dûl", "dûl"), vec![(4..8)]);
        assert_eq!(match_ranges("aaaa", "aa"), [0..2, 2..4]);
        assert!(match_ranges("text", "").is_empty());
    }

    #[test]
    fn highlight_styles_only_the_matches() {
        let style = Style::new().fg(Color::Yellow);
        let base = Style::new().add_modifier(Modifier::BOLD);
        let line = Line::from(vec![Span::raw("▌"), Span::styled("Shivan Dragon", base)]);
        let lit = highlight(line, "drag", style);
        let parts: Vec<(&str, Style)> = lit
            .spans
            .iter()
            .map(|s| (s.content.as_ref(), s.style))
            .collect();
        assert_eq!(
            parts,
            [
                ("▌", Style::default()),
                ("Shivan ", base),
                ("Drag", base.patch(style)),
                ("on", base),
            ]
        );
    }

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

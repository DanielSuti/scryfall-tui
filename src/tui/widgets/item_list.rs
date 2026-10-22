//! A scrollable list with a cursor, moved with vim motions.

use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

/// A cursor movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    /// One row down (`j`).
    Down,
    /// One row up (`k`).
    Up,
    /// First row (`gg`).
    Top,
    /// Last row (`G`).
    Bottom,
    /// Half a viewport down (`<C-d>`).
    HalfPageDown,
    /// Half a viewport up (`<C-u>`).
    HalfPageUp,
}

/// What a motion did beyond moving the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MotionOutcome {
    /// True when the motion asked to go past the last row: `j` on the last row, or `G`.
    pub end_reached: bool,
}

/// The rows of a list, the cursor and the scroll offset. Each row's text is what `/` searches.
#[derive(Debug, Clone, Default)]
pub struct ItemList {
    rows: Vec<String>,
    cursor: usize,
    offset: usize,
}

/// The marker drawn in front of the row under the cursor.
pub const CURSOR_MARKER: &str = "▌";

impl ItemList {
    /// Creates an empty list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the rows. The cursor stays on its index, clamped to the new length.
    pub fn set_rows(&mut self, rows: Vec<String>) {
        self.rows = rows;
        self.cursor = self.cursor.min(self.rows.len().saturating_sub(1));
        self.offset = self.offset.min(self.cursor);
    }

    /// Returns the rows.
    pub fn rows(&self) -> &[String] {
        &self.rows
    }

    /// Returns the number of rows.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Returns true when the list has no rows.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Returns the index of the row under the cursor. It is 0 for an empty list.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Moves the cursor to `index`, clamped to the last row, and scrolls it into a viewport of
    /// `viewport` rows.
    pub fn select(&mut self, index: usize, viewport: usize) {
        self.cursor = index.min(self.rows.len().saturating_sub(1));
        self.scroll_into_view(viewport);
    }

    /// Applies `motion` with a viewport of `viewport` rows.
    pub fn apply(&mut self, motion: Motion, viewport: usize) -> MotionOutcome {
        let last = self.rows.len().saturating_sub(1);
        let half = (viewport / 2).max(1);
        let mut outcome = MotionOutcome::default();
        self.cursor = match motion {
            Motion::Down => {
                outcome.end_reached = self.cursor == last;
                (self.cursor + 1).min(last)
            }
            Motion::Up => self.cursor.saturating_sub(1),
            Motion::Top => 0,
            Motion::Bottom => {
                outcome.end_reached = true;
                last
            }
            Motion::HalfPageDown => (self.cursor + half).min(last),
            Motion::HalfPageUp => self.cursor.saturating_sub(half),
        };
        if self.rows.is_empty() {
            outcome.end_reached = false;
        }
        self.scroll_into_view(viewport);
        outcome
    }

    /// Adjusts the scroll offset so the cursor is inside a viewport of `viewport` rows.
    pub fn scroll_into_view(&mut self, viewport: usize) {
        let viewport = viewport.max(1);
        if self.cursor < self.offset {
            self.offset = self.cursor;
        } else if self.cursor >= self.offset + viewport {
            self.offset = self.cursor + 1 - viewport;
        }
    }

    /// Returns the indices of the rows visible in a viewport of `viewport` rows.
    pub fn visible_range(&self, viewport: usize) -> Range<usize> {
        let start = self.offset.min(self.cursor);
        let start = if self.cursor >= start + viewport.max(1) {
            self.cursor + 1 - viewport.max(1)
        } else {
            start
        };
        start..(start + viewport).min(self.rows.len())
    }

    /// Draws the visible rows into `area`. `line` formats the row at an index to fit the given
    /// width. The row under the cursor gets the marker, and is bold when `focused`.
    pub fn render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        focused: bool,
        line: impl Fn(usize, usize) -> Line<'static>,
    ) {
        let width = usize::from(area.width).saturating_sub(1);
        for (row, index) in self.visible_range(usize::from(area.height)).enumerate() {
            let Ok(row) = u16::try_from(row) else { break };
            let selected = index == self.cursor;
            let marker = if selected { CURSOR_MARKER } else { " " };
            let mut spans = vec![Span::raw(marker)];
            spans.extend(line(index, width).spans);
            let mut rendered = Line::from(spans);
            if selected && focused {
                rendered = rendered.style(Style::default().add_modifier(Modifier::BOLD));
            }
            rendered.render(
                Rect {
                    y: area.y + row,
                    height: 1,
                    ..area
                },
                buf,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(len: usize) -> ItemList {
        let mut list = ItemList::new();
        list.set_rows((0..len).map(|i| format!("row {i}")).collect());
        list
    }

    #[test]
    fn motions_move_and_clamp() {
        let viewport = 10;
        let mut list = list(25);
        list.apply(Motion::Up, viewport);
        assert_eq!(list.cursor(), 0);
        list.apply(Motion::Down, viewport);
        assert_eq!(list.cursor(), 1);
        list.apply(Motion::HalfPageDown, viewport);
        assert_eq!(list.cursor(), 6);
        list.apply(Motion::HalfPageUp, viewport);
        assert_eq!(list.cursor(), 1);
        list.apply(Motion::Bottom, viewport);
        assert_eq!(list.cursor(), 24);
        list.apply(Motion::HalfPageDown, viewport);
        assert_eq!(list.cursor(), 24);
        list.apply(Motion::Top, viewport);
        assert_eq!(list.cursor(), 0);
        list.apply(Motion::HalfPageUp, viewport);
        assert_eq!(list.cursor(), 0);
    }

    #[test]
    fn cursor_stays_visible() {
        let viewport = 10;
        let mut list = list(25);
        list.apply(Motion::Bottom, viewport);
        assert_eq!(list.visible_range(viewport), 15..25);
        list.apply(Motion::Top, viewport);
        assert_eq!(list.visible_range(viewport), 0..10);
        for _ in 0..12 {
            list.apply(Motion::Down, viewport);
        }
        assert_eq!(list.visible_range(viewport), 3..13);
    }

    #[test]
    fn end_signal_fires_only_past_the_last_row_and_on_bottom() {
        let viewport = 10;
        let mut list = list(3);
        assert!(!list.apply(Motion::Down, viewport).end_reached);
        assert!(!list.apply(Motion::Down, viewport).end_reached);
        assert_eq!(list.cursor(), 2);
        assert!(list.apply(Motion::Down, viewport).end_reached);
        assert!(!list.apply(Motion::Up, viewport).end_reached);
        assert!(!list.apply(Motion::HalfPageDown, viewport).end_reached);
        list.apply(Motion::Top, viewport);
        assert!(list.apply(Motion::Bottom, viewport).end_reached);
        assert!(!ItemList::new().apply(Motion::Bottom, viewport).end_reached);
    }

    #[test]
    fn shrinking_rows_clamps_the_cursor() {
        let mut list = list(10);
        list.apply(Motion::Bottom, 5);
        list.set_rows(vec!["only".into()]);
        assert_eq!(list.cursor(), 0);
    }
}

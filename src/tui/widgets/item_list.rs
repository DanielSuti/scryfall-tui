//! A scrollable list with a cursor, moved with vim motions.

use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::tui::theme::Theme;
use crate::tui::widgets::pane_search::{PaneSearch, draw_position, highlight};

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
    unfocused: UnfocusedCursor,
}

/// The marker drawn in front of the cursor row of an unfocused list that shows it.
pub const CURSOR_MARKER: &str = "▌";

/// What the cursor row of a list looks like while the list doesn't have focus. With focus,
/// the row is always highlighted with the theme's `selected_row` style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UnfocusedCursor {
    /// [`CURSOR_MARKER`] in a column kept in front of every row.
    #[default]
    Marker,
    /// Nothing; the list has no marker column.
    Hidden,
}

impl ItemList {
    /// Creates an empty list showing the marker while unfocused.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an empty list whose cursor row looks like `unfocused` while unfocused.
    pub fn with_unfocused(unfocused: UnfocusedCursor) -> Self {
        Self {
            unfocused,
            ..Self::default()
        }
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

    /// Returns the columns in front of each row: 1 for the marker column, or 0.
    pub fn marker_width(&self) -> usize {
        usize::from(self.unfocused == UnfocusedCursor::Marker)
    }

    /// Draws the visible rows into `area`. `line` formats the row at an index to fit the given
    /// width. With `focused`, the cursor row is filled across the whole width with
    /// `theme.selected_row`; without, it looks as the list's [`UnfocusedCursor`] says. With
    /// `search`, the pattern's matches are drawn in `theme.find_keyword`, and the cursor row's
    /// position among the matching rows at its right end in `theme.find_position`.
    pub fn render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        focused: bool,
        theme: &Theme,
        search: Option<&PaneSearch<'_>>,
        line: impl Fn(usize, usize) -> Line<'static>,
    ) {
        let marker_width = self.marker_width();
        let width = usize::from(area.width).saturating_sub(marker_width);
        for (row, index) in self.visible_range(usize::from(area.height)).enumerate() {
            let Ok(row) = u16::try_from(row) else { break };
            let row_area = Rect {
                y: area.y + row,
                height: 1,
                ..area
            };
            let selected = index == self.cursor;
            let mut spans = Vec::new();
            if marker_width > 0 {
                let marker = if selected && !focused {
                    CURSOR_MARKER
                } else {
                    " "
                };
                spans.push(Span::raw(marker));
            }
            let content = line(index, width);
            let content = match search {
                Some(search) => highlight(content, search.pattern, theme.find_keyword),
                None => content,
            };
            spans.extend(content.spans);
            Line::from(spans).render(row_area, buf);
            if selected && focused {
                buf.set_style(row_area, theme.selected_row);
            }
            if let (true, Some(search)) = (selected, search) {
                draw_position(buf, row_area, search, theme.find_position);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Color, Modifier};

    fn list(len: usize) -> ItemList {
        let mut list = ItemList::new();
        list.set_rows((0..len).map(|i| format!("row {i}")).collect());
        list
    }

    /// Draws `list` into a 10 by 3 buffer and returns it.
    fn draw(list: &ItemList, focused: bool) -> Buffer {
        let area = Rect::new(0, 0, 10, 3);
        let mut buf = Buffer::empty(area);
        let rows = list.rows().to_vec();
        list.render(
            area,
            &mut buf,
            focused,
            &Theme::default(),
            None,
            move |i, _| Line::from(rows[i].clone()),
        );
        buf
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
    }

    #[test]
    fn the_focused_cursor_row_is_highlighted_across_the_width_without_marker() {
        let mut list = list(3);
        list.apply(Motion::Down, 3);
        let buf = draw(&list, true);
        let selected = Theme::default().selected_row;
        for x in 0..10 {
            assert_eq!(
                buf[(x, 1)].bg,
                selected.bg.unwrap(),
                "cell {x} of the cursor row"
            );
            assert!(buf[(x, 1)].modifier.contains(Modifier::BOLD));
            assert_eq!(buf[(x, 0)].bg, Color::Reset, "cell {x} of another row");
        }
        assert!(!row_text(&buf, 1).contains(CURSOR_MARKER));
        assert_eq!(row_text(&buf, 1), " row 1    ");
    }

    #[test]
    fn an_unfocused_list_shows_the_marker_or_nothing() {
        let mut marked = list(3);
        marked.apply(Motion::Down, 3);
        let buf = draw(&marked, false);
        assert_eq!(row_text(&buf, 1), "▌row 1    ");
        assert!((0..10).all(|x| buf[(x, 1)].bg == Color::Reset));

        let mut hidden = ItemList::with_unfocused(UnfocusedCursor::Hidden);
        hidden.set_rows(vec!["row 0".into(), "row 1".into()]);
        hidden.apply(Motion::Down, 3);
        let buf = draw(&hidden, false);
        assert_eq!(row_text(&buf, 1), "row 1     ", "no marker column");
        assert!((0..10).all(|x| buf[(x, 1)].bg == Color::Reset));
        let buf = draw(&hidden, true);
        assert_eq!(row_text(&buf, 1), "row 1     ");
        assert!((0..10).all(|x| buf[(x, 1)].bg == Color::Blue));
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

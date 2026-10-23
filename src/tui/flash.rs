//! Flash jumps, as in LazyVim's flash.nvim: type characters, every matching text on screen gets
//! a one-character label, and typing a label jumps there.
//!
//! Matching works on the screen as drawn, so the targets are exactly the text the user sees,
//! including cut-off text.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::tui::layout::Panel;
use crate::tui::text::display_width;
use crate::tui::widgets::pane_search::match_ranges;

/// The label characters in the order they are given out: flash.nvim's default, home row first.
pub const LABELS: &str = "asdfghjklqwertyuiopzxcvbnm";

/// One row of text on screen, inside one panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenRow {
    /// The panel the row is in.
    pub panel: Panel,
    /// The row's position from the top of the panel's text area, starting at 0.
    pub row: u16,
    /// The screen row.
    pub y: u16,
    /// The row's text, one entry per character.
    pub text: String,
    /// The screen column of each character of `text`, in order. A wide character covers its
    /// column and the next.
    pub columns: Vec<u16>,
}

impl ScreenRow {
    /// Reads the row at `y` of `area` from `buf`, skipping the cells covered by the second
    /// half of a wide character.
    pub fn read(buf: &Buffer, panel: Panel, area: Rect, y: u16) -> Self {
        let mut text = String::new();
        let mut columns = Vec::new();
        let mut x = area.x;
        while x < area.right() {
            let symbol = buf[(x, y)].symbol();
            for _ in symbol.chars() {
                columns.push(x);
            }
            text.push_str(symbol);
            let width = u16::try_from(display_width(symbol).max(1)).unwrap_or(1);
            x = x.saturating_add(width);
        }
        Self {
            panel,
            row: y - area.y,
            y,
            text,
            columns,
        }
    }

    fn column_at_byte(&self, byte: usize) -> Option<u16> {
        let index = self.text[..byte].chars().count();
        self.columns.get(index).copied()
    }
}

/// Reads every row of each `(panel, area)` from `buf`.
pub fn read_screen(buf: &Buffer, areas: &[(Panel, Rect)]) -> Vec<ScreenRow> {
    areas
        .iter()
        .flat_map(|(panel, area)| {
            (area.y..area.bottom()).map(move |y| ScreenRow::read(buf, *panel, *area, y))
        })
        .collect()
}

/// One occurrence of the typed characters on screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlashMatch {
    /// The panel it is in.
    pub panel: Panel,
    /// Its row from the top of the panel's text area.
    pub row: u16,
    /// Its screen row.
    pub y: u16,
    /// The screen columns it covers, from the first to the last.
    pub columns: (u16, u16),
    /// Its position in the row's text, counted in characters from 0.
    pub char_index: usize,
    /// The character that follows it, if any; it must never be a label.
    pub next_char: Option<char>,
    /// Its label, or `None` when the labels ran out.
    pub label: Option<char>,
}

/// Finds every occurrence of `pattern` in `rows`, as `/` matches it: ignoring case unless the
/// pattern has an uppercase letter. An empty pattern matches nothing. The matches come without
/// labels; see [`assign_labels`].
pub fn find_matches(rows: &[ScreenRow], pattern: &str) -> Vec<FlashMatch> {
    let mut matches = Vec::new();
    for row in rows {
        for range in match_ranges(&row.text, pattern) {
            let (Some(first), Some(last)) = (
                row.column_at_byte(range.start),
                row.text[..range.end]
                    .char_indices()
                    .last()
                    .and_then(|(at, _)| row.column_at_byte(at)),
            ) else {
                continue;
            };
            matches.push(FlashMatch {
                panel: row.panel,
                row: row.row,
                y: row.y,
                columns: (first, last),
                char_index: row.text[..range.start].chars().count(),
                next_char: row.text[range.end..].chars().next(),
                label: None,
            });
        }
    }
    matches
}

/// Orders `matches` nearest first and gives each a label from [`LABELS`], then their uppercase
/// forms. Matches in `cursor_panel` come first, by distance from the screen position `cursor`
/// (rows first, then columns); the other panels follow in screen order. A letter that follows
/// any match, in either case, is not used, so typing it narrows the search instead of jumping.
/// Matches beyond the available labels get none.
pub fn assign_labels(matches: &mut [FlashMatch], cursor_panel: Panel, cursor: (u16, u16)) {
    let (cx, cy) = cursor;
    matches.sort_by_key(|m| {
        let near = m.panel == cursor_panel;
        let distance = if near {
            (m.y.abs_diff(cy), m.columns.0.abs_diff(cx))
        } else {
            (m.y, m.columns.0)
        };
        (!near, distance)
    });
    let taken: Vec<char> = matches
        .iter()
        .filter_map(|m| m.next_char)
        .flat_map(char::to_lowercase)
        .collect();
    let mut labels = LABELS
        .chars()
        .chain(LABELS.chars().map(|c| c.to_ascii_uppercase()))
        .filter(|label| !taken.contains(&label.to_ascii_lowercase()));
    for m in matches.iter_mut() {
        m.label = labels.next();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::text::Line;
    use ratatui::widgets::Widget;

    fn screen(lines: &[&str]) -> (Buffer, Rect) {
        let area = Rect::new(2, 1, 20, u16::try_from(lines.len()).unwrap());
        let mut buf = Buffer::empty(Rect::new(0, 0, 24, area.bottom() + 1));
        for (i, line) in lines.iter().enumerate() {
            let row = Rect {
                y: area.y + u16::try_from(i).unwrap(),
                height: 1,
                ..area
            };
            Line::from(*line).render(row, &mut buf);
        }
        (buf, area)
    }

    #[test]
    fn rows_map_characters_to_their_columns() {
        let (buf, area) = screen(&["ab", "龍x"]);
        let rows = read_screen(&buf, &[(Panel::Main, area)]);
        assert_eq!(rows[0].text.trim_end(), "ab");
        assert_eq!(&rows[0].columns[..2], [2, 3]);
        assert_eq!(rows[1].row, 1);
        assert_eq!(rows[1].y, 2);
        assert!(rows[1].text.starts_with("龍x"));
        assert_eq!(
            &rows[1].columns[..2],
            [2, 4],
            "the wide character covers columns 2 and 3"
        );
    }

    #[test]
    fn matches_know_their_columns_and_the_next_character() {
        let (buf, area) = screen(&["Shivan Dragon", "Dragon Mage"]);
        let rows = read_screen(&buf, &[(Panel::Main, area)]);
        let matches = find_matches(&rows, "dra");
        assert_eq!(matches.len(), 2);
        assert_eq!((matches[0].row, matches[0].columns), (0, (9, 11)));
        assert_eq!(matches[0].next_char, Some('g'));
        assert_eq!(matches[0].char_index, 7);
        assert_eq!((matches[1].row, matches[1].columns), (1, (2, 4)));
        assert!(find_matches(&rows, "").is_empty());
        assert!(find_matches(&rows, "DRA").is_empty(), "smartcase");
    }

    #[test]
    fn labels_go_nearest_first_and_skip_continuations() {
        let (buf, area) = screen(&["sa", "xx", "sd", "sa"]);
        let rows = read_screen(&buf, &[(Panel::Main, area)]);
        let mut matches = find_matches(&rows, "s");
        assign_labels(&mut matches, Panel::Main, (2, 3));
        let order: Vec<(u16, Option<char>)> = matches.iter().map(|m| (m.row, m.label)).collect();
        // Row 2 is the cursor's row, row 3 one away, row 0 two away. `a` and `d` follow a
        // match, so typing them narrows the search; they are skipped.
        assert_eq!(order, [(2, Some('s')), (3, Some('f')), (0, Some('g'))]);
    }

    #[test]
    fn the_cursor_panel_comes_first_and_labels_run_out() {
        let mut many: Vec<FlashMatch> = (0..60)
            .map(|i| FlashMatch {
                panel: if i == 59 { Panel::Main } else { Panel::History },
                row: 0,
                y: 1,
                columns: (u16::try_from(i).unwrap(), u16::try_from(i).unwrap()),
                char_index: 0,
                next_char: None,
                label: None,
            })
            .collect();
        assign_labels(&mut many, Panel::Main, (0, 0));
        assert_eq!(many[0].panel, Panel::Main);
        assert_eq!(many[0].label, Some('a'));
        assert_eq!(many[26].label, Some('A'));
        assert_eq!(many[51].label, Some('M'));
        assert_eq!(many[52].label, None, "52 labels in all");
    }
}

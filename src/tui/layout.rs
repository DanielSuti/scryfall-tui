//! Screen layout: search bar, history, main panel, optional detail split and status line.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType};

use crate::app::command::Scope;

/// Below this terminal width the history panel is hidden and opens as an overlay instead.
pub const HISTORY_MIN_TERMINAL_WIDTH: u16 = 100;
/// The history panel takes this fraction of the terminal width, as `1 / HISTORY_WIDTH_DIVISOR`.
pub const HISTORY_WIDTH_DIVISOR: u16 = 4;
/// Height of the search bar, including its border.
pub const SEARCH_BAR_HEIGHT: u16 = 3;
/// Share of the main panel's width that the results keep in the split, in percent.
pub const SPLIT_RESULTS_PERCENT: u16 = 40;

/// A focusable panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Panel {
    /// The query input at the top.
    SearchBar,
    /// The search history on the left.
    History,
    /// The main panel: results, or the full detail.
    Main,
    /// The detail next to the results in the split.
    Detail,
}

impl Panel {
    /// Returns the key that jumps to the panel, shown in its title.
    pub fn jump_key(self) -> char {
        match self {
            Panel::SearchBar => '1',
            Panel::History => '2',
            Panel::Main => '3',
            Panel::Detail => '0',
        }
    }
}

/// Key bindings and commands that apply while the search bar has focus.
pub const SEARCH_BAR_SCOPE: Scope = Scope("search_bar");
/// Key bindings and commands that apply while the history has focus.
pub const HISTORY_SCOPE: Scope = Scope("history");
/// Key bindings that apply while the results have focus, without a split.
pub const RESULTS_SCOPE: Scope = Scope("results");
/// Key bindings that apply while the results have focus in the split.
pub const SPLIT_SCOPE: Scope = Scope("split");
/// Key bindings that apply while the full detail replaces the results.
pub const FULL_DETAIL_SCOPE: Scope = Scope("full_detail");
/// Key bindings that apply while the `/` prompt is open.
pub const PROMPT_SCOPE: Scope = Scope("prompt");
/// Key bindings that apply while the detail of the split has focus.
pub const DETAIL_SCOPE: Scope = Scope("detail");

/// The rectangles of one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenAreas {
    /// The search bar.
    pub search: Rect,
    /// The history panel, or `None` when the terminal is too narrow.
    pub history: Option<Rect>,
    /// The history overlay on top of the main panel, when requested on a narrow terminal.
    pub overlay: Option<Rect>,
    /// The main panel. In the split it holds only the results.
    pub main: Rect,
    /// The detail panel of the split.
    pub detail: Option<Rect>,
    /// The status line.
    pub status: Rect,
}

impl ScreenAreas {
    /// Lays out `area`. `split` shows the detail next to the results. `overlay` requests the
    /// history overlay, which appears only when the history panel itself is hidden.
    pub fn compute(area: Rect, split: bool, overlay: bool) -> Self {
        let [search, body, status] = Layout::vertical([
            Constraint::Length(SEARCH_BAR_HEIGHT),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(area);
        let history_width = area.width / HISTORY_WIDTH_DIVISOR;
        let (history, main_area) = if area.width >= HISTORY_MIN_TERMINAL_WIDTH {
            let [history, main] =
                Layout::horizontal([Constraint::Length(history_width), Constraint::Min(0)])
                    .areas(body);
            (Some(history), main)
        } else {
            (None, body)
        };
        let (main, detail) = if split {
            let [results, detail] = Layout::horizontal([
                Constraint::Percentage(SPLIT_RESULTS_PERCENT),
                Constraint::Min(0),
            ])
            .areas(main_area);
            (results, Some(detail))
        } else {
            (main_area, None)
        };
        let overlay = (overlay && history.is_none()).then(|| Rect {
            x: main_area.x + 1,
            y: main_area.y + 1,
            width: history_width.min(main_area.width.saturating_sub(2)),
            height: main_area.height.saturating_sub(2),
        });
        Self {
            search,
            history,
            overlay,
            main,
            detail,
            status,
        }
    }
}

/// A bordered panel block titled `title`. The focused panel gets a heavy border.
pub fn panel_block(title: String, focused: bool) -> Block<'static> {
    let block = Block::bordered().title(format!(" {title} "));
    if focused {
        block
            .border_type(BorderType::Thick)
            .title_style(Style::default().add_modifier(Modifier::BOLD))
    } else {
        block.border_type(BorderType::Rounded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_takes_a_quarter_at_and_above_the_threshold() {
        let at = ScreenAreas::compute(Rect::new(0, 0, 100, 20), false, false);
        assert_eq!(at.history.unwrap().width, 25);
        assert_eq!(at.main.width, 75);
        assert_eq!(at.search.height, SEARCH_BAR_HEIGHT);
        assert_eq!(at.status.y, 19);
        for (width, history) in [(112, 28), (160, 40)] {
            let areas = ScreenAreas::compute(Rect::new(0, 0, width, 20), false, false);
            assert_eq!(areas.history.unwrap().width, history);
            assert_eq!(areas.main.width, width - history);
        }
    }

    #[test]
    fn history_hides_below_the_threshold_and_opens_as_overlay() {
        let area = Rect::new(0, 0, 99, 20);
        let hidden = ScreenAreas::compute(area, false, false);
        assert_eq!(hidden.history, None);
        assert_eq!(hidden.main.width, 99);
        assert_eq!(hidden.overlay, None);
        let overlay = ScreenAreas::compute(area, false, true).overlay.unwrap();
        assert_eq!((overlay.x, overlay.width), (1, 24));
        // A visible history panel never gets an overlay.
        let wide = ScreenAreas::compute(Rect::new(0, 0, 100, 20), false, true);
        assert_eq!(wide.overlay, None);
    }

    #[test]
    fn split_divides_the_main_panel() {
        let areas = ScreenAreas::compute(Rect::new(0, 0, 112, 20), true, false);
        let detail = areas.detail.unwrap();
        assert_eq!(areas.main.width + detail.width, 84);
        assert_eq!(detail.x, areas.main.right());
    }
}

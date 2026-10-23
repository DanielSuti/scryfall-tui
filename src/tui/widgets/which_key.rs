//! The which-key popup listing the keys that complete a pending sequence.

use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Widget};
use tokio::time::Instant;

use crate::tui::keymap::dispatcher::Continuation;
use crate::tui::text::display_width;

/// How long a key sequence must stay pending before the popup appears.
pub const WHICH_KEY_DELAY: Duration = Duration::from_millis(400);

/// Tracks a pending key sequence and when its popup becomes visible.
#[derive(Debug, Clone, Default)]
pub struct WhichKey {
    pending_since: Option<Instant>,
    visible: bool,
    continuations: Vec<Continuation>,
    title: Option<&'static str>,
}

impl WhichKey {
    /// Records that a sequence is pending with `continuations`, titled `title`. The popup
    /// appears at once when `instant` is true, and otherwise after [`WHICH_KEY_DELAY`]. A popup
    /// already on screen stays visible and shows the new continuations.
    pub fn start(
        &mut self,
        continuations: Vec<Continuation>,
        now: Instant,
        title: Option<&'static str>,
        instant: bool,
    ) {
        if !self.visible {
            self.pending_since = Some(now);
            self.visible = instant;
        }
        self.continuations = continuations;
        self.title = title;
    }

    /// Returns the title of the pending prefix, if it has one.
    pub fn title(&self) -> Option<&'static str> {
        self.title
    }

    /// Forgets the pending sequence and hides the popup.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Shows the popup once [`WHICH_KEY_DELAY`] has passed since the sequence started.
    pub fn tick(&mut self, now: Instant) {
        if let Some(since) = self.pending_since
            && now >= since + WHICH_KEY_DELAY
        {
            self.visible = true;
        }
    }

    /// Returns when the popup should appear, or `None` when nothing is pending or it is shown.
    pub fn deadline(&self) -> Option<Instant> {
        match (self.pending_since, self.visible) {
            (Some(since), false) => Some(since + WHICH_KEY_DELAY),
            _ => None,
        }
    }

    /// Returns true when the popup is on screen.
    pub fn visible(&self) -> bool {
        self.visible
    }

    /// Returns the continuations of the pending sequence.
    pub fn continuations(&self) -> &[Continuation] {
        &self.continuations
    }
}

/// One line of the popup: the keys still to press and what they do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhichKeyEntry {
    /// The remaining keys, in key notation.
    pub keys: String,
    /// The command description.
    pub description: String,
}

/// The popup, drawn in the bottom-right corner of the area it is given.
#[derive(Debug, Clone)]
pub struct WhichKeyPopup<'a> {
    entries: &'a [WhichKeyEntry],
    title: Option<&'a str>,
}

/// Separates the subject of a description from its detail, as in `Name: A → Z`.
pub const DETAIL_SEPARATOR: &str = ": ";

/// Merges each lowercase single-key entry with its uppercase twin when both descriptions
/// share the text before [`DETAIL_SEPARATOR`]: `n Name: A → Z` and `N Name: Z → A` become
/// `n/N Name: A → Z / Z → A`, in the place of the lowercase entry. Other entries keep their
/// order.
pub fn pair_case_twins(entries: Vec<WhichKeyEntry>) -> Vec<WhichKeyEntry> {
    let split = |e: &WhichKeyEntry| {
        e.description
            .split_once(DETAIL_SEPARATOR)
            .map(|(stem, detail)| (stem.to_owned(), detail.to_owned()))
    };
    let twin_of = |lower: &WhichKeyEntry, other: &WhichKeyEntry| {
        let mut chars = lower.keys.chars();
        let (Some(c), None) = (chars.next(), chars.next()) else {
            return false;
        };
        c.is_lowercase()
            && other.keys == c.to_uppercase().to_string()
            && matches!((split(lower), split(other)), (Some((a, _)), Some((b, _))) if a == b)
    };
    let mut merged = Vec::new();
    let mut used = vec![false; entries.len()];
    for (i, entry) in entries.iter().enumerate() {
        if used[i] {
            continue;
        }
        let twin =
            (0..entries.len()).find(|j| !used[*j] && *j != i && twin_of(entry, &entries[*j]));
        match (twin, split(entry)) {
            (Some(j), Some((stem, detail))) => {
                used[j] = true;
                let (_, twin_detail) = split(&entries[j]).unwrap_or_default();
                merged.push(WhichKeyEntry {
                    keys: format!("{}/{}", entry.keys, entries[j].keys),
                    description: format!("{stem}{DETAIL_SEPARATOR}{detail} / {twin_detail}"),
                });
            }
            _ => merged.push(entry.clone()),
        }
        used[i] = true;
    }
    merged
}

impl<'a> WhichKeyPopup<'a> {
    /// Creates a popup listing `entries`.
    pub fn new(entries: &'a [WhichKeyEntry]) -> Self {
        Self {
            entries,
            title: None,
        }
    }

    /// Sets the title drawn in the border.
    #[must_use]
    pub fn title(mut self, title: Option<&'a str>) -> Self {
        self.title = title;
        self
    }

    fn size(&self, area: Rect) -> (u16, u16) {
        let keys = self.keys_width();
        let desc = self
            .entries
            .iter()
            .map(|e| display_width(&e.description))
            .chain(
                self.title
                    .map(|t| display_width(t).saturating_sub(keys + 1)),
            )
            .max()
            .unwrap_or(0);
        let width = u16::try_from(keys + desc + 5).unwrap_or(u16::MAX);
        let height = u16::try_from(self.entries.len() + 2).unwrap_or(u16::MAX);
        (width.min(area.width), height.min(area.height))
    }
}

impl WhichKeyPopup<'_> {
    fn keys_width(&self) -> usize {
        self.entries
            .iter()
            .map(|e| display_width(&e.keys))
            .max()
            .unwrap_or(0)
    }
}

impl Widget for WhichKeyPopup<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let (width, height) = self.size(area);
        let popup = Rect {
            x: area.right() - width,
            y: area.bottom() - height,
            width,
            height,
        };
        Clear.render(popup, buf);
        let mut block = Block::bordered().border_type(BorderType::Rounded);
        if let Some(title) = self.title {
            block = block.title(format!(" {title} "));
        }
        let inner = block.inner(popup);
        block.render(popup, buf);
        let keys_width = self.keys_width();
        for (row, entry) in self.entries.iter().enumerate() {
            let Ok(row) = u16::try_from(row) else { break };
            if row >= inner.height {
                break;
            }
            let line = Line::from(vec![
                Span::styled(
                    format!(" {:keys_width$} ", entry.keys),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(entry.description.as_str()),
            ]);
            line.render(
                Rect {
                    y: inner.y + row,
                    height: 1,
                    ..inner
                },
                buf,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(keys: &str, description: &str) -> WhichKeyEntry {
        WhichKeyEntry {
            keys: keys.into(),
            description: description.into(),
        }
    }

    #[test]
    fn case_twins_share_one_line() {
        let merged = pair_case_twins(vec![
            entry("n", "Name: A → Z"),
            entry("d", "Release date: newest first"),
            entry("N", "Name: Z → A"),
            entry("x", "Lone key"),
            entry("q", "Quit: now"),
            entry("Q", "Leave: later"),
            entry("ss", "New search"),
        ]);
        assert_eq!(
            merged,
            [
                entry("n/N", "Name: A → Z / Z → A"),
                entry("d", "Release date: newest first"),
                entry("x", "Lone key"),
                entry("q", "Quit: now"),
                entry("Q", "Leave: later"),
                entry("ss", "New search"),
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn an_instant_prefix_is_visible_at_once() {
        let mut which_key = WhichKey::default();
        which_key.start(Vec::new(), Instant::now(), Some("Sort by"), true);
        assert!(which_key.visible());
        assert_eq!(which_key.title(), Some("Sort by"));
        assert_eq!(which_key.deadline(), None);
    }

    #[tokio::test(start_paused = true)]
    async fn becomes_visible_after_the_delay() {
        let mut which_key = WhichKey::default();
        which_key.start(Vec::new(), Instant::now(), None, false);
        tokio::time::advance(WHICH_KEY_DELAY - Duration::from_millis(1)).await;
        which_key.tick(Instant::now());
        assert!(!which_key.visible());
        tokio::time::advance(Duration::from_millis(1)).await;
        which_key.tick(Instant::now());
        assert!(which_key.visible());
        assert_eq!(which_key.deadline(), None);
    }
}

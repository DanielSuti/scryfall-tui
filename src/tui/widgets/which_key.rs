//! The which-key popup listing the keys that complete a pending sequence.

use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Widget};
use tokio::time::Instant;

use crate::tui::keymap::dispatcher::Continuation;

/// How long a key sequence must stay pending before the popup appears.
pub const WHICH_KEY_DELAY: Duration = Duration::from_millis(400);

/// Tracks a pending key sequence and when its popup becomes visible.
#[derive(Debug, Clone, Default)]
pub struct WhichKey {
    pending_since: Option<Instant>,
    visible: bool,
    continuations: Vec<Continuation>,
}

impl WhichKey {
    /// Records that a sequence is pending with `continuations`. A popup already on screen
    /// stays visible and shows the new continuations.
    pub fn start(&mut self, continuations: Vec<Continuation>, now: Instant) {
        if !self.visible {
            self.pending_since = Some(now);
        }
        self.continuations = continuations;
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
}

impl<'a> WhichKeyPopup<'a> {
    /// Creates a popup listing `entries`.
    pub fn new(entries: &'a [WhichKeyEntry]) -> Self {
        Self { entries }
    }

    fn size(&self, area: Rect) -> (u16, u16) {
        let keys = self.entries.iter().map(|e| e.keys.len()).max().unwrap_or(0);
        let desc = self
            .entries
            .iter()
            .map(|e| e.description.len())
            .max()
            .unwrap_or(0);
        let width = u16::try_from(keys + desc + 5).unwrap_or(u16::MAX);
        let height = u16::try_from(self.entries.len() + 2).unwrap_or(u16::MAX);
        (width.min(area.width), height.min(area.height))
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
        let block = Block::bordered().border_type(BorderType::Rounded);
        let inner = block.inner(popup);
        block.render(popup, buf);
        let keys_width = self.entries.iter().map(|e| e.keys.len()).max().unwrap_or(0);
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

    #[tokio::test(start_paused = true)]
    async fn becomes_visible_after_the_delay() {
        let mut which_key = WhichKey::default();
        which_key.start(Vec::new(), Instant::now());
        tokio::time::advance(WHICH_KEY_DELAY - Duration::from_millis(1)).await;
        which_key.tick(Instant::now());
        assert!(!which_key.visible());
        tokio::time::advance(Duration::from_millis(1)).await;
        which_key.tick(Instant::now());
        assert!(which_key.visible());
        assert_eq!(which_key.deadline(), None);
    }
}

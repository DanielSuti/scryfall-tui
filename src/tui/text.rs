//! Text measurement and truncation by terminal display width.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// The marker that ends truncated text.
pub const ELLIPSIS: &str = "...";

/// Returns the number of terminal columns `text` occupies.
pub fn display_width(text: &str) -> usize {
    text.width()
}

/// Fits `text` into `width` terminal columns.
///
/// Text that fits is returned unchanged. Longer text is cut and ends in [`ELLIPSIS`], so the
/// result never occupies more than `width` columns. A cut never splits a wide character, so the
/// result can be one column narrower than `width`. Below 3 columns only part of the ellipsis
/// fits, and that part is returned.
pub fn truncate(text: &str, width: usize) -> String {
    if display_width(text) <= width {
        return text.to_owned();
    }
    let marker_width = display_width(ELLIPSIS);
    if width < marker_width {
        return ELLIPSIS[..width].to_owned();
    }
    let budget = width - marker_width;
    let mut used = 0;
    let mut out = String::new();
    for ch in text.chars() {
        let w = ch.width().unwrap_or(0);
        if used + w > budget {
            break;
        }
        used += w;
        out.push(ch);
    }
    out.push_str(ELLIPSIS);
    out
}

/// Breaks `text` into lines of at most `width` columns. Existing line breaks are kept, words
/// are moved to the next line whole, and a word longer than `width` is cut with [`truncate`].
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let word = truncate(word, width);
            let needed =
                display_width(&line) + usize::from(!line.is_empty()) + display_width(&word);
            if !line.is_empty() && needed > width {
                lines.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(&word);
        }
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_that_fits_is_unchanged() {
        assert_eq!(truncate("Sol Ring", 8), "Sol Ring");
        assert_eq!(truncate("Sol Ring", 20), "Sol Ring");
    }

    #[test]
    fn one_column_too_long_ends_in_ellipsis_at_full_width() {
        let cut = truncate("Glorybringer", 11);
        assert_eq!(cut, "Glorybri...");
        assert_eq!(display_width(&cut), 11);
    }

    #[test]
    fn widths_below_the_marker_return_part_of_it() {
        assert_eq!(truncate("Glorybringer", 2), "..");
        assert_eq!(truncate("Glorybringer", 0), "");
        assert_eq!(truncate("Glorybringer", 3), "...");
    }

    #[test]
    fn accented_characters_count_one_column() {
        // 23 characters, 23 columns, 25 bytes.
        let name = "Lim-Dûl the Necromancer";
        assert_eq!(truncate(name, 23), name);
        assert_eq!(truncate(name, 10), "Lim-Dûl...");
    }

    #[test]
    fn wrap_moves_whole_words_and_keeps_line_breaks() {
        assert_eq!(
            wrap("Flying, haste\nYou may exert this creature", 12),
            ["Flying,", "haste", "You may", "exert this", "creature"]
        );
        assert_eq!(wrap("", 10), [""]);
        assert_eq!(wrap("Abracadabrantesque", 8), ["Abrac..."]);
        for line in wrap("one two three four five six seven", 9) {
            assert!(display_width(&line) <= 9, "{line}");
        }
    }

    #[test]
    fn wide_characters_are_never_split() {
        // Each of these characters is two columns wide.
        let cut = truncate("龍龍龍龍", 6);
        assert_eq!(cut, "龍...");
        assert_eq!(display_width(&cut), 5);
        let cut = truncate("龍龍龍龍", 7);
        assert_eq!(cut, "龍龍...");
        assert_eq!(display_width(&cut), 7);
    }
}

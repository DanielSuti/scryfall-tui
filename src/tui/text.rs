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

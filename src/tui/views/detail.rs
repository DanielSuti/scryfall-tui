//! The detail of one card: every face's text, the chosen formats' legalities, then the
//! rulings, with room for the card image when the terminal can draw one.

use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::domain::card::{CARD_IMAGE_PIXELS, Card, Face};
use crate::domain::ruling::{Ruling, RulingsState};
use crate::tui::text::{display_width, truncate, wrap, wrap_with};
use crate::tui::theme::Theme;
use crate::tui::widgets::item_list::{Motion, MotionOutcome};
use crate::tui::widgets::pane_search::{
    Navigable, PaneSearch, Searchable, draw_position, highlight,
};

/// From this width on the legalities are laid out in two columns.
pub const LEGALITY_TWO_COLUMN_MIN_WIDTH: usize = 50;
/// Columns reserved for a format name in the legalities.
const FORMAT_WIDTH: usize = 10;
/// Columns reserved for a status in the legalities.
const STATUS_WIDTH: usize = 10;
/// From this width on each ruling's date stands in a column left of its text.
pub const RULING_DATE_COLUMN_MIN_WIDTH: usize = 36;
/// Columns of a ruling's date and the gap after it.
const DATE_COLUMN_WIDTH: usize = 12;
/// The split's image takes this share of the detail's inner width, in percent.
pub const SPLIT_IMAGE_PERCENT: u16 = 40;
/// The split shows no image when the text beside it would be narrower than this.
pub const MIN_TEXT_BESIDE_IMAGE: usize = 24;
/// The full detail shows no image when the text right of it would be narrower than this.
pub const MIN_TEXT_RIGHT_OF_IMAGE: usize = 30;
/// Columns between the image and the text.
const IMAGE_GAP: u16 = 2;
/// Columns right of the split's image, matching the indent left of the text.
const IMAGE_MARGIN: u16 = 1;

/// Where the card image stands in the detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagePlacement {
    /// At the top right, in the split. The text wraps beside it and then below it, and the
    /// image scrolls with the text.
    Beside,
    /// On the left at the panel's height, in the full detail. The text scrolls right of it.
    Left,
}

/// The cells the card image takes in the detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageBox {
    /// Where it stands.
    pub placement: ImagePlacement,
    /// Its width and height in cells.
    pub size: Size,
}

impl ImageBox {
    /// Returns the box of the split's image in a detail of `inner` cells, each `cell` pixels in
    /// size: [`SPLIT_IMAGE_PERCENT`] of the width, at most the height. Returns `None` when the
    /// text beside it would be narrower than [`MIN_TEXT_BESIDE_IMAGE`].
    pub fn beside(inner: Size, cell: Size) -> Option<Self> {
        let mut width = inner.width * SPLIT_IMAGE_PERCENT / 100;
        let mut height = rows_for(width, cell);
        if height > inner.height {
            height = inner.height;
            width = cols_for(height, cell);
        }
        let text = usize::from(inner.width)
            .checked_sub(usize::from(1 + width + IMAGE_GAP + IMAGE_MARGIN))?;
        (width > 0 && height > 0 && text >= MIN_TEXT_BESIDE_IMAGE).then_some(Self {
            placement: ImagePlacement::Beside,
            size: Size::new(width, height),
        })
    }

    /// Returns the box of the full detail's image in a detail of `inner` cells, each `cell`
    /// pixels in size: the full height, at most half the width. Returns `None` when the text
    /// right of it would be narrower than [`MIN_TEXT_RIGHT_OF_IMAGE`].
    pub fn left(inner: Size, cell: Size) -> Option<Self> {
        let mut height = inner.height;
        let mut width = cols_for(height, cell);
        if width > inner.width / 2 {
            width = inner.width / 2;
            height = rows_for(width, cell);
        }
        let text = usize::from(inner.width).checked_sub(usize::from(1 + width + IMAGE_GAP))?;
        (width > 0 && height > 0 && text >= MIN_TEXT_RIGHT_OF_IMAGE).then_some(Self {
            placement: ImagePlacement::Left,
            size: Size::new(width, height),
        })
    }

    /// Returns the text widths when the full text width is `width`.
    fn widths(self, width: usize) -> LineWidths {
        let taken = usize::from(self.size.width + IMAGE_GAP);
        match self.placement {
            ImagePlacement::Beside => LineWidths {
                narrow: width.saturating_sub(taken + usize::from(IMAGE_MARGIN)),
                narrow_lines: usize::from(self.size.height),
                full: width,
            },
            ImagePlacement::Left => LineWidths::uniform(width.saturating_sub(taken)),
        }
    }

    /// Returns the columns left of the text taken by the image and the gap after it.
    fn text_offset(self) -> u16 {
        match self.placement {
            ImagePlacement::Beside => 0,
            ImagePlacement::Left => self.size.width + IMAGE_GAP,
        }
    }
}

/// Returns the rows a card image `cols` cells wide takes with cells of `cell` pixels.
fn rows_for(cols: u16, cell: Size) -> u16 {
    let (w, h) = CARD_IMAGE_PIXELS;
    let pixels = u32::from(cols) * u32::from(cell.width) * h;
    let per_row = w * u32::from(cell.height).max(1);
    u16::try_from((pixels + per_row / 2) / per_row).unwrap_or(u16::MAX)
}

/// Returns the columns a card image `rows` cells high takes with cells of `cell` pixels.
fn cols_for(rows: u16, cell: Size) -> u16 {
    let (w, h) = CARD_IMAGE_PIXELS;
    let pixels = u32::from(rows) * u32::from(cell.height) * w;
    let per_col = h * u32::from(cell.width).max(1);
    u16::try_from((pixels + per_col / 2) / per_col).unwrap_or(u16::MAX)
}

/// What the image box holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageStatus {
    /// The image is on its way.
    Loading,
    /// The image is drawn into the box by the caller.
    Ready,
    /// The image could not be fetched or decoded, for this reason.
    Failed(String),
}

/// The card image's box and what it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailImage {
    /// The box.
    pub area: ImageBox,
    /// What it holds.
    pub status: ImageStatus,
}

/// The widths of the detail's lines: the first `narrow_lines` are `narrow` columns wide, the
/// rest `full`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineWidths {
    /// The width of the first lines.
    pub narrow: usize,
    /// How many lines are narrow.
    pub narrow_lines: usize,
    /// The width of the other lines.
    pub full: usize,
}

impl LineWidths {
    /// Every line `width` columns wide.
    pub fn uniform(width: usize) -> Self {
        Self {
            narrow: width,
            narrow_lines: 0,
            full: width,
        }
    }

    /// Returns the width of line `index`.
    pub fn at(&self, index: usize) -> usize {
        if index < self.narrow_lines {
            self.narrow
        } else {
            self.full
        }
    }
}

impl Default for LineWidths {
    fn default() -> Self {
        Self::uniform(0)
    }
}

/// A card formatted for the detail.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardText {
    /// The lines, each at most as wide as asked.
    pub lines: Vec<String>,
    /// The indices of the lines that hold flavor text, one range per face.
    pub flavor: Vec<Range<usize>>,
}

impl CardText {
    /// Returns true when line `index` holds flavor text.
    pub fn is_flavor(&self, index: usize) -> bool {
        self.flavor.iter().any(|range| range.contains(&index))
    }
}

/// Formats `card` into lines of at most `width` columns: for each face its name and cost,
/// its full type line and stats, its oracle text and its flavor text, with faces separated
/// by a `//` line; then the legality of each of `formats` the card reports, in that order;
/// then `rulings`, when given.
pub fn format_card(
    card: &Card,
    formats: &[String],
    rulings: Option<&RulingsState>,
    width: usize,
) -> CardText {
    format_card_in(card, formats, rulings, LineWidths::uniform(width))
}

/// Formats `card` as [`format_card`] does, with each line at most as wide as `widths` says.
/// The legalities and each ruling are laid out for the width of the line they start on.
pub fn format_card_in(
    card: &Card,
    formats: &[String],
    rulings: Option<&RulingsState>,
    widths: LineWidths,
) -> CardText {
    let mut lines = Vec::new();
    let mut flavor = Vec::new();
    for (index, face) in card.faces().iter().enumerate() {
        if index > 0 {
            lines.push(String::new());
            lines.push("//".to_owned());
            lines.push(String::new());
        }
        format_face(face, widths, &mut lines, &mut flavor);
    }
    let legalities: Vec<String> = formats
        .iter()
        .filter_map(|format| {
            let status = card.legalities().get(format)?;
            Some(format!(
                "{:FORMAT_WIDTH$} {:STATUS_WIDTH$}",
                truncate(format, FORMAT_WIDTH),
                status.to_string()
            ))
        })
        .collect();
    if !legalities.is_empty() {
        lines.push(String::new());
        if widths.at(lines.len()) >= LEGALITY_TWO_COLUMN_MIN_WIDTH {
            let rows = legalities.len().div_ceil(2);
            for row in 0..rows {
                let left = &legalities[row];
                match legalities.get(row + rows) {
                    Some(right) => lines.push(format!("{left}   {right}").trim_end().to_owned()),
                    None => lines.push(left.trim_end().to_owned()),
                }
            }
        } else {
            lines.extend(legalities.iter().map(|l| l.trim_end().to_owned()));
        }
    }
    if let Some(rulings) = rulings {
        lines.push(String::new());
        format_rulings(rulings, widths, &mut lines);
    }
    CardText {
        lines: lines
            .into_iter()
            .enumerate()
            .map(|(index, line)| truncate(&line, widths.at(index)))
            .collect(),
        flavor,
    }
}

/// Adds the lines of `face` to `lines`, and the range of its flavor text lines to `flavor`.
fn format_face(
    face: &Face,
    widths: LineWidths,
    lines: &mut Vec<String>,
    flavor: &mut Vec<Range<usize>>,
) {
    let width_at = |index| widths.at(index);
    lines.push(spread(
        &face.name,
        face.display_cost(),
        widths.at(lines.len()),
    ));
    let type_line = face.type_line.as_ref().map_or("—", |t| t.full());
    let stats = face.stats().unwrap_or_default();
    lines.push(spread(type_line, &stats, widths.at(lines.len())));
    if let Some(text) = &face.oracle_text {
        lines.push(String::new());
        lines.extend(wrap_with(text, lines.len(), width_at));
    }
    if let Some(text) = &face.flavor_text {
        lines.push(String::new());
        let start = lines.len();
        lines.extend(wrap_with(text, start, width_at));
        flavor.push(start..lines.len());
    }
}

/// Formats `rulings` under a heading with their count. From
/// [`RULING_DATE_COLUMN_MIN_WIDTH`] on, each date stands left of its text; on narrower
/// panels it takes a line above it.
fn format_rulings(rulings: &RulingsState, widths: LineWidths, lines: &mut Vec<String>) {
    let width_at = |index| widths.at(index);
    let list = match rulings {
        RulingsState::Loading => return lines.push("Rulings: loading...".to_owned()),
        RulingsState::Failed(message) => {
            let text = format!("Rulings: {message}");
            return lines.extend(wrap_with(&text, lines.len(), width_at));
        }
        RulingsState::Loaded(list) if list.is_empty() => {
            return lines.push("No rulings".to_owned());
        }
        RulingsState::Loaded(list) => list,
    };
    lines.push(format!("Rulings ({})", list.len()));
    for Ruling {
        published_at,
        comment,
    } in list
    {
        if widths.at(lines.len()) < RULING_DATE_COLUMN_MIN_WIDTH {
            lines.push(published_at.clone());
            lines.extend(wrap_with(comment, lines.len(), width_at));
            continue;
        }
        let indent = " ".repeat(DATE_COLUMN_WIDTH);
        let text_width_at = |index| widths.at(index).saturating_sub(DATE_COLUMN_WIDTH);
        for (index, text) in wrap_with(comment, lines.len(), text_width_at)
            .into_iter()
            .enumerate()
        {
            let left = if index == 0 {
                format!("{published_at:DATE_COLUMN_WIDTH$}")
            } else {
                indent.clone()
            };
            lines.push(format!("{left}{text}"));
        }
    }
}

/// Puts `left` at the start and `right` at the end of a line of `width` columns. When both
/// don't fit, `left` is cut with `...`.
fn spread(left: &str, right: &str, width: usize) -> String {
    if right.is_empty() {
        return truncate(left, width);
    }
    let right_width = display_width(right);
    let left = truncate(left, width.saturating_sub(right_width + 1));
    let gap = width
        .saturating_sub(display_width(&left) + right_width)
        .max(1);
    format!("{left}{}{right}", " ".repeat(gap))
}

/// The detail lines of the selected card, the scroll position, and the card image's box.
#[derive(Debug, Clone, Default)]
pub struct DetailView {
    card_id: Option<String>,
    text: CardText,
    widths: LineWidths,
    image: Option<DetailImage>,
    scroll: usize,
}

impl DetailView {
    /// Creates an empty view.
    pub fn new() -> Self {
        Self::default()
    }

    /// Shows `card` with `rulings` and `image`, formatted for `width` columns less the space
    /// the image takes. The scroll goes back to the top when the card changes.
    pub fn show(
        &mut self,
        card: Option<&Card>,
        formats: &[String],
        rulings: Option<&RulingsState>,
        image: Option<DetailImage>,
        width: usize,
    ) {
        let id = card.map(|c| c.id().to_owned());
        if id != self.card_id {
            self.scroll = 0;
            self.card_id = id;
        }
        self.image = image.filter(|_| card.is_some());
        self.widths = self
            .image
            .as_ref()
            .map_or(LineWidths::uniform(width), |i| i.area.widths(width));
        self.text = card.map_or_else(CardText::default, |c| {
            format_card_in(c, formats, rulings, self.widths)
        });
        self.scroll = self.scroll.min(self.text.lines.len().saturating_sub(1));
    }

    /// Returns the formatted lines.
    pub fn lines(&self) -> &[String] {
        &self.text.lines
    }

    /// Returns the index of the top visible line.
    pub fn scroll(&self) -> usize {
        self.scroll
    }

    /// Returns the card image's box and what it holds, if the detail has one.
    pub fn image(&self) -> Option<&DetailImage> {
        self.image.as_ref()
    }

    /// Returns where the card image is drawn when the detail is drawn into `area`: the
    /// rectangle to draw into, and the row of the image's top relative to it, negative once
    /// the split has scrolled past the top. Returns `None` without an image box.
    pub fn image_area(&self, area: Rect) -> Option<(Rect, i16)> {
        let image = self.image.as_ref()?.area;
        match image.placement {
            ImagePlacement::Beside => {
                let width = image.size.width.min(area.width);
                let x = area
                    .right()
                    .saturating_sub(width + IMAGE_MARGIN)
                    .max(area.x);
                let top = i16::try_from(self.scroll).map_or(i16::MIN, |s| -s);
                Some((Rect { x, width, ..area }, top))
            }
            ImagePlacement::Left => {
                let rect = Rect {
                    x: area.x + 1,
                    width: image.size.width,
                    height: image.size.height,
                    ..area
                }
                .intersection(area);
                Some((rect, 0))
            }
        }
    }

    /// Draws the lines from the scroll position into `area`, each indented by one column and
    /// placed right of an image on the left, with flavor text in `theme.flavor` and the
    /// matches of `search` in `theme.find_keyword`. The top line, the current one, shows its
    /// position among the matching lines at its right end. The image box shows its status
    /// while the image itself is not ready.
    pub fn render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        search: Option<&PaneSearch<'_>>,
    ) {
        if self.text.lines.is_empty() {
            buf.set_line(
                area.x + 1,
                area.y,
                &Line::from("No card selected"),
                area.width,
            );
            return;
        }
        let offset = self.image.as_ref().map_or(0, |i| i.area.text_offset());
        let x = (area.x + offset).min(area.right());
        for (index, line) in self.text.lines.iter().enumerate().skip(self.scroll) {
            let Ok(row) = u16::try_from(index - self.scroll) else {
                break;
            };
            if row >= area.height {
                break;
            }
            let style = if self.text.is_flavor(index) {
                theme.flavor
            } else {
                Style::default()
            };
            let mut text = Line::from(Span::styled(line.clone(), style));
            if let Some(search) = search {
                text = highlight(text, search.pattern, theme.find_keyword);
            }
            // The line's own width, so nothing is drawn over an image beside it.
            let width = u16::try_from(self.widths.at(index) + 1)
                .unwrap_or(u16::MAX)
                .min(area.right() - x);
            buf.set_line(x + 1, area.y + row, &text, width.saturating_sub(1));
            if let (0, Some(search)) = (row, search) {
                let line_area = Rect {
                    x,
                    y: area.y,
                    width,
                    height: 1,
                };
                draw_position(buf, line_area, search, theme.find_position);
            }
        }
        self.render_image_status(area, buf, theme);
    }

    /// Writes into the image box that the image is loading or why it failed.
    fn render_image_status(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let Some(image) = &self.image else { return };
        let Some((rect, top)) = self.image_area(area) else {
            return;
        };
        let (lines, style) = match &image.status {
            ImageStatus::Ready => return,
            ImageStatus::Loading => (vec!["Loading image...".to_owned()], theme.dim),
            ImageStatus::Failed(message) => (
                wrap(&format!("Image: {message}"), usize::from(rect.width)),
                theme.error,
            ),
        };
        let lines = lines.iter().take(usize::from(image.area.size.height));
        for (index, line) in lines.enumerate() {
            let row = i32::from(top) + i32::try_from(index).unwrap_or(i32::MAX);
            if row < 0 || row >= i32::from(rect.height) {
                continue;
            }
            let y = rect.y + u16::try_from(row).unwrap_or(0);
            let text = Line::from(Span::styled(truncate(line, usize::from(rect.width)), style));
            buf.set_line(rect.x, y, &text, rect.width);
        }
    }
}

impl Searchable for DetailView {
    fn search_lines(&self) -> &[String] {
        &self.text.lines
    }

    fn current_line(&self) -> usize {
        self.scroll
    }

    fn jump_to(&mut self, index: usize, _viewport: usize) {
        self.scroll = index.min(self.text.lines.len().saturating_sub(1));
    }

    fn first_visible(&self, _viewport: usize) -> usize {
        self.scroll
    }
}

impl Navigable for DetailView {
    fn apply_motion(&mut self, motion: Motion, viewport: usize) -> MotionOutcome {
        let last = self.text.lines.len().saturating_sub(viewport.max(1));
        let half = (viewport / 2).max(1);
        self.scroll = match motion {
            Motion::Down => (self.scroll + 1).min(last),
            Motion::Up => self.scroll.saturating_sub(1),
            Motion::Top => 0,
            Motion::Bottom => last,
            Motion::HalfPageDown => (self.scroll + half).min(last),
            Motion::HalfPageUp => self.scroll.saturating_sub(half),
        };
        MotionOutcome::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scryfall::parse::parse_card;
    use ratatui::style::Modifier;

    fn fixture(name: &str) -> Card {
        let path = format!(
            "{}/tests/fixtures/scryfall/{name}.json",
            env!("CARGO_MANIFEST_DIR")
        );
        parse_card(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    fn formats(names: &[&str]) -> Vec<String> {
        names.iter().map(|f| (*f).to_owned()).collect()
    }

    #[test]
    fn single_face_shows_text_flavor_and_legalities() {
        let card = fixture("normal_glorybringer");
        let lines = format_card(&card, &formats(&["standard", "commander"]), None, 40).lines;
        assert_eq!(
            lines[0],
            format!("Glorybringer{}{{3}}{{R}}{{R}}", " ".repeat(19))
        );
        assert_eq!(lines[1], format!("Creature — Dragon{}4/4", " ".repeat(20)));
        assert_eq!(lines[3], "Flying, haste");
        let flavor = card.faces()[0].flavor_text.clone().unwrap();
        let first_flavor_word = flavor.split_whitespace().next().unwrap();
        assert!(
            lines.iter().any(|l| l.starts_with(first_flavor_word)),
            "{lines:#?}"
        );
        assert_eq!(
            &lines[lines.len() - 2..],
            ["standard   not legal", "commander  legal"]
        );
        assert!(lines.iter().all(|l| display_width(l) <= 40));
    }

    #[test]
    fn faces_are_separated_and_each_has_its_own_block() {
        let card = fixture("transform_delver");
        let lines = format_card(&card, &[], None, 40).lines;
        let separator = lines.iter().position(|l| l == "//").unwrap();
        assert!(lines[0].starts_with("Delver of Secrets") && lines[0].ends_with("{U}"));
        assert!(lines[1].starts_with("Creature — Human Wizard") && lines[1].ends_with("1/1"));
        let back = &lines[separator + 2];
        assert!(
            back.starts_with("Insectile Aberration") && back.ends_with("—"),
            "{back}"
        );
        assert!(lines[separator + 3].ends_with("3/2"));
    }

    #[test]
    fn split_and_adventure_cards_show_both_halves() {
        for (name, halves) in [
            ("split_fire_ice", ["Fire", "Ice"]),
            ("adventure_bonecrusher", ["Bonecrusher Giant", "Stomp"]),
        ] {
            let lines = format_card(&fixture(name), &[], None, 60).lines;
            for half in halves {
                assert!(
                    lines.iter().any(|l| l.starts_with(half)),
                    "{name}: {lines:#?}"
                );
            }
        }
    }

    #[test]
    fn legalities_use_two_columns_when_wide() {
        let card = fixture("normal_glorybringer");
        let chosen = formats(&["standard", "pioneer", "modern"]);
        let wide = format_card(&card, &chosen, None, 60).lines;
        assert_eq!(
            &wide[wide.len() - 2..],
            [
                "standard   not legal    modern     legal",
                "pioneer    legal"
            ]
        );
        let unknown = format_card(&card, &formats(&["bogus"]), None, 60).lines;
        assert!(!unknown.iter().any(|l| l.contains("bogus")));
    }

    fn rulings() -> RulingsState {
        RulingsState::Loaded(vec![
            Ruling {
                published_at: "2017-04-18".to_owned(),
                comment: "You can’t exert a creature unless an effect allows you to do so."
                    .to_owned(),
            },
            Ruling {
                published_at: "2017-04-19".to_owned(),
                comment: "Exert is not tapping.".to_owned(),
            },
        ])
    }

    /// Returns the lines after the legalities: those `rulings` adds.
    fn ruling_lines(rulings: Option<&RulingsState>, width: usize) -> Vec<String> {
        let card = fixture("normal_glorybringer");
        let chosen = formats(&["standard"]);
        let without = format_card(&card, &chosen, None, width).lines.len();
        format_card(&card, &chosen, rulings, width).lines[without..].to_vec()
    }

    #[test]
    fn rulings_follow_the_legalities_with_their_dates_in_a_column() {
        assert!(ruling_lines(None, 40).is_empty());
        assert_eq!(
            ruling_lines(Some(&rulings()), 40),
            [
                "",
                "Rulings (2)",
                "2017-04-18  You can’t exert a creature",
                "            unless an effect allows you",
                "            to do so.",
                "2017-04-19  Exert is not tapping.",
            ]
        );
    }

    #[test]
    fn narrow_panels_put_each_date_above_its_ruling() {
        let lines = ruling_lines(Some(&rulings()), 30);
        assert_eq!(
            &lines[..5],
            [
                "",
                "Rulings (2)",
                "2017-04-18",
                "You can’t exert a creature",
                "unless an effect allows you to",
            ]
        );
        assert!(lines.iter().all(|l| display_width(l) <= 30));
    }

    #[test]
    fn rulings_say_when_they_load_fail_or_are_none() {
        assert_eq!(
            ruling_lines(Some(&RulingsState::Loading), 40),
            ["", "Rulings: loading..."]
        );
        assert_eq!(
            ruling_lines(Some(&RulingsState::Loaded(Vec::new())), 40),
            ["", "No rulings"]
        );
        assert_eq!(
            ruling_lines(
                Some(&RulingsState::Failed("cannot reach Scryfall".to_owned())),
                40
            ),
            ["", "Rulings: cannot reach Scryfall"]
        );
    }

    #[test]
    fn flavor_text_is_drawn_in_italics_and_oracle_text_is_not() {
        let card = fixture("normal_glorybringer");
        let flavor = card.faces()[0].flavor_text.clone().unwrap();
        let first_word = flavor.split_whitespace().next().unwrap();
        let text = format_card(&card, &[], None, 40);
        let first = text
            .lines
            .iter()
            .position(|l| l.starts_with(first_word))
            .unwrap();
        assert_eq!(
            text.flavor,
            vec![(first..text.lines.len())],
            "{:#?}",
            text.lines
        );
        assert!(
            !text.is_flavor(first - 1),
            "the blank line before is not flavor"
        );

        let mut view = DetailView::new();
        view.show(Some(&card), &[], None, None, 40);
        let area = Rect::new(0, 0, 42, 20);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf, &Theme::default(), None);
        let italic = |row: usize| {
            let y = u16::try_from(row).unwrap();
            (1..=3).all(|x| buf[(x, y)].modifier.contains(Modifier::ITALIC))
        };
        assert_eq!(text.lines[3], "Flying, haste");
        assert!(
            !italic(0) && !italic(3),
            "name and oracle text stay upright"
        );
        assert!(italic(first) && italic(text.lines.len() - 1));
    }

    #[test]
    fn scrolling_stays_within_the_text() {
        let mut view = DetailView::new();
        let card = fixture("normal_glorybringer");
        view.show(Some(&card), &[], None, None, 20);
        let viewport = 3;
        view.apply_motion(Motion::Bottom, viewport);
        assert_eq!(view.scroll(), view.lines().len() - viewport);
        view.apply_motion(Motion::Down, viewport);
        assert_eq!(view.scroll(), view.lines().len() - viewport);
        view.apply_motion(Motion::Top, viewport);
        assert_eq!(view.scroll(), 0);
    }

    /// Cells of 10 by 20 pixels, a common terminal font.
    const CELL: Size = Size::new(10, 20);

    fn row(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
    }

    #[test]
    fn the_splits_image_takes_two_fifths_of_the_width() {
        // A 112-column screen gives the split's detail 49 inner columns, a 160-column one 70.
        let at_112 = ImageBox::beside(Size::new(49, 20), CELL).unwrap();
        assert_eq!(at_112.size, Size::new(19, 13));
        assert_eq!(at_112.placement, ImagePlacement::Beside);
        let at_160 = ImageBox::beside(Size::new(70, 40), CELL).unwrap();
        assert_eq!(at_160.size, Size::new(28, 20));
        let short = ImageBox::beside(Size::new(49, 10), CELL).unwrap();
        assert_eq!(short.size, Size::new(14, 10), "the height caps the width");
        assert_eq!(
            ImageBox::beside(Size::new(40, 20), CELL),
            None,
            "the text beside it would be 20 columns"
        );
    }

    #[test]
    fn the_full_details_image_takes_the_height_on_the_left() {
        let at_112 = ImageBox::left(Size::new(82, 24), CELL).unwrap();
        assert_eq!(at_112.size, Size::new(34, 24));
        assert_eq!(at_112.placement, ImagePlacement::Left);
        let tall = ImageBox::left(Size::new(70, 40), CELL).unwrap();
        assert_eq!(tall.size, Size::new(35, 24), "at most half the width");
        assert_eq!(
            ImageBox::left(Size::new(60, 40), CELL),
            None,
            "the text right of it would be 27 columns"
        );
    }

    fn view_with(image: ImageBox, status: ImageStatus, width: usize) -> DetailView {
        let mut view = DetailView::new();
        let card = fixture("normal_glorybringer");
        let image = DetailImage {
            area: image,
            status,
        };
        view.show(
            Some(&card),
            &formats(&["standard"]),
            None,
            Some(image),
            width,
        );
        view
    }

    #[test]
    fn text_beside_the_image_is_narrower_until_the_image_ends() {
        let image = ImageBox::beside(Size::new(49, 20), CELL).unwrap();
        let view = view_with(image, ImageStatus::Loading, 48);
        let lines = view.lines();
        assert!(
            lines[..13].iter().all(|l| display_width(l) <= 26),
            "{lines:#?}"
        );
        assert_eq!(
            lines[0],
            format!("Glorybringer{}{{3}}{{R}}{{R}}", " ".repeat(5))
        );
        assert!(
            lines[13..].iter().any(|l| display_width(l) > 26),
            "below the image the text takes the full width: {lines:#?}"
        );
        assert!(lines.iter().all(|l| display_width(l) <= 48));
    }

    #[test]
    fn the_box_says_the_image_is_loading_and_scrolls_with_the_text() {
        let image = ImageBox::beside(Size::new(49, 20), CELL).unwrap();
        let mut view = view_with(image, ImageStatus::Loading, 48);
        let area = Rect::new(0, 0, 49, 20);
        assert_eq!(view.image_area(area), Some((Rect::new(29, 0, 19, 20), 0)));
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf, &Theme::default(), None);
        assert!(
            row(&buf, 0).starts_with(" Glorybringer"),
            "{}",
            row(&buf, 0)
        );
        assert_eq!(&row(&buf, 0)[29..45], "Loading image...");
        assert!(buf[(29, 0)].modifier.contains(Modifier::DIM));
        view.apply_motion(Motion::Down, 5);
        assert_eq!(view.image_area(area), Some((Rect::new(29, 0, 19, 20), -1)));
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf, &Theme::default(), None);
        assert!(
            !row(&buf, 0).contains("Loading"),
            "scrolled off with the image's top"
        );
    }

    #[test]
    fn the_full_details_text_stands_right_of_the_image() {
        let image = ImageBox::left(Size::new(82, 24), CELL).unwrap();
        let failed = ImageStatus::Failed("cannot reach Scryfall".to_owned());
        let view = view_with(image, failed, 81);
        assert!(view.lines().iter().all(|l| display_width(l) <= 45));
        let area = Rect::new(0, 0, 82, 24);
        assert_eq!(view.image_area(area), Some((Rect::new(1, 0, 34, 24), 0)));
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf, &Theme::default(), None);
        assert_eq!(&row(&buf, 0)[1..35], "Image: cannot reach Scryfall      ");
        assert_eq!(
            &row(&buf, 0)[35..49],
            "  Glorybringer",
            "indent, image, gap"
        );
        let ready = view_with(image, ImageStatus::Ready, 81);
        let mut buf = Buffer::empty(area);
        ready.render(area, &mut buf, &Theme::default(), None);
        assert_eq!(
            &row(&buf, 0)[..37],
            " ".repeat(37),
            "a ready image is drawn by the caller"
        );
    }
}

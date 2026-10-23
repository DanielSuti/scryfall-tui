//! Loading and drawing the image of the card the detail shows.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use ratatui::Frame;
use ratatui::layout::{Rect, Size};
use ratatui_image::sliced::{SignedPosition, SlicedImage, SlicedProtocol};
use ratatui_image::{FilterType, Resize};
use tokio::time::Instant;

use crate::app::config::LoadMode;
use crate::domain::card::Card;
use crate::search::MAX_RATE_LIMIT_RETRIES;
use crate::tui::app::detail_load::Known;
use crate::tui::app::{App, AppEvent};
use crate::tui::main_panel::MainView;
use crate::tui::views::detail::{DetailImage, ImageBox, ImageStatus};

/// Downloaded images kept for the session, the oldest dropped first. A `normal` image is
/// about 80 KB.
pub const IMAGE_CACHE_ENTRIES: usize = 50;
/// Shown when the app runs without an image source, which only happens in tests.
const NO_IMAGE_SOURCE: &str = "no image source is configured";
/// How images are scaled into their box: up or down to fit, with the smooth filter Yazi uses
/// by default.
const RESIZE: Resize = Resize::Scale(Some(FilterType::Triangle));

/// A card image encoded for the terminal's graphics protocol at one size.
#[derive(Clone)]
pub struct EncodedImage(Arc<SlicedProtocol>);

impl std::fmt::Debug for EncodedImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("EncodedImage").finish_non_exhaustive()
    }
}

/// A fetch that has not delivered its image.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ImageLoad {
    /// It is running.
    Fetching,
    /// It failed, or the image could not be decoded, for this reason.
    Failed(String),
}

/// The session's card images: those downloaded, the fetches that have not delivered, and the
/// shown card's image encoded for its box.
#[derive(Debug, Default)]
pub(super) struct CardImages {
    bytes: VecDeque<(String, Arc<[u8]>)>,
    loads: HashMap<String, ImageLoad>,
    encoded: Option<(String, Size, EncodedImage)>,
    encoding: Option<(String, Size)>,
}

impl CardImages {
    fn bytes(&self, card_id: &str) -> Option<Arc<[u8]>> {
        self.bytes
            .iter()
            .find(|(id, _)| id == card_id)
            .map(|(_, bytes)| Arc::clone(bytes))
    }

    fn keep_bytes(&mut self, card_id: String, bytes: Arc<[u8]>) {
        self.bytes.retain(|(id, _)| *id != card_id);
        if self.bytes.len() == IMAGE_CACHE_ENTRIES {
            self.bytes.pop_front();
        }
        self.bytes.push_back((card_id, bytes));
    }

    fn known(&self, card_id: &str) -> Known {
        if self.bytes(card_id).is_some() {
            return Known::Kept;
        }
        match self.loads.get(card_id) {
            None => Known::Missing,
            Some(ImageLoad::Fetching) => Known::Kept,
            Some(ImageLoad::Failed(_)) => Known::Failed,
        }
    }

    /// Returns the image encoded for `card_id` at `size`, if it is.
    fn encoded(&self, card_id: &str, size: Size) -> Option<&EncodedImage> {
        self.encoded
            .as_ref()
            .filter(|(id, s, _)| id == card_id && *s == size)
            .map(|(_, _, image)| image)
    }
}

impl App {
    /// Returns the image box for `card` in a detail of `inner` cells: on the left in the full
    /// detail, beside the text in the split. Returns `None` when the terminal can't draw
    /// images, the card has none, `detail.images` doesn't show it in this view, or there is no
    /// room. In `full_detail` mode the split shows an image already downloaded.
    pub(super) fn image_box(&self, card: &Card, inner: Size) -> Option<ImageBox> {
        let picker = self.image_picker.as_ref()?;
        card.image_url()?;
        let full_detail = self.main_view == MainView::FullDetail;
        match self.config.detail.images.mode {
            LoadMode::Never => return None,
            LoadMode::FullDetail if !full_detail && self.images.bytes(card.id()).is_none() => {
                return None;
            }
            LoadMode::Auto | LoadMode::FullDetail => {}
        }
        let font = picker.font_size();
        let cell = Size::new(font.width, font.height);
        if full_detail {
            ImageBox::left(inner, cell)
        } else {
            ImageBox::beside(inner, cell)
        }
    }

    /// Prepares the image of `card`, the card the detail shows in `area`, or of no card, as
    /// `detail.images` asks, and returns the image box with what it holds. A downloaded image
    /// is encoded for the box in the background; a failure is retried when the card is shown
    /// anew.
    pub(super) fn prepare_image(
        &mut self,
        card: Option<&Card>,
        area: Option<ImageBox>,
    ) -> Option<DetailImage> {
        let target = card
            .filter(|_| area.is_some())
            .and_then(|c| Some((c.id(), c.image_url()?)));
        let known = target.map_or(Known::Missing, |(id, _)| self.images.known(id));
        let settings = &self.config.detail.images;
        let plan = self.image_schedule.schedule(
            target.map(|(id, _)| (id, id)),
            known,
            settings.mode,
            settings.pause(),
            self.main_view == MainView::FullDetail,
        );
        let ((card_id, url), area) = (target?, area?);
        if plan.forget_failure {
            self.images.loads.remove(card_id);
        }
        if plan.fetch.is_some() {
            self.fetch_image(card_id.to_owned(), url.to_owned());
        }
        let status = if let Some(ImageLoad::Failed(message)) = self.images.loads.get(card_id) {
            ImageStatus::Failed(message.clone())
        } else if self.images.encoded(card_id, area.size).is_some() {
            ImageStatus::Ready
        } else {
            if let Some(bytes) = self.images.bytes(card_id) {
                self.encode_image(card_id.to_owned(), area.size, bytes);
            }
            ImageStatus::Loading
        };
        Some(DetailImage { area, status })
    }

    /// Starts the waiting fetch once its time has come.
    pub(super) fn start_due_image(&mut self, now: Instant) {
        let Some(fetch) = self.image_schedule.take_due(now) else {
            return;
        };
        let url = self
            .selection()
            .map(|s| s.card)
            .filter(|card| card.id() == fetch.card_id)
            .and_then(Card::image_url)
            .map(str::to_owned);
        if let Some(url) = url {
            self.fetch_image(fetch.card_id, url);
        }
    }

    /// Fetches the image at `url` of the card with printing id `card_id` in the background. A
    /// rate-limited request is retried up to [`MAX_RATE_LIMIT_RETRIES`] times; the source's
    /// request gateway does the waiting.
    fn fetch_image(&mut self, card_id: String, url: String) {
        let Some(services) = self.services.clone() else {
            self.images
                .loads
                .insert(card_id, ImageLoad::Failed(NO_IMAGE_SOURCE.to_owned()));
            return;
        };
        self.images
            .loads
            .insert(card_id.clone(), ImageLoad::Fetching);
        let source = Arc::clone(&services.images);
        tokio::spawn(async move {
            let mut retries = 0;
            let result = loop {
                match source.image(&url).await {
                    Ok(bytes) => break Ok(bytes),
                    Err(err) if err.retry_after().is_some() && retries < MAX_RATE_LIMIT_RETRIES => {
                        retries += 1;
                    }
                    Err(err) => break Err(err.to_string()),
                }
            };
            // A closed channel means the app has quit; the image has no reader left.
            let _ = services
                .events
                .send(AppEvent::ImageFetched { card_id, result });
        });
    }

    /// Keeps the image fetched for `card_id`, or the reason it could not be.
    pub(super) fn handle_image_fetched(
        &mut self,
        card_id: String,
        result: Result<Vec<u8>, String>,
    ) {
        match result {
            Ok(bytes) => {
                self.images.loads.remove(&card_id);
                self.images.keep_bytes(card_id, bytes.into());
            }
            Err(message) => {
                self.images
                    .loads
                    .insert(card_id, ImageLoad::Failed(message));
            }
        }
    }

    /// Decodes `bytes` and encodes them for a box of `size` cells on a blocking thread, unless
    /// that encoding is already running. Only one result is kept: the shown card's.
    fn encode_image(&mut self, card_id: String, size: Size, bytes: Arc<[u8]>) {
        let target = (card_id, size);
        if self.images.encoding.as_ref() == Some(&target) {
            return;
        }
        let (Some(services), Some(picker)) = (self.services.clone(), self.image_picker.clone())
        else {
            return;
        };
        self.images.encoding = Some(target.clone());
        let (card_id, size) = target;
        tokio::task::spawn_blocking(move || {
            let result = image::load_from_memory(&bytes)
                .map_err(|err| format!("cannot decode the image: {err}"))
                .and_then(|image| {
                    SlicedProtocol::new_with_resize(&picker, image, size, RESIZE)
                        .map_err(|err| format!("cannot draw the image: {err}"))
                })
                .map(|protocol| EncodedImage(Arc::new(protocol)));
            // A closed channel means the app has quit; the image has no reader left.
            let _ = services.events.send(AppEvent::ImageEncoded {
                card_id,
                size,
                result,
            });
        });
    }

    /// Keeps the image encoded for `card_id` at `size` if the detail still waits for it.
    /// An image that can't be decoded is dropped and fails, so it is fetched again when its
    /// card is shown anew.
    pub(super) fn handle_image_encoded(
        &mut self,
        card_id: String,
        size: Size,
        result: Result<EncodedImage, String>,
    ) {
        if self.images.encoding.as_ref() == Some(&(card_id.clone(), size)) {
            self.images.encoding = None;
        }
        match result {
            Ok(image) => {
                let shown = self.selection().is_some_and(|s| s.card.id() == card_id);
                if shown {
                    self.images.encoded = Some((card_id, size, image));
                }
            }
            Err(message) => {
                self.images.bytes.retain(|(id, _)| *id != card_id);
                self.images
                    .loads
                    .insert(card_id, ImageLoad::Failed(message));
            }
        }
    }

    /// Draws the shown card's image into the detail drawn in `inner`, unless a popup covers
    /// the screen: sixel and iTerm2 images would show through it, so Yazi hides its preview
    /// under popups as well.
    pub(super) fn render_card_image(&self, frame: &mut Frame, inner: Rect, covered: bool) {
        if covered {
            return;
        }
        let Some(card) = self.selection().map(|s| s.card) else {
            return;
        };
        let Some(image) = self.detail.image() else {
            return;
        };
        let Some(encoded) = self.images.encoded(card.id(), image.area.size) else {
            return;
        };
        let Some((rect, top)) = self.detail.image_area(inner) else {
            return;
        };
        let position = SignedPosition::from((0, top));
        frame.render_widget(SlicedImage::new(&encoded.0, position), rect);
    }
}

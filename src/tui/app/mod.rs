//! Application state and event handling.

mod commands;
mod history;
pub mod registrar;
mod search;
mod sort;
#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{KeyEvent, KeyEventKind};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Clear, Paragraph, Widget};
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::Instant;

use crate::app::command::{CommandId, CommandRegistry, Scope};
use crate::app::config::Config;
use crate::app::history::History;
use crate::app::results_cache::ResultsCache;
use crate::app::selection::Selection;
use crate::domain::search_results::{SearchPage, SearchResults};
use crate::search::{CardSearch, SearchRequest, SortOrder};
use crate::tui::keymap::bindings::Keymap;
use crate::tui::keymap::dispatcher::{Dispatch, Dispatcher};
use crate::tui::keymap::mode::Mode;
use crate::tui::keymap::sequence::Key;
use crate::tui::layout::{
    DETAIL_SCOPE, FULL_DETAIL_SCOPE, HISTORY_SCOPE, PROMPT_SCOPE, Panel, RESULTS_SCOPE,
    SEARCH_BAR_SCOPE, SPLIT_SCOPE, ScreenAreas, panel_block,
};
use crate::tui::main_panel::{Back, MainView};
use crate::tui::picker::picker::{Picker, PickerOutcome};
use crate::tui::views::detail::DetailView;
use crate::tui::views::history::HistoryView;
use crate::tui::views::results::{ResultsView, load_summary};
use crate::tui::views::search_bar::SearchBar;
use crate::tui::widgets::item_list::{Motion, MotionOutcome};
use crate::tui::widgets::pane_search::Pane;
use crate::tui::widgets::query_input::QueryInput;
use crate::tui::widgets::status_line::{StatusLine, StatusMessage};
use crate::tui::widgets::which_key::{WhichKey, WhichKeyEntry, WhichKeyPopup, pair_case_twins};

pub use commands::*;
pub use search::{NEW_SEARCH, RUN_SEARCH};

/// The key that `<leader>` stands for.
pub const LEADER: Key = Key::char(' ');

/// Progress of a background search, delivered as an [`AppEvent::Search`].
#[derive(Debug, Clone)]
pub enum SearchUpdate {
    /// A page arrived.
    Page(SearchPage),
    /// The source rate-limited a request and it is retried after this pause.
    RateLimited(Duration),
    /// A request failed with this message.
    Failed(String),
    /// The batch ended.
    BatchDone,
}

/// An input to the application, from the terminal, a timer or a background task.
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// A key event from the terminal.
    Key(KeyEvent),
    /// A deadline returned by [`App::next_deadline`] has passed.
    Tick,
    /// The terminal now has this many columns and rows.
    Resize(u16, u16),
    /// Progress of the background search for `query`.
    Search {
        /// The request the update belongs to.
        request: SearchRequest,
        /// What happened.
        update: SearchUpdate,
    },
}

/// What the application needs from outside: a search source, and a channel on which
/// background tasks send their [`AppEvent`]s back to the event loop.
#[derive(Clone)]
pub struct Services {
    /// The card search.
    pub search: Arc<dyn CardSearch>,
    /// Where background tasks send their events.
    pub events: UnboundedSender<AppEvent>,
}

impl std::fmt::Debug for Services {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Services").finish_non_exhaustive()
    }
}

/// The whole application state. Rendering is a pure function of it.
#[derive(Debug)]
pub struct App {
    config: Config,
    services: Option<Services>,
    mode: Mode,
    quit: bool,
    commands: CommandRegistry<App>,
    keymap: Keymap,
    dispatcher: Dispatcher,
    which_key: WhichKey,
    area: Rect,
    focus: Panel,
    history_overlay: bool,
    main_view: MainView,
    search_bar: SearchBar,
    results: ResultsView,
    detail: DetailView,
    format_warning: Option<String>,
    formats_checked: bool,
    history: History,
    history_path: Option<std::path::PathBuf>,
    history_view: HistoryView,
    cache: ResultsCache,
    batches_running: usize,
    sort_order: SortOrder,
    prompt: Option<QueryInput>,
    last_pattern: Option<String>,
    message: Option<StatusMessage>,
    picker: Option<Picker<CommandId>>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    /// Creates the application with the default configuration and no search source.
    ///
    /// # Panics
    /// See [`App::with_config`].
    pub fn new() -> Self {
        Self::with_config(Config::default())
    }

    /// Creates the application in normal mode with all commands and keys registered, and no
    /// search source.
    ///
    /// # Panics
    /// Panics if two built-in commands share an id or two built-in key bindings conflict. Both
    /// are programming errors caught by the tests.
    pub fn with_config(config: Config) -> Self {
        let max_history = config.history.max_entries;
        let mut app = Self {
            config,
            services: None,
            mode: Mode::Normal,
            quit: false,
            commands: CommandRegistry::new(),
            keymap: Keymap::new(LEADER),
            dispatcher: Dispatcher::new(),
            which_key: WhichKey::default(),
            area: Rect::default(),
            focus: Panel::Main,
            history_overlay: false,
            main_view: MainView::Results,
            search_bar: SearchBar::new(),
            results: ResultsView::new(),
            detail: DetailView::new(),
            format_warning: None,
            formats_checked: false,
            history: History::new(max_history),
            history_path: None,
            history_view: HistoryView::new(),
            cache: ResultsCache::new(),
            batches_running: 0,
            sort_order: SortOrder::default(),
            prompt: None,
            last_pattern: None,
            message: None,
            picker: None,
        };
        commands::register_all(&mut app.commands, &mut app.keymap)
            .expect("built-in commands and keys are consistent");
        app
    }

    /// Creates the application with `config` and `services`.
    ///
    /// # Panics
    /// See [`App::with_config`].
    pub fn with_services(config: Config, services: Services) -> Self {
        let mut app = Self::with_config(config);
        app.services = Some(services);
        app
    }

    /// Returns the configuration the app was started with.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Returns true once the application has been asked to quit.
    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// Returns the current mode.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Returns the command registry.
    pub fn commands(&self) -> &CommandRegistry<App> {
        &self.commands
    }

    /// Returns the keymap.
    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    /// Returns the query in the search bar.
    pub fn search_query(&self) -> &str {
        self.search_bar.query()
    }

    /// Returns the results shown in the main panel, or `None` before the first search.
    pub fn displayed_results(&self) -> Option<&SearchResults> {
        self.cache.get(self.results.request()?)
    }

    /// Returns the index of the results row under the cursor.
    pub fn results_cursor(&self) -> usize {
        self.results.list().cursor()
    }

    /// Returns the card under the results cursor, or `None` when no results are shown.
    pub fn selection(&self) -> Option<Selection<'_>> {
        let results = self.displayed_results()?;
        let index = self.results.list().cursor();
        Some(Selection {
            card: results.cards().get(index)?,
            index,
            query: results.query(),
        })
    }

    /// Returns the lines of the detail view.
    pub fn detail_lines(&self) -> &[String] {
        self.detail.lines()
    }

    /// Returns the problem found in `detail.formats`, if any.
    pub fn format_warning(&self) -> Option<&str> {
        self.format_warning.as_deref()
    }

    /// Returns true while a background search batch is running.
    pub fn is_loading(&self) -> bool {
        self.batches_running > 0
    }

    /// Applies one event to the state.
    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Key(key) => self.handle_key(key),
            AppEvent::Tick => self.which_key.tick(Instant::now()),
            AppEvent::Resize(width, height) => self.area = Rect::new(0, 0, width, height),
            AppEvent::Search { request, update } => self.handle_search_update(&request, update),
        }
        self.sync_detail();
    }

    /// Returns the rectangle the detail is drawn in: the split's detail panel, or the main
    /// panel in the full detail.
    fn detail_rect(&self, areas: &ScreenAreas) -> Option<Rect> {
        match self.main_view {
            MainView::Results => None,
            MainView::Split => areas.detail,
            MainView::FullDetail => Some(areas.main),
        }
    }

    /// Formats the selected card for the detail's current width.
    fn sync_detail(&mut self) {
        let Some(rect) = self.detail_rect(&self.areas()) else {
            return;
        };
        let width = usize::from(rect.width.saturating_sub(3));
        let card = self.selection().map(|s| s.card.clone());
        let formats = self.config.detail.formats.clone();
        self.detail.show(card.as_ref(), &formats, width);
    }

    /// Checks `detail.formats` against the formats the first loaded card reports.
    fn check_formats(&mut self, card: &crate::domain::card::Card) {
        if self.formats_checked {
            return;
        }
        self.formats_checked = true;
        let reported: Vec<&str> = card.legalities().formats().collect();
        let unknown: Vec<String> = self
            .config
            .detail
            .formats
            .iter()
            .filter(|f| !reported.contains(&f.as_str()))
            .map(|f| format!("`{f}`"))
            .collect();
        if !unknown.is_empty() {
            self.format_warning = Some(format!(
                "detail.formats: Scryfall reports no format {}",
                unknown.join(", ")
            ));
        }
    }

    /// Returns the focused panel.
    pub fn focus(&self) -> Panel {
        self.focus
    }

    /// Returns the screen areas for the last known terminal size.
    pub fn areas(&self) -> ScreenAreas {
        self.areas_for(self.area)
    }

    fn areas_for(&self, area: Rect) -> ScreenAreas {
        ScreenAreas::compute(
            area,
            self.main_view == MainView::Split,
            self.history_overlay,
        )
    }

    /// Returns what the main panel shows.
    pub fn main_view(&self) -> MainView {
        self.main_view
    }

    fn show_main(&mut self, view: MainView) {
        self.main_view = view;
        if view != MainView::Split && self.focus == Panel::Detail {
            self.focus = Panel::Main;
        }
    }

    fn main_back(&mut self) {
        match self.main_view.on_back() {
            Back::To(view) => self.show_main(view),
            Back::Quit => self.quit = true,
        }
    }

    fn set_focus(&mut self, panel: Panel) {
        if panel != Panel::History {
            self.history_overlay = false;
        }
        self.focus = panel;
    }

    /// Returns when the app next needs an [`AppEvent::Tick`], or `None` when it waits only
    /// for other events.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.which_key.deadline()
    }

    /// Returns the entries of the which-key popup, with case twins on one line, or an empty
    /// list when it is hidden.
    pub fn which_key_entries(&self) -> Vec<WhichKeyEntry> {
        if !self.which_key.visible() {
            return Vec::new();
        }
        let entries = self
            .which_key
            .continuations()
            .iter()
            .map(|c| WhichKeyEntry {
                keys: c.rest.to_string(),
                description: self
                    .commands
                    .get(c.command)
                    .map_or_else(|| c.command.to_string(), |cmd| cmd.description.to_string()),
            })
            .collect();
        pair_case_twins(entries)
    }

    /// Returns the title of the which-key popup, if the pending prefix has one.
    pub fn which_key_title(&self) -> Option<&'static str> {
        self.which_key.title()
    }

    /// Runs the command with `id`. Unknown ids are ignored, because every bound id is
    /// registered at start-up.
    pub fn run_command(&mut self, id: CommandId) {
        if let Some(run) = self
            .commands
            .get(id)
            .map(|command| Arc::clone(&command.run))
        {
            run(self);
        }
    }

    /// Returns the message shown in the status line, if any.
    pub fn message(&self) -> Option<&StatusMessage> {
        self.message.as_ref()
    }

    /// Returns the scope of the focused panel, or of the `/` prompt while it is open.
    pub fn focused_scope(&self) -> Scope {
        if self.prompt.is_some() {
            return PROMPT_SCOPE;
        }
        match self.focus {
            Panel::SearchBar => SEARCH_BAR_SCOPE,
            Panel::History => HISTORY_SCOPE,
            Panel::Main => match self.main_view {
                MainView::Results => RESULTS_SCOPE,
                MainView::Split => SPLIT_SCOPE,
                MainView::FullDetail => FULL_DETAIL_SCOPE,
            },
            Panel::Detail => DETAIL_SCOPE,
        }
    }

    fn active_scopes(&self) -> Vec<Scope> {
        vec![self.focused_scope(), Scope::GLOBAL]
    }

    /// Returns the status-line hints of `scope`: each command with a hint label and a
    /// normal-mode binding in that scope, in registration order, formatted as its first such
    /// key and the label.
    pub fn hints_for(&self, scope: Scope) -> Vec<String> {
        self.commands
            .iter()
            .filter_map(|command| {
                let hint = command.hint?;
                let binding = self
                    .keymap
                    .bindings_for(command.id)
                    .find(|b| b.mode == Mode::Normal && b.scope == scope)?;
                Some(format!(
                    "{} {hint}",
                    hint_keys(&binding.sequence.to_string())
                ))
            })
            .collect()
    }

    fn handle_key(&mut self, event: KeyEvent) {
        if event.kind == KeyEventKind::Release {
            return;
        }
        let Some(key) = Key::from_event(&event) else {
            return;
        };
        self.message = None;
        if let Some(picker) = self.picker.as_mut() {
            match picker.handle_key(event) {
                PickerOutcome::Open => {}
                PickerOutcome::Cancelled => self.close_picker(),
                PickerOutcome::Selected(id) => {
                    self.close_picker();
                    self.run_command(id);
                }
            }
            return;
        }
        let scopes = self.active_scopes();
        match self.dispatcher.feed(&self.keymap, self.mode, &scopes, key) {
            Dispatch::Matched(id) => {
                self.which_key.clear();
                self.run_command(id);
            }
            Dispatch::Pending(continuations) => {
                let group = self
                    .keymap
                    .group_for(self.mode, &scopes, self.dispatcher.pending());
                let (title, instant) = group.map_or((None, false), |g| (Some(g.title), g.instant));
                self.which_key
                    .start(continuations, Instant::now(), title, instant);
            }
            Dispatch::NoMatch(_) => {
                self.which_key.clear();
                if self.mode == Mode::Insert {
                    match self.prompt.as_mut() {
                        Some(prompt) => {
                            prompt.handle_key(event);
                        }
                        None => self.search_bar.handle_key(event),
                    }
                }
            }
        }
    }

    /// Returns the number of text rows inside `panel` for the last known terminal size.
    fn viewport(&self, panel: Panel) -> usize {
        let areas = self.areas();
        let rect = match panel {
            Panel::SearchBar => Some(areas.search),
            Panel::History => areas.history.or(areas.overlay),
            Panel::Main => Some(areas.main),
            Panel::Detail => areas.detail,
        };
        rect.map_or(0, |r| usize::from(r.height.saturating_sub(2)))
    }

    /// Returns the focused panel's searchable, navigable content and its viewport height, or
    /// `None` for a panel without such content.
    fn focused_pane(&mut self) -> Option<(&mut dyn Pane, usize)> {
        let viewport = self.viewport(self.focus);
        match (self.focus, self.main_view) {
            (Panel::Main, MainView::Results | MainView::Split) => {
                Some((&mut self.results, viewport))
            }
            (Panel::Main, MainView::FullDetail) | (Panel::Detail, _) => {
                Some((&mut self.detail, viewport))
            }
            (Panel::History, _) => Some((&mut self.history_view, viewport)),
            (Panel::SearchBar, _) => None,
        }
    }

    fn motion(&mut self, motion: Motion) -> MotionOutcome {
        let outcome = self
            .focused_pane()
            .map(|(pane, viewport)| pane.apply_motion(motion, viewport))
            .unwrap_or_default();
        let on_results = self.focus == Panel::Main && self.main_view != MainView::FullDetail;
        if outcome.end_reached && on_results {
            self.load_more();
        }
        outcome
    }

    fn close_picker(&mut self) {
        self.picker = None;
        self.mode = Mode::Normal;
    }

    /// Returns the open picker, if any.
    pub fn picker(&self) -> Option<&Picker<CommandId>> {
        self.picker.as_ref()
    }

    /// Switches to insert mode, so unbound keys edit the focused text input.
    pub fn enter_insert_mode(&mut self) {
        self.mode = Mode::Insert;
        self.dispatcher.reset();
    }

    /// Draws the whole screen.
    pub fn render(&self, frame: &mut Frame) {
        let areas = self.areas_for(frame.area());
        let focused = |panel| self.focus == panel;

        self.search_bar.render(
            frame,
            areas.search,
            self.title(Panel::SearchBar),
            &self.config.search.base,
            focused(Panel::SearchBar),
            focused(Panel::SearchBar) && self.mode == Mode::Insert && self.prompt.is_none(),
        );
        if let Some(history) = areas.history {
            self.render_history(frame, history);
        }
        let block = panel_block(self.title(Panel::Main), focused(Panel::Main));
        let inner = block.inner(areas.main);
        frame.render_widget(block, areas.main);
        if self.main_view == MainView::FullDetail {
            self.detail.render(inner, frame.buffer_mut());
        } else {
            self.results
                .render(inner, frame.buffer_mut(), focused(Panel::Main));
        }
        if let Some(detail) = areas.detail {
            let block = panel_block(self.title(Panel::Detail), focused(Panel::Detail));
            let inner = block.inner(detail);
            frame.render_widget(block, detail);
            self.detail.render(inner, frame.buffer_mut());
        }
        if let Some(overlay) = areas.overlay {
            frame.render_widget(Clear, overlay);
            self.render_history(frame, overlay);
        }

        if let Some(prompt) = &self.prompt {
            let status = areas.status;
            frame.render_widget(Paragraph::new("/"), status);
            let input = Rect {
                x: status.x + 1,
                width: status.width.saturating_sub(1),
                ..status
            };
            prompt.render(frame, input, true);
        } else {
            let hints = self.hints_for(self.focused_scope());
            let mut status = StatusLine::new(self.mode).section(self.title(self.focus));
            let standing = self.standing_message();
            if let Some(results) = self.displayed_results() {
                status = status.section(load_summary(results));
                if let Some(request) = self.results.request() {
                    status = status.section(request.order.summary());
                }
            }
            frame.render_widget(
                status
                    .message(self.message.as_ref().or(standing.as_ref()))
                    .hints(&hints),
                areas.status,
            );
        }
        if let Some(picker) = &self.picker {
            picker.render(frame, frame.area());
        }
        let entries = self.which_key_entries();
        if !entries.is_empty() {
            let body = Rect {
                height: areas.status.y.saturating_sub(areas.search.y),
                ..frame.area()
            };
            WhichKeyPopup::new(&entries)
                .title(self.which_key.title())
                .render(body, frame.buffer_mut());
        }
    }

    fn render_history(&self, frame: &mut Frame, area: Rect) {
        let focused = self.focus == Panel::History;
        let block = panel_block(self.title(Panel::History), focused);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        self.history_view.render(inner, frame.buffer_mut(), focused);
    }

    /// The message shown while no event message is set: a problem in `detail.formats`, then
    /// the failure of the displayed search, then Scryfall's warnings about its query.
    fn standing_message(&self) -> Option<StatusMessage> {
        if let Some(warning) = &self.format_warning {
            return Some(StatusMessage::error(warning.clone()));
        }
        let results = self.displayed_results()?;
        if let crate::domain::search_results::LoadState::Failed(reason) = results.state() {
            return Some(StatusMessage::error(reason.clone()));
        }
        (!results.warnings().is_empty()).then(|| StatusMessage::info(results.warnings().join(" ")))
    }

    fn title(&self, panel: Panel) -> String {
        let name = match panel {
            Panel::SearchBar => "Search",
            Panel::History => "History",
            Panel::Main if self.main_view == MainView::FullDetail => "Detail",
            Panel::Main => "Results",
            Panel::Detail => "Detail",
        };
        format!("[{}] {name}", panel.jump_key())
    }
}

/// Shortens key notation for the hint line: `<CR>` becomes `⏎`.
fn hint_keys(notation: &str) -> String {
    notation.replace("<CR>", "⏎")
}

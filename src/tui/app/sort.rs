//! Choosing the order of the results.

use crate::app::command::CommandId;
use crate::search::SortOrder;
use crate::tui::app::registrar::{Registrar, RegistrationError, command};
use crate::tui::app::{App, OpenPicker};
use crate::tui::keymap::mode::Mode;
use crate::tui::layout::{RESULTS_SCOPE, SPLIT_SCOPE};
use crate::tui::picker::picker::Picker;

/// Opens the picker that chooses the sort order of the results.
pub const SORT: CommandId = CommandId("results.sort");

pub(super) fn register(reg: &mut Registrar<'_>) -> Result<(), RegistrationError> {
    reg.add(
        Mode::Normal,
        &["s"],
        command(
            SORT,
            RESULTS_SCOPE,
            "Choose the sort order",
            Some("sort"),
            App::open_sort_picker,
        ),
    )?;
    reg.bind(Mode::Normal, SPLIT_SCOPE, &["s"], SORT)
}

impl App {
    /// Returns the order used for searches in this session.
    pub fn sort_order(&self) -> SortOrder {
        self.sort_order
    }

    fn open_sort_picker(&mut self) {
        let items = SortOrder::ALL
            .iter()
            .map(|order| (order.label().to_owned(), *order))
            .collect();
        let mut picker = Picker::new("Sort by", items);
        let current = self.sort_order;
        picker.select_where(|order| *order == current);
        self.picker = Some(OpenPicker::Sort(picker));
        self.enter_insert_mode();
    }

    /// Makes `order` the session's sort order and shows the displayed query in it, from the
    /// cache when it holds those results.
    pub(super) fn apply_sort(&mut self, order: SortOrder) {
        if order == self.sort_order {
            return;
        }
        self.sort_order = order;
        if let Some(query) = self.results.request().map(|r| r.query.clone()) {
            self.run_query(&query);
        }
    }
}

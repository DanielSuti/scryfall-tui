//! States of the main panel and the transitions between them.

/// What the main panel shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MainView {
    /// The results list only.
    #[default]
    Results,
    /// The results with the detail of the card under the cursor next to them.
    Split,
    /// The detail of one card, replacing the results.
    FullDetail,
}

/// The outcome of stepping back from a view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Back {
    /// Show this view.
    To(MainView),
    /// Nothing to step back to: quit the application.
    Quit,
}

impl MainView {
    /// The view after `d`: the full detail.
    pub fn on_detail(self) -> MainView {
        MainView::FullDetail
    }

    /// The view after `<CR>`: the split.
    pub fn on_split(self) -> MainView {
        MainView::Split
    }

    /// The view after `q`: one level back, or quit from the results.
    pub fn on_back(self) -> Back {
        match self {
            MainView::Results => Back::Quit,
            MainView::Split | MainView::FullDetail => Back::To(MainView::Results),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use MainView::{FullDetail, Results, Split};

    #[test]
    fn every_transition_matches_the_table() {
        assert_eq!(Results.on_detail(), FullDetail);
        assert_eq!(Results.on_split(), Split);
        assert_eq!(Results.on_back(), Back::Quit);
        assert_eq!(Split.on_detail(), FullDetail);
        assert_eq!(Split.on_split(), Split, "no-op");
        assert_eq!(Split.on_back(), Back::To(Results));
        assert_eq!(FullDetail.on_detail(), FullDetail, "no-op");
        assert_eq!(FullDetail.on_split(), Split);
        assert_eq!(FullDetail.on_back(), Back::To(Results));
    }
}

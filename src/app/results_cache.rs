//! Search results kept for the session, keyed by the exact query text.

use std::collections::HashMap;

use crate::domain::search_results::{LoadState, SearchResults};
use crate::search::BatchStart;

/// The results of every query run in this session.
#[derive(Debug, Clone, Default)]
pub struct ResultsCache {
    entries: HashMap<String, SearchResults>,
}

impl ResultsCache {
    /// Creates an empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Prepares `query` for display. Returns `None` when its results are cached, so no request
    /// is needed. Returns the batch to load when the query is new or its last attempt failed;
    /// the entry then starts over empty.
    pub fn begin(&mut self, query: &str) -> Option<BatchStart> {
        if let Some(results) = self.entries.get(query)
            && !matches!(results.state(), LoadState::Failed(_))
        {
            return None;
        }
        self.entries
            .insert(query.to_owned(), SearchResults::new(query));
        Some(BatchStart::Query(query.to_owned()))
    }

    /// Starts the next batch of `query` when its loading is paused, and returns where it
    /// starts. Returns `None` for an unknown query, while a batch is loading, after the last
    /// page and after a failure.
    pub fn resume(&mut self, query: &str) -> Option<BatchStart> {
        self.entries.get_mut(query)?.resume().map(BatchStart::Next)
    }

    /// Returns the results of `query`, if cached.
    pub fn get(&self, query: &str) -> Option<&SearchResults> {
        self.entries.get(query)
    }

    /// Returns the results of `query` for updating, if cached.
    pub fn get_mut(&mut self, query: &str) -> Option<&mut SearchResults> {
        self.entries.get_mut(query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::search_results::{PageToken, SearchPage};

    fn finish_with_more(cache: &mut ResultsCache, query: &str) {
        let results = cache.get_mut(query).unwrap();
        results.apply(SearchPage {
            next: Some(PageToken(format!("{query}-p2"))),
            ..SearchPage::default()
        });
        results.finish_batch();
    }

    #[test]
    fn a_cached_query_needs_no_request() {
        let mut cache = ResultsCache::new();
        assert_eq!(
            cache.begin("t:dragon"),
            Some(BatchStart::Query("t:dragon".into()))
        );
        finish_with_more(&mut cache, "t:dragon");
        assert_eq!(cache.begin("t:dragon"), None);
        assert_eq!(
            cache.get("t:dragon").unwrap().pages_loaded(),
            1,
            "entry is kept"
        );
    }

    #[test]
    fn a_different_text_misses() {
        let mut cache = ResultsCache::new();
        cache.begin("t:dragon");
        assert_eq!(
            cache.begin("t:dragon "),
            Some(BatchStart::Query("t:dragon ".into()))
        );
        assert_eq!(
            cache.begin("T:dragon"),
            Some(BatchStart::Query("T:dragon".into()))
        );
    }

    #[test]
    fn a_further_batch_resumes_from_the_stored_token() {
        let mut cache = ResultsCache::new();
        cache.begin("o:draw");
        assert_eq!(cache.resume("o:draw"), None, "still loading");
        finish_with_more(&mut cache, "o:draw");
        assert_eq!(cache.begin("o:draw"), None);
        assert_eq!(
            cache.resume("o:draw"),
            Some(BatchStart::Next(PageToken("o:draw-p2".into())))
        );
        assert_eq!(cache.resume("unknown"), None);
    }

    #[test]
    fn a_failed_query_is_loaded_again() {
        let mut cache = ResultsCache::new();
        cache.begin("o:draw");
        cache.get_mut("o:draw").unwrap().fail("offline");
        assert_eq!(
            cache.begin("o:draw"),
            Some(BatchStart::Query("o:draw".into()))
        );
        assert_eq!(cache.get("o:draw").unwrap().state(), &LoadState::Loading);
    }
}

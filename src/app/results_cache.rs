//! Search results kept for the session, keyed by the exact query text and sort order.

use std::collections::HashMap;

use crate::domain::search_results::{LoadState, SearchResults};
use crate::search::{BatchStart, SearchRequest};

/// The results of every request run in this session.
#[derive(Debug, Clone, Default)]
pub struct ResultsCache {
    entries: HashMap<SearchRequest, SearchResults>,
}

impl ResultsCache {
    /// Creates an empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Prepares `request` for display. Returns `None` when its results are cached, so no
    /// request is needed. Returns the batch to load when the request is new or its last attempt
    /// failed; the entry then starts over empty.
    pub fn begin(&mut self, request: &SearchRequest) -> Option<BatchStart> {
        if let Some(results) = self.entries.get(request)
            && !matches!(results.state(), LoadState::Failed(_))
        {
            return None;
        }
        self.entries
            .insert(request.clone(), SearchResults::new(request.query.clone()));
        Some(BatchStart::Query(request.clone()))
    }

    /// Starts the next batch of `request` when its loading is paused, and returns where it
    /// starts. Returns `None` for an unknown request, while a batch is loading, after the last
    /// page and after a failure.
    pub fn resume(&mut self, request: &SearchRequest) -> Option<BatchStart> {
        self.entries
            .get_mut(request)?
            .resume()
            .map(BatchStart::Next)
    }

    /// Returns the results of `request`, if cached.
    pub fn get(&self, request: &SearchRequest) -> Option<&SearchResults> {
        self.entries.get(request)
    }

    /// Returns the results of `request` for updating, if cached.
    pub fn get_mut(&mut self, request: &SearchRequest) -> Option<&mut SearchResults> {
        self.entries.get_mut(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::search_results::{PageToken, SearchPage};
    use crate::search::{SortField, SortOrder};

    fn by_name(query: &str) -> SearchRequest {
        SearchRequest::new(query, SortOrder::default())
    }

    fn finish_with_more(cache: &mut ResultsCache, request: &SearchRequest) {
        let results = cache.get_mut(request).unwrap();
        results.apply(SearchPage {
            next: Some(PageToken(format!("{}-p2", request.query))),
            ..SearchPage::default()
        });
        results.finish_batch();
    }

    #[test]
    fn a_cached_query_needs_no_request() {
        let mut cache = ResultsCache::new();
        let dragon = by_name("t:dragon");
        assert_eq!(
            cache.begin(&dragon),
            Some(BatchStart::Query(dragon.clone()))
        );
        finish_with_more(&mut cache, &dragon);
        assert_eq!(cache.begin(&dragon), None);
        assert_eq!(
            cache.get(&dragon).unwrap().pages_loaded(),
            1,
            "entry is kept"
        );
    }

    #[test]
    fn a_different_text_misses() {
        let mut cache = ResultsCache::new();
        cache.begin(&by_name("t:dragon"));
        for other in ["t:dragon ", "T:dragon"] {
            let request = by_name(other);
            assert_eq!(
                cache.begin(&request),
                Some(BatchStart::Query(request.clone()))
            );
        }
    }

    #[test]
    fn another_order_misses_and_switching_back_hits() {
        let mut cache = ResultsCache::new();
        let by_name = by_name("t:dragon");
        let by_release = SearchRequest::new("t:dragon", SortOrder::new(SortField::Released, false));
        cache.begin(&by_name);
        finish_with_more(&mut cache, &by_name);
        assert_eq!(
            cache.begin(&by_release),
            Some(BatchStart::Query(by_release.clone()))
        );
        assert_eq!(cache.begin(&by_name), None);
    }

    #[test]
    fn a_further_batch_resumes_from_the_stored_token() {
        let mut cache = ResultsCache::new();
        let draw = by_name("o:draw");
        cache.begin(&draw);
        assert_eq!(cache.resume(&draw), None, "still loading");
        finish_with_more(&mut cache, &draw);
        assert_eq!(cache.begin(&draw), None);
        assert_eq!(
            cache.resume(&draw),
            Some(BatchStart::Next(PageToken("o:draw-p2".into())))
        );
        assert_eq!(cache.resume(&by_name("unknown")), None);
    }

    #[test]
    fn a_failed_query_is_loaded_again() {
        let mut cache = ResultsCache::new();
        let draw = by_name("o:draw");
        cache.begin(&draw);
        cache.get_mut(&draw).unwrap().fail("offline");
        assert_eq!(cache.begin(&draw), Some(BatchStart::Query(draw.clone())));
        assert_eq!(cache.get(&draw).unwrap().state(), &LoadState::Loading);
    }
}

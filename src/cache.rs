//! In-memory async segment cache.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::segments::SegmentContent;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub segment_id: String,
    pub source: String,
    pub config_generation: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AsyncValue {
    Loading,
    Ready(Option<SegmentContent>),
    Stale(Option<SegmentContent>),
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum CachedValue {
    Success(Option<SegmentContent>),
    Failure,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CacheEntry {
    value: CachedValue,
    collected_at: Instant,
}

#[derive(Debug)]
pub struct SegmentCache {
    entries: HashMap<CacheKey, CacheEntry>,
    inflight: HashSet<CacheKey>,
    capacity: usize,
}

impl CacheKey {
    pub fn new(
        segment_id: impl Into<String>,
        source: impl Into<String>,
        config_generation: u64,
    ) -> Self {
        Self {
            segment_id: segment_id.into(),
            source: source.into(),
            config_generation,
        }
    }
}

impl SegmentCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(capacity),
            inflight: HashSet::new(),
            capacity,
        }
    }

    pub fn lookup(&self, key: &CacheKey, now: Instant, ttl: Duration) -> AsyncValue {
        let Some(entry) = self.entries.get(key) else {
            return AsyncValue::Loading;
        };

        match &entry.value {
            CachedValue::Success(value) if is_fresh(entry.collected_at, now, ttl) => {
                AsyncValue::Ready(value.clone())
            }
            CachedValue::Success(value) => AsyncValue::Stale(value.clone()),
            CachedValue::Failure => AsyncValue::Failed,
        }
    }

    pub fn needs_refresh(&self, key: &CacheKey, now: Instant, ttl: Duration) -> bool {
        if self.inflight.contains(key) {
            return false;
        }

        match self.entries.get(key) {
            None => true,
            Some(entry) => !is_fresh(entry.collected_at, now, ttl),
        }
    }

    pub fn mark_inflight(&mut self, key: CacheKey) -> bool {
        self.inflight.insert(key)
    }

    pub fn is_inflight(&self, key: &CacheKey) -> bool {
        self.inflight.contains(key)
    }

    pub fn complete_success(
        &mut self,
        key: CacheKey,
        value: Option<SegmentContent>,
        collected_at: Instant,
    ) {
        self.inflight.remove(&key);
        self.insert(
            key,
            CacheEntry {
                value: CachedValue::Success(value),
                collected_at,
            },
        );
    }

    pub fn complete_failure(&mut self, key: CacheKey, collected_at: Instant) {
        self.inflight.remove(&key);

        if matches!(
            self.entries.get(&key).map(|entry| &entry.value),
            Some(CachedValue::Success(_))
        ) {
            return;
        }

        self.insert(
            key,
            CacheEntry {
                value: CachedValue::Failure,
                collected_at,
            },
        );
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn insert(&mut self, key: CacheKey, entry: CacheEntry) {
        if self.capacity == 0 {
            return;
        }

        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.evict_oldest();
        }

        self.entries.insert(key, entry);
    }

    fn evict_oldest(&mut self) {
        let Some(oldest_key) = self
            .entries
            .iter()
            .min_by_key(|(_key, entry)| entry.collected_at)
            .map(|(key, _entry)| key.clone())
        else {
            return;
        };

        self.entries.remove(&oldest_key);
    }
}

fn is_fresh(collected_at: Instant, now: Instant, ttl: Duration) -> bool {
    now.duration_since(collected_at) < ttl
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segments::Style;

    fn segment(text: &str) -> SegmentContent {
        SegmentContent::new("git_status", text, Style::default())
    }

    fn key(source: &str) -> CacheKey {
        CacheKey::new("git_status", source, 1)
    }

    #[test]
    fn lookup_returns_loading_for_missing_entry() {
        let cache = SegmentCache::new(2);
        let now = Instant::now();

        assert_eq!(
            cache.lookup(&key("/repo"), now, Duration::from_secs(1)),
            AsyncValue::Loading
        );
    }

    #[test]
    fn lookup_returns_ready_when_entry_is_fresh() {
        let mut cache = SegmentCache::new(2);
        let now = Instant::now();
        let key = key("/repo");
        cache.complete_success(key.clone(), Some(segment("*")), now);

        assert_eq!(
            cache.lookup(
                &key,
                now + Duration::from_millis(999),
                Duration::from_secs(1)
            ),
            AsyncValue::Ready(Some(segment("*")))
        );
    }

    #[test]
    fn lookup_returns_stale_when_entry_exceeds_ttl() {
        let mut cache = SegmentCache::new(2);
        let now = Instant::now();
        let key = key("/repo");
        cache.complete_success(key.clone(), Some(segment("*")), now);

        assert_eq!(
            cache.lookup(&key, now + Duration::from_secs(1), Duration::from_secs(1)),
            AsyncValue::Stale(Some(segment("*")))
        );
    }

    #[test]
    fn failed_refresh_keeps_existing_success_as_stale() {
        let mut cache = SegmentCache::new(2);
        let now = Instant::now();
        let key = key("/repo");
        cache.complete_success(key.clone(), Some(segment("*")), now);
        cache.complete_failure(key.clone(), now + Duration::from_secs(2));

        assert_eq!(
            cache.lookup(&key, now + Duration::from_secs(2), Duration::from_secs(1)),
            AsyncValue::Stale(Some(segment("*")))
        );
    }

    #[test]
    fn lookup_returns_ready_for_empty_success() {
        let mut cache = SegmentCache::new(2);
        let now = Instant::now();
        let key = key("/repo");
        cache.complete_success(key.clone(), None, now);

        assert_eq!(
            cache.lookup(&key, now, Duration::from_secs(1)),
            AsyncValue::Ready(None)
        );
    }

    #[test]
    fn failed_refresh_keeps_existing_empty_success_as_stale() {
        let mut cache = SegmentCache::new(2);
        let now = Instant::now();
        let key = key("/repo");
        cache.complete_success(key.clone(), None, now);
        cache.complete_failure(key.clone(), now + Duration::from_secs(2));

        assert_eq!(
            cache.lookup(&key, now + Duration::from_secs(2), Duration::from_secs(1)),
            AsyncValue::Stale(None)
        );
    }

    #[test]
    fn failure_without_prior_value_is_recorded() {
        let mut cache = SegmentCache::new(2);
        let now = Instant::now();
        let key = key("/repo");
        cache.complete_failure(key.clone(), now);

        assert_eq!(
            cache.lookup(&key, now, Duration::from_secs(1)),
            AsyncValue::Failed
        );
    }

    #[test]
    fn inflight_entries_coalesce_refreshes() {
        let mut cache = SegmentCache::new(2);
        let key = key("/repo");

        assert!(cache.mark_inflight(key.clone()));
        assert!(!cache.mark_inflight(key.clone()));
        assert!(cache.is_inflight(&key));
        assert!(!cache.needs_refresh(&key, Instant::now(), Duration::ZERO));
    }

    #[test]
    fn success_completion_clears_inflight_state() {
        let mut cache = SegmentCache::new(2);
        let now = Instant::now();
        let key = key("/repo");
        cache.mark_inflight(key.clone());

        cache.complete_success(key.clone(), Some(segment("*")), now);

        assert!(!cache.is_inflight(&key));
        assert_eq!(
            cache.lookup(&key, now, Duration::from_secs(1)),
            AsyncValue::Ready(Some(segment("*")))
        );
    }

    #[test]
    fn evicts_oldest_entry_when_capacity_is_reached() {
        let mut cache = SegmentCache::new(2);
        let now = Instant::now();
        let first = key("/one");
        let second = key("/two");
        let third = key("/three");
        cache.complete_success(first.clone(), Some(segment("1")), now);
        cache.complete_success(
            second.clone(),
            Some(segment("2")),
            now + Duration::from_secs(1),
        );
        cache.complete_success(
            third.clone(),
            Some(segment("3")),
            now + Duration::from_secs(2),
        );

        assert_eq!(
            cache.lookup(
                &first,
                now + Duration::from_secs(2),
                Duration::from_secs(10)
            ),
            AsyncValue::Loading
        );
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn zero_capacity_cache_does_not_store_entries() {
        let mut cache = SegmentCache::new(0);
        let now = Instant::now();
        let key = key("/repo");
        cache.complete_success(key.clone(), Some(segment("*")), now);

        assert!(cache.is_empty());
        assert_eq!(
            cache.lookup(&key, now, Duration::from_secs(1)),
            AsyncValue::Loading
        );
    }
}

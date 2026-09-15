//! Sliding-window limits on login attempts, kept per address and per account name.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub(crate) struct Attempts {
    limit: usize,
    window: Duration,
    recent: Mutex<HashMap<String, VecDeque<Instant>>>,
}

impl Attempts {
    pub(crate) fn new(limit: usize, window: Duration) -> Self {
        Self { limit, window, recent: Mutex::new(HashMap::new()) }
    }

    /// Records an attempt for `key` at `now`; false when `key` already used its attempts in the window.
    // ponytail: entries for keys that stop trying stay until they try again; prune on a timer if memory shows.
    pub(crate) fn allow(&self, key: &str, now: Instant) -> bool {
        let mut recent = self.recent.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let times = recent.entry(key.to_owned()).or_default();
        while times.front().is_some_and(|&time| now.duration_since(time) >= self.window) {
            times.pop_front();
        }
        if times.len() >= self.limit {
            return false;
        }
        times.push_back(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attempts_refill_as_the_window_passes() {
        let attempts = Attempts::new(2, Duration::from_secs(60));
        let start = Instant::now();
        assert!(attempts.allow("ana", start));
        assert!(attempts.allow("ana", start + Duration::from_secs(1)));
        assert!(!attempts.allow("ana", start + Duration::from_secs(2)));
        assert!(attempts.allow("bia", start + Duration::from_secs(2)));
        assert!(attempts.allow("ana", start + Duration::from_secs(60)));
    }
}

use std::collections::HashMap;
use std::process::{Command, Output};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Maximum number of cache entries to prevent unbounded growth
const MAX_CACHE_ENTRIES: usize = 50;

/// Cache entry with output and timestamp
struct CacheEntry {
    output: Output,
    timestamp: Instant,
}

/// Global cache for command outputs
static COMMAND_CACHE: Mutex<Option<CommandCache>> = Mutex::new(None);

struct CommandCache {
    entries: HashMap<String, CacheEntry>,
}

impl CommandCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    fn get(&self, key: &str, max_age: Duration) -> Option<&Output> {
        self.entries.get(key).and_then(|entry| {
            if entry.timestamp.elapsed() < max_age {
                Some(&entry.output)
            } else {
                None
            }
        })
    }

    fn insert(&mut self, key: String, output: Output) {
        // Evict oldest entries if cache is full
        if self.entries.len() >= MAX_CACHE_ENTRIES {
            self.evict_oldest();
        }
        self.entries.insert(key, CacheEntry {
            output,
            timestamp: Instant::now(),
        });
    }

    fn evict_oldest(&mut self) {
        // Find and remove the oldest entry
        if let Some(oldest_key) = self.entries
            .iter()
            .min_by_key(|(_, entry)| entry.timestamp)
            .map(|(k, _)| k.clone())
        {
            self.entries.remove(&oldest_key);
        }
    }

    fn cleanup_expired(&mut self, max_age: Duration) {
        self.entries.retain(|_, entry| entry.timestamp.elapsed() < max_age);
    }
}

/// Execute a function with the cache, handling mutex errors gracefully
fn with_cache<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut CommandCache) -> R,
{
    // Use try_lock or handle poisoned mutex gracefully
    let mut guard = match COMMAND_CACHE.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            // Recover from poisoned mutex by taking the inner value
            log::warn!("Cache mutex was poisoned, recovering");
            poisoned.into_inner()
        }
    };

    if guard.is_none() {
        *guard = Some(CommandCache::new());
    }

    Some(f(guard.as_mut().unwrap()))
}

/// Cache durations for different command types
const MOUNT_CACHE_DURATION: Duration = Duration::from_millis(1000);
/// Max age for cleanup (entries older than this are removed during cleanup)
const MAX_CACHE_AGE: Duration = Duration::from_secs(60);

/// Get cached mount command output (1 second cache)
/// Used only for checking which partitions are already mounted by the OS (diskutil).
pub fn get_mount_output() -> Option<Output> {
    let cache_key = "mount".to_string();

    // Check cache first
    let cached = with_cache(|cache| {
        cache.cleanup_expired(MAX_CACHE_AGE);
        cache.get(&cache_key, MOUNT_CACHE_DURATION).cloned()
    })?;

    if let Some(output) = cached {
        return Some(output);
    }

    // Execute and cache
    let output = Command::new("mount").output().ok()?;
    with_cache(|cache| {
        cache.insert(cache_key, output.clone());
    });
    Some(output)
}

/// Invalidate all caches (call after mount/unmount operations)
pub fn invalidate_all() {
    with_cache(|cache| {
        cache.entries.clear();
    });
}

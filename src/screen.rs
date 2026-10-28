//! Tracks what each key shows, so page switches only send the keys that change

use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// How long a page switch waits for all of its keys
const PAGE_TIMEOUT: Duration = Duration::from_millis(300);

/// Hash of a cleared key
pub const CLEARED: u64 = 0;

#[derive(Default)]
struct Screen {
    /// Image hash per key
    shown: HashMap<u8, u64>,
    /// Page switch in progress: when it began and the keys it still waits for
    page: Option<(Instant, HashSet<u8>)>,
}

static SCREENS: LazyLock<Mutex<HashMap<String, Screen>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn hash(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish() | 1 // never collides with CLEARED
}

/// Records the image a key is about to show, returns false if it already shows it
pub async fn update(id: &str, position: u8, image_hash: u64) -> bool {
    let mut screens = SCREENS.lock().await;
    let screen = screens.entry(id.to_string()).or_default();

    if let Some((_, waiting)) = screen.page.as_mut() {
        waiting.remove(&position);
    }

    screen.shown.insert(position, image_hash) != Some(image_hash)
}

/// Starts waiting for all keys of a new page
pub async fn page_switch(id: &str, keys: usize) {
    let mut screens = SCREENS.lock().await;
    screens.entry(id.to_string()).or_default().page =
        Some((Instant::now(), (0..keys as u8).collect()));
}

/// Some devices blank a key on press and rely on OpenDeck sending the same image again (#47)
pub async fn pressed(id: &str, position: u8) {
    if let Some(screen) = SCREENS.lock().await.get_mut(id) {
        screen.shown.remove(&position);
    }
}

/// How long to wait before flushing: no time once a page switch has all of its keys,
/// the rest of its timeout while it doesn't, and `quiet` otherwise
pub async fn flush_delay(id: &str, quiet: Duration) -> Duration {
    let mut screens = SCREENS.lock().await;
    let Some(screen) = screens.get_mut(id) else {
        return quiet;
    };
    let Some((since, waiting)) = screen.page.as_ref() else {
        return quiet;
    };

    let elapsed = since.elapsed();
    if waiting.is_empty() || elapsed >= PAGE_TIMEOUT {
        screen.page = None;
        return Duration::ZERO;
    }

    PAGE_TIMEOUT - elapsed
}

/// Forgets everything, e.g. after the device was cleared or reset
pub async fn forget(id: &str) {
    SCREENS.lock().await.remove(id);
}

//! Helpers shared by integration tests.

use std::sync::atomic::{AtomicUsize, Ordering};

/// A token for temporary paths that differs between calls in one process,
/// even within the same clock tick.
pub fn nonce() -> String {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{nanos}-{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

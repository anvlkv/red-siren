//! Navigation commit cache (context-based).
//!
//! Purpose:
//! A `Page` component may mount *after* a `NAV_COMMITTED` event was emitted
//! (because the router only mounts it once we call `navigate()` in response
//! to that commit). Without a cache, the new page misses the event and skips
//! its enter animation on the first visit.
//!
//! This module provides a lightweight context-scoped cache for the *last*
//! committed navigation payload so newly mounted pages can synthesize an
//! enter animation if they missed the live event.
//!
//! Usage Pattern:
//! 1. In root (e.g. `App`): `let cache = provide_nav_commit_cache(cx);`
//! 2. When `NAV_COMMITTED` is received (before `navigate()`): `cache.set(Some(payload.clone()));`
//! 3. In `Page`: obtain via `use_nav_commit_cache()`; if the page’s own
//!    listener missed the commit but `cache.get()` targets this route,
//!    queue a fallback enter animation (skip bootstrap tx_id=0).

use leptos::prelude::*;
use shared::NavCommittedPayload;

/// Context-provided navigation commit cache holding the last commit payload.
#[derive(Clone, Default)]
pub struct NavCommitCache {
    last: RwSignal<Option<NavCommittedPayload>>,
}

#[allow(dead_code)]
impl NavCommitCache {
    /// Set/replace the cached payload (use `None` to clear).
    pub fn set(&self, payload: Option<NavCommittedPayload>) {
        self.last.set(payload);
    }

    /// Clear the cached payload.
    pub fn clear(&self) {
        self.last.set(None);
    }

    /// Tracked read of the current value (reactive).
    pub fn get(&self) -> Option<NavCommittedPayload> {
        self.last.get().clone()
    }

    /// Untracked read (does not subscribe to changes).
    pub fn get_untracked(&self) -> Option<NavCommittedPayload> {
        self.last.get_untracked().clone()
    }

    /// Read-only signal handle if a pure signal interface is preferred.
    pub fn read_signal(&self) -> ReadSignal<Option<NavCommittedPayload>> {
        self.last.read_only()
    }
}

/// Try to retrieve the cache from context.
pub fn use_nav_commit_cache() -> Option<NavCommitCache> {
    use_context::<NavCommitCache>()
}

/// Retrieve or panic (useful where absence is a logic error).
pub fn expect_nav_commit_cache() -> NavCommitCache {
    use_nav_commit_cache().expect(
        "NavCommitCache missing. Call `provide_nav_commit_cache(cx)` in a root component before usage.",
    )
}

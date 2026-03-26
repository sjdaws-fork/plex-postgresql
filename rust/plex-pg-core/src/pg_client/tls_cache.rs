use std::cell::Cell;

/// Thread-local cache for pool slot fast path.
/// Stores slot index + generation instead of raw pointer to prevent dangling refs.
#[derive(Clone, Copy)]
pub(crate) struct TlsPoolCache {
    pub db_handle: usize,
    pub slot_index: u32,
    pub generation: u32,
}

impl TlsPoolCache {
    pub const EMPTY: Self = Self {
        db_handle: 0,
        slot_index: u32::MAX,
        generation: 0,
    };

    pub fn is_empty(&self) -> bool {
        self.slot_index == u32::MAX
    }
}

thread_local! {
    static TLS_POOL_CACHE: Cell<TlsPoolCache> = const { Cell::new(TlsPoolCache::EMPTY) };
}

pub(crate) fn tls_pool_cache_set(db_handle: usize, slot_index: u32, generation: u32) {
    let _ = TLS_POOL_CACHE.try_with(|cache| {
        cache.set(TlsPoolCache {
            db_handle,
            slot_index,
            generation,
        });
    });
}

pub(crate) fn tls_pool_cache_get(db_handle: usize) -> Option<(u32, u32)> {
    TLS_POOL_CACHE
        .try_with(|cache| {
            let cached = cache.get();
            if cached.db_handle == db_handle && !cached.is_empty() {
                Some((cached.slot_index, cached.generation))
            } else {
                None
            }
        })
        .ok()
        .flatten()
}

pub(crate) fn tls_pool_cache_clear() {
    let _ = TLS_POOL_CACHE.try_with(|cache| cache.set(TlsPoolCache::EMPTY));
}

use std::cell::RefCell;

use super::{stmt_cache_disabled, stmt_ref_ptr, stmt_unref_ptr, MAX_CACHED_STMTS_PER_THREAD};

pub(super) struct CachedStmtEntry {
    sqlite_stmt: usize,
    pg_stmt: usize,
}

pub(super) struct ThreadCachedStmts {
    pub(super) entries: Vec<CachedStmtEntry>,
}

impl ThreadCachedStmts {
    pub(super) fn new() -> Self {
        Self {
            entries: Vec::with_capacity(MAX_CACHED_STMTS_PER_THREAD),
        }
    }

    pub(super) fn register(&mut self, sqlite_stmt: usize, pg_stmt: usize) {
        for entry in &mut self.entries {
            if entry.sqlite_stmt == sqlite_stmt {
                let old = entry.pg_stmt;
                if old != pg_stmt {
                    stmt_unref_ptr(old);
                }
                stmt_ref_ptr(pg_stmt);
                entry.pg_stmt = pg_stmt;
                return;
            }
        }

        stmt_ref_ptr(pg_stmt);

        if self.entries.len() < MAX_CACHED_STMTS_PER_THREAD {
            self.entries.push(CachedStmtEntry {
                sqlite_stmt,
                pg_stmt,
            });
        } else {
            let old = self.entries[0].pg_stmt;
            stmt_unref_ptr(old);
            self.entries.remove(0);
            self.entries.push(CachedStmtEntry {
                sqlite_stmt,
                pg_stmt,
            });
        }
    }

    pub(super) fn find(&self, sqlite_stmt: usize) -> Option<usize> {
        for entry in &self.entries {
            if entry.sqlite_stmt == sqlite_stmt {
                return Some(entry.pg_stmt);
            }
        }
        None
    }

    pub(super) fn clear(&mut self, sqlite_stmt: usize) {
        if let Some(pos) = self
            .entries
            .iter()
            .position(|e| e.sqlite_stmt == sqlite_stmt)
        {
            let old_pg_stmt = self.entries[pos].pg_stmt;
            self.entries.remove(pos);
            stmt_unref_ptr(old_pg_stmt);
        }
    }

    pub(super) fn clear_weak(&mut self, sqlite_stmt: usize) {
        if let Some(pos) = self
            .entries
            .iter()
            .position(|e| e.sqlite_stmt == sqlite_stmt)
        {
            self.entries.remove(pos);
        }
    }

    pub(super) fn drain_all(&mut self) -> Vec<usize> {
        self.entries.drain(..).map(|e| e.pg_stmt).collect()
    }
}

impl Drop for ThreadCachedStmts {
    fn drop(&mut self) {
        if stmt_cache_disabled() {
            self.entries.clear();
            return;
        }
        for entry in self.entries.drain(..) {
            stmt_unref_ptr(entry.pg_stmt);
        }
    }
}

thread_local! {
    static TLS_CACHED_STMTS: RefCell<Option<ThreadCachedStmts>> = const { RefCell::new(None) };
}

pub(super) fn with_tls_cache<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut ThreadCachedStmts) -> R,
{
    TLS_CACHED_STMTS
        .try_with(|cell| {
            let mut borrow = cell.borrow_mut();
            let cache = borrow.get_or_insert_with(ThreadCachedStmts::new);
            Some(f(cache))
        })
        .ok()
        .flatten()
}

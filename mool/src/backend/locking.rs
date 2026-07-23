//! Row-locking capabilities shared by backends that support them.

use crate::queries::QueryScope;
use crate::query_error::LockMode;
#[cfg(any(feature = "postgres", feature = "mysql"))]
use crate::query_error::LockWait;

/// Row-locking operations for typed select scopes.
pub trait RowLockExt: Sized {
    /// Adds an exclusive update lock to row-returning select terminals.
    fn for_update(self) -> QueryScope;

    /// Adds a shared lock to row-returning select terminals.
    fn for_share(self) -> QueryScope;
}

impl RowLockExt for QueryScope {
    fn for_update(self) -> QueryScope {
        self.with_lock(LockMode::Update)
    }

    fn for_share(self) -> QueryScope {
        self.with_lock(LockMode::Share)
    }
}

/// Wait policies for an existing row lock.
#[cfg(any(feature = "postgres", feature = "mysql"))]
pub trait LockWaitExt: Sized {
    /// Fails immediately when a requested row lock cannot be acquired.
    fn nowait(self) -> QueryScope;

    /// Omits rows that cannot be locked immediately.
    fn skip_locked(self) -> QueryScope;
}

#[cfg(any(feature = "postgres", feature = "mysql"))]
impl LockWaitExt for QueryScope {
    fn nowait(self) -> QueryScope {
        self.with_lock_wait(LockWait::NoWait)
    }

    fn skip_locked(self) -> QueryScope {
        self.with_lock_wait(LockWait::SkipLocked)
    }
}

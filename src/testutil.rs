//! Shared synchronization for tests that start child processes.

static SPAWNING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Keep forks in this test binary from copying a file that another test is
/// still writing. In particular, Linux can reject an exec with ETXTBSY when
/// that copied descriptor is still open in the child.
pub(crate) fn spawning_lock() -> std::sync::MutexGuard<'static, ()> {
    SPAWNING.lock().unwrap_or_else(|held| held.into_inner())
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_process_wide_lock_can_be_recovered_after_poisoning() {
        let guard = super::spawning_lock();
        drop(guard);
        let guard = super::spawning_lock();
        drop(guard);
    }
}

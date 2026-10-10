//! A mutex a panicked thread left poisoned is used anyway: the data behind Cerno's locks stays
//! consistent between statements, and one failed photo must not stop every later one.

use std::sync::{Mutex, MutexGuard};

pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

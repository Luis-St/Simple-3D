//! The flag a running evaluation is asked to give up by.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Cheap co-operative cancellation. The UI thread flips this when the user
/// edits again, and the worker abandons the run at the next node boundary.
#[derive(Clone, Debug, Default)]
pub struct Cancel(pub(super) Arc<AtomicBool>);

impl Cancel {
    pub fn new() -> Cancel {
        Cancel(Arc::new(AtomicBool::new(false)))
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

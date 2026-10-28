//! Harness stub of fastframe_shell's Waker (real: github.com/crmne/fastframe).
use std::sync::Arc;

#[derive(Clone)]
pub struct Waker {
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl Waker {
    pub fn wake(&self) {
        (self.wake)()
    }
}

impl Default for Waker {
    fn default() -> Self {
        Self { wake: Arc::new(|| {}) }
    }
}

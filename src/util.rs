use std::sync::atomic::AtomicU64;


pub struct IdGen(AtomicU64);

impl IdGen {
    pub fn new() -> Self {
        Self(AtomicU64::new(0))
    }

    pub fn next(&self) -> u64 {
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }
}

pub struct RunOnExit<F: FnOnce()>(pub Option<F>);

impl<F: FnOnce()> Drop for RunOnExit<F> {
    fn drop(&mut self) {
        if let Some(f) = self.0.take() {
            f();
        }
    }
}

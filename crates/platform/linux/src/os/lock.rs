use std::sync::{
    Condvar, Mutex, MutexGuard,
    atomic::{AtomicBool, Ordering},
};

use microps::{Lock, WaitResult};

#[derive(Debug, Default)]
pub struct LinuxMutex<T>(Mutex<T>, Condvar, AtomicBool);

impl<T> LinuxMutex<T> {
    pub fn new(value: T) -> Self {
        Self(Mutex::new(value), Condvar::new(), AtomicBool::new(false))
    }
}

impl<T> Lock<T> for LinuxMutex<T> {
    type Error = core::convert::Infallible;
    type Guard<'a>
        = MutexGuard<'a, T>
    where
        T: 'a;

    fn new(value: T) -> Self {
        Self::new(value)
    }

    fn acquire(&self) -> Result<Self::Guard<'_>, Self::Error> {
        Ok(self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()))
    }

    fn wait<'a>(
        &'a self,
        guard: Self::Guard<'a>,
    ) -> Result<WaitResult<Self::Guard<'a>>, Self::Error> {
        if self.2.load(Ordering::Acquire) {
            return Ok(WaitResult::Interrupted(guard));
        }
        let guard = self
            .1
            .wait(guard)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.2.load(Ordering::Acquire) {
            Ok(WaitResult::Interrupted(guard))
        } else {
            Ok(WaitResult::Notified(guard))
        }
    }

    fn wake_all(&self) {
        self.1.notify_all();
    }

    fn interrupt_all(&self) {
        self.2.store(true, Ordering::Release);
        self.1.notify_all();
    }
}

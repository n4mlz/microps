use std::sync::{Condvar, Mutex, MutexGuard};

use microps::Lock;

#[derive(Debug, Default)]
pub struct LinuxMutex<T>(Mutex<T>, Condvar);

impl<T> LinuxMutex<T> {
    pub fn new(value: T) -> Self {
        Self(Mutex::new(value), Condvar::new())
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

    fn wait<'a>(&'a self, guard: Self::Guard<'a>) -> Result<Self::Guard<'a>, Self::Error> {
        Ok(self
            .1
            .wait(guard)
            .unwrap_or_else(|poisoned| poisoned.into_inner()))
    }

    fn wake_all(&self) {
        self.1.notify_all();
    }
}

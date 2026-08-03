use std::sync::{Mutex, MutexGuard};

use microps::Lock;

#[derive(Debug, Default)]
pub struct LinuxMutex<T>(Mutex<T>);

impl<T> LinuxMutex<T> {
    pub fn new(value: T) -> Self {
        Self(Mutex::new(value))
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
}

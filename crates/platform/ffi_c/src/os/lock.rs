use core::{
    cell::UnsafeCell,
    convert::Infallible,
    ffi::c_void,
    ops::{Deref, DerefMut},
    sync::atomic::Ordering,
};

use microps::{Lock, WaitResult};

use crate::abi::{PLATFORM_READY, platform};

pub struct CMutex<T> {
    handle: *mut c_void,
    value: UnsafeCell<T>,
}

unsafe impl<T: Send> Send for CMutex<T> {}
unsafe impl<T: Send> Sync for CMutex<T> {}

pub struct CMutexGuard<'a, T> {
    mutex: &'a CMutex<T>,
}

impl<T> Deref for CMutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mutex.value.get() }
    }
}

impl<T> DerefMut for CMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mutex.value.get() }
    }
}

impl<T> Drop for CMutexGuard<'_, T> {
    fn drop(&mut self) {
        let ops = platform();
        unsafe { (ops.mutex_unlock)(ops.context, self.mutex.handle) }
    }
}

impl<T> CMutex<T> {
    fn new(value: T) -> Self {
        let ops = platform();
        let handle = unsafe { (ops.mutex_create)(ops.context) };
        assert!(!handle.is_null(), "microps mutex_create callback failed");
        Self {
            handle,
            value: UnsafeCell::new(value),
        }
    }
}

impl<T> Lock<T> for CMutex<T> {
    type Error = Infallible;
    type Guard<'a>
        = CMutexGuard<'a, T>
    where
        T: 'a;

    fn new(value: T) -> Self {
        Self::new(value)
    }

    fn acquire(&self) -> Result<Self::Guard<'_>, Self::Error> {
        let ops = platform();
        unsafe { (ops.mutex_lock)(ops.context, self.handle) };
        Ok(CMutexGuard { mutex: self })
    }

    fn wait<'a>(
        &'a self,
        guard: Self::Guard<'a>,
    ) -> Result<WaitResult<Self::Guard<'a>>, Self::Error> {
        let ops = platform();
        let result = unsafe { (ops.mutex_wait)(ops.context, self.handle) };
        core::mem::forget(guard);
        let guard = CMutexGuard { mutex: self };
        Ok(if result == 1 {
            WaitResult::Interrupted(guard)
        } else {
            WaitResult::Notified(guard)
        })
    }

    fn wake_all(&self) {
        let ops = platform();
        unsafe { (ops.mutex_wake_all)(ops.context, self.handle) }
    }

    fn interrupt_all(&self) {
        let ops = platform();
        unsafe { (ops.mutex_interrupt_all)(ops.context, self.handle) }
    }
}

impl<T> Drop for CMutex<T> {
    fn drop(&mut self) {
        if PLATFORM_READY.load(Ordering::Acquire) {
            let ops = platform();
            unsafe { (ops.mutex_destroy)(ops.context, self.handle) }
        }
    }
}

use core::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};

use microps::{Irq, IrqLine, Lock, Platform, Stack};

struct MockRuntime;

#[derive(Debug, Default)]
struct TestMutex<T>(Mutex<T>);

impl<T> Lock<T> for TestMutex<T> {
    type Error = core::convert::Infallible;
    type Guard<'a>
        = MutexGuard<'a, T>
    where
        T: 'a;

    fn new(value: T) -> Self {
        Self(Mutex::new(value))
    }

    fn acquire(&self) -> Result<Self::Guard<'_>, Self::Error> {
        Ok(self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()))
    }
}

static INIT_CALLS: AtomicUsize = AtomicUsize::new(0);
static SHUTDOWN_CALLS: AtomicUsize = AtomicUsize::new(0);

impl Platform for MockRuntime {
    type Error = core::convert::Infallible;
    type Mutex<T: Send> = TestMutex<T>;

    fn init() -> Result<(), <Self as Platform>::Error> {
        INIT_CALLS.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn shutdown() {
        SHUTDOWN_CALLS.fetch_add(1, Ordering::SeqCst);
    }
}

impl Irq for MockRuntime {
    type Error = core::convert::Infallible;

    fn register(_: IrqLine, _: fn(IrqLine, usize), _: usize) -> Result<(), Self::Error> {
        Ok(())
    }

    fn raise(_: IrqLine) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[test]
fn stack_lifecycle_calls_runtime_hooks() {
    INIT_CALLS.store(0, Ordering::SeqCst);
    SHUTDOWN_CALLS.store(0, Ordering::SeqCst);

    Stack::<MockRuntime>::init().unwrap();
    Stack::<MockRuntime>::shutdown();

    assert_eq!(INIT_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(SHUTDOWN_CALLS.load(Ordering::SeqCst), 1);
}

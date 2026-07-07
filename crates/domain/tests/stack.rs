use core::sync::atomic::{AtomicUsize, Ordering};

use microps::{Platform, Stack};

struct MockRuntime;

static INIT_CALLS: AtomicUsize = AtomicUsize::new(0);
static SHUTDOWN_CALLS: AtomicUsize = AtomicUsize::new(0);

impl Platform for MockRuntime {
    type Error = core::convert::Infallible;

    fn init() -> Result<(), Self::Error> {
        INIT_CALLS.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn shutdown() {
        SHUTDOWN_CALLS.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn stack_lifecycle_calls_runtime_hooks() {
    INIT_CALLS.store(0, Ordering::SeqCst);
    SHUTDOWN_CALLS.store(0, Ordering::SeqCst);

    Stack::init::<MockRuntime>().unwrap();
    Stack::shutdown::<MockRuntime>();

    assert_eq!(INIT_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(SHUTDOWN_CALLS.load(Ordering::SeqCst), 1);
}

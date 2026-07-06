use core::sync::atomic::{AtomicUsize, Ordering};

use microps::{Runtime, Stack};

struct MockRuntime;

static POLL_CALLS: AtomicUsize = AtomicUsize::new(0);
static SHUTDOWN_CALLS: AtomicUsize = AtomicUsize::new(0);

impl Runtime for MockRuntime {
    type Error = core::convert::Infallible;

    fn poll() -> Result<(), Self::Error> {
        POLL_CALLS.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn shutdown() {
        SHUTDOWN_CALLS.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn stack_lifecycle_calls_runtime_hooks() {
    POLL_CALLS.store(0, Ordering::SeqCst);
    SHUTDOWN_CALLS.store(0, Ordering::SeqCst);

    Stack::init::<MockRuntime>().unwrap();
    Stack::poll::<MockRuntime>().unwrap();
    Stack::shutdown::<MockRuntime>();

    assert_eq!(POLL_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(SHUTDOWN_CALLS.load(Ordering::SeqCst), 1);
}

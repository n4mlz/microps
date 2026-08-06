use core::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

use microps::{
    Irq, IrqLine, Lock, Platform, Random, Stack,
    protocol::{Ipv4Addr, Ipv4Endpoint, UdpPcbError},
};

struct MockRuntime;

static STACK: OnceLock<Stack<MockRuntime>> = OnceLock::new();

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

    fn stack() -> &'static Stack<Self> {
        STACK.get_or_init(Stack::new)
    }

    fn init() -> Result<(), <Self as Platform>::Error> {
        INIT_CALLS.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn shutdown() {
        SHUTDOWN_CALLS.fetch_add(1, Ordering::SeqCst);
    }
}

impl Random for MockRuntime {
    type Error = core::convert::Infallible;

    fn random16() -> Result<u16, Self::Error> {
        Ok(0)
    }
}

impl Irq for MockRuntime {
    type Error = core::convert::Infallible;

    fn register(_: IrqLine, _: Box<dyn Fn(IrqLine) + Send + Sync>) -> Result<(), Self::Error> {
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

#[test]
fn udp_registry_opens_binds_and_releases_sockets() {
    let stack = MockRuntime::stack();
    let first = stack.udp_pcbs.open();
    let second = stack.udp_pcbs.open();
    let endpoint = Ipv4Endpoint::new(Ipv4Addr::from([192, 0, 2, 2]), 7);

    assert_eq!(stack.udp_pcbs.bind(first, endpoint), Ok(()));
    assert_eq!(
        stack.udp_pcbs.bind(second, endpoint),
        Err(UdpPcbError::AlreadyBound)
    );
    assert_eq!(stack.udp_pcbs.close(first), Ok(()));
    assert_eq!(stack.udp_pcbs.bind(second, endpoint), Ok(()));
    assert_eq!(stack.udp_pcbs.close(second), Ok(()));
    assert_eq!(stack.udp_pcbs.close(second), Err(UdpPcbError::NotFound));
}

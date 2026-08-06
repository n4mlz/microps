use std::sync::{
    Arc, Condvar, Mutex, OnceLock,
    atomic::{AtomicUsize, Ordering},
};

use microps::{
    Device, DeviceBackend, DeviceError, DeviceKind, DeviceMeta, DeviceRegistry, Irq, IrqLine, Lock,
    LoopbackDevice, Platform, Random, Stack, Time, protocol::EtherType,
};

#[derive(Debug, Default)]
struct TestMutex<T>(Mutex<T>, Condvar);

impl<T> Lock<T> for TestMutex<T> {
    type Error = core::convert::Infallible;
    type Guard<'a>
        = std::sync::MutexGuard<'a, T>
    where
        T: 'a;

    fn new(value: T) -> Self {
        Self(Mutex::new(value), Condvar::new())
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

#[derive(Debug, Clone, Copy, Default)]
struct TestPlatform;

static STACK: OnceLock<Stack<TestPlatform>> = OnceLock::new();

impl Platform for TestPlatform {
    type Error = core::convert::Infallible;
    type Mutex<T: Send> = TestMutex<T>;

    fn stack() -> &'static Stack<Self> {
        STACK.get_or_init(Stack::new)
    }

    fn shutdown() {}
}

impl Random for TestPlatform {
    type Error = core::convert::Infallible;

    fn random16() -> Result<u16, Self::Error> {
        Ok(0)
    }
}

impl Time for TestPlatform {
    fn monotonic_time_microseconds() -> u64 {
        0
    }
}

impl Irq for TestPlatform {
    type Error = core::convert::Infallible;

    fn register(_: IrqLine, _: Box<dyn Fn(IrqLine) + Send + Sync>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn raise(_: IrqLine) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct CountingBackend {
    open_calls: Arc<AtomicUsize>,
    close_calls: Arc<AtomicUsize>,
    output_calls: Arc<AtomicUsize>,
}

impl CountingBackend {
    fn new() -> (Self, Arc<AtomicUsize>, Arc<AtomicUsize>, Arc<AtomicUsize>) {
        let open_calls = Arc::new(AtomicUsize::new(0));
        let close_calls = Arc::new(AtomicUsize::new(0));
        let output_calls = Arc::new(AtomicUsize::new(0));
        (
            Self {
                open_calls: Arc::clone(&open_calls),
                close_calls: Arc::clone(&close_calls),
                output_calls: Arc::clone(&output_calls),
            },
            open_calls,
            close_calls,
            output_calls,
        )
    }
}

impl DeviceBackend<TestPlatform> for CountingBackend {
    fn open(&mut self) -> Result<(), DeviceError> {
        self.open_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn close(&mut self) -> Result<(), DeviceError> {
        self.close_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn output(&mut self, _: u16, _: &[u8], _: Option<&[u8]>) -> Result<(), DeviceError> {
        self.output_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn input(&mut self) -> Result<(), DeviceError> {
        Ok(())
    }
}

#[test]
fn registry_returns_a_stable_device_key() {
    let registry = DeviceRegistry::<TestPlatform>::default();
    let (backend, _, _, _) = CountingBackend::new();
    let handle = registry.register(Device::new(
        DeviceMeta::new("net0", DeviceKind::Dummy, 128),
        backend,
    ));
    let devices = registry.acquire().unwrap();
    assert_eq!(devices.get(handle).unwrap().meta().name(), "net0");
}

#[test]
fn device_enforces_state_and_mtu() {
    let (backend, open_calls, close_calls, output_calls) = CountingBackend::new();
    let mut device = Device::new(DeviceMeta::new("net0", DeviceKind::Dummy, 4), backend);
    assert!(matches!(
        device.output(EtherType::Ipv4 as u16, &[1], None),
        Err(DeviceError::NotOpen)
    ));
    device.open().unwrap();
    assert!(matches!(
        device.output(EtherType::Ipv4 as u16, &[1, 2, 3, 4, 5], None),
        Err(DeviceError::PayloadTooLarge { mtu: 4, len: 5 })
    ));
    device
        .output(EtherType::Ipv4 as u16, &[1, 2, 3, 4], None)
        .unwrap();
    device.close().unwrap();
    assert_eq!(open_calls.load(Ordering::SeqCst), 1);
    assert_eq!(close_calls.load(Ordering::SeqCst), 1);
    assert_eq!(output_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn loopback_transfers_output_to_the_input_queue() {
    let stack = TestPlatform::stack();
    let device = stack.devices.register_device(
        DeviceMeta::new("net0", DeviceKind::Loopback, 65_535),
        LoopbackDevice::new(),
    );
    stack.open_all().unwrap();
    {
        let mut devices = stack.devices.acquire().unwrap();
        devices
            .get_mut(device)
            .unwrap()
            .output(EtherType::Ipv4 as u16, &[1, 2, 3], None)
            .unwrap();
    }
    stack.soft_input().unwrap();
    stack.close_all();
}

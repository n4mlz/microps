use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use microps::{
    Device, DeviceBackend, DeviceError, DeviceKind, DeviceMeta, DeviceRegistry, Irq, IrqLine, Lock,
    LoopbackDevice, Platform, Stack, protocol::EtherType,
};

#[derive(Debug, Default)]
struct TestMutex<T>(Mutex<T>);

impl<T> Lock<T> for TestMutex<T> {
    type Error = core::convert::Infallible;
    type Guard<'a>
        = std::sync::MutexGuard<'a, T>
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

#[derive(Debug, Clone, Copy, Default)]
struct TestPlatform;

impl Platform for TestPlatform {
    type Error = core::convert::Infallible;
    type Mutex<T: Send> = TestMutex<T>;

    fn shutdown() {}
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
    fn open(&mut self) {
        self.open_calls.fetch_add(1, Ordering::SeqCst);
    }
    fn close(&mut self) {
        self.close_calls.fetch_add(1, Ordering::SeqCst);
    }
    fn output(&mut self, _: u16, _: &[u8], _: Option<&[u8]>) -> Result<(), DeviceError> {
        self.output_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn input(&mut self, _: u16, _: &[u8]) -> Result<(), DeviceError> {
        Ok(())
    }
}

#[test]
fn registry_returns_a_stable_device_key() {
    let mut registry = DeviceRegistry::<TestPlatform>::default();
    let (backend, _, _, _) = CountingBackend::new();
    let handle = registry.register(Device::new(
        DeviceMeta::new("net0", DeviceKind::Dummy, 128),
        backend,
    ));
    assert_eq!(registry.device(handle).unwrap().meta().name(), "net0");
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
    let mut stack = Stack::<TestPlatform>::new();
    let device = stack.register_device(
        DeviceMeta::new("net0", DeviceKind::Loopback, 65_535),
        LoopbackDevice::new(stack.input_queue().clone()),
    );
    stack.open_all().unwrap();
    let device_ref = stack.devices.device_mut(device).unwrap();
    device_ref
        .output(EtherType::Ipv4 as u16, &[1, 2, 3], None)
        .unwrap();
    stack.soft_input().unwrap();
    stack.close_all();
}

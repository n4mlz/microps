use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use microps::{Device, DeviceBackend, DeviceError, DeviceKind, DeviceMeta, DeviceRegistry};

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

impl DeviceBackend for CountingBackend {
    fn open(
        &mut self,
        _meta: &DeviceMeta,
        _state: &microps::DeviceState,
    ) -> Result<(), DeviceError> {
        self.open_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn close(
        &mut self,
        _meta: &DeviceMeta,
        _state: &microps::DeviceState,
    ) -> Result<(), DeviceError> {
        self.close_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn output(
        &mut self,
        _meta: &DeviceMeta,
        _state: &microps::DeviceState,
        _frame_type: u16,
        _data: &[u8],
        _dst: Option<&[u8]>,
    ) -> Result<(), DeviceError> {
        self.output_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn registry_assigns_sequential_names() {
    let mut registry = DeviceRegistry::new();
    let (backend, _, _, _) = CountingBackend::new();

    let handle = registry.register(Device::new(
        DeviceMeta::new("net0", DeviceKind::Dummy, 128),
        backend,
    ));

    assert_eq!(handle, 0);
    let device = registry.device(handle).expect("device exists");
    assert_eq!(device.meta().name(), "net0");
}

#[test]
fn device_enforces_state_and_mtu() {
    let (backend, open_calls, close_calls, output_calls) = CountingBackend::new();
    let mut device = Device::new(DeviceMeta::new("net0", DeviceKind::Dummy, 4), backend);

    assert!(matches!(
        device.output(0x0800, &[1], None),
        Err(DeviceError::NotOpen)
    ));

    device.open().expect("device opens");
    assert!(matches!(
        device.output(0x0800, &[1, 2, 3, 4, 5], None),
        Err(DeviceError::PayloadTooLarge { mtu: 4, len: 5 })
    ));
    device
        .output(0x0800, &[1, 2, 3, 4], None)
        .expect("payload fits");
    device.close().expect("device closes");

    assert_eq!(open_calls.load(Ordering::SeqCst), 1);
    assert_eq!(close_calls.load(Ordering::SeqCst), 1);
    assert_eq!(output_calls.load(Ordering::SeqCst), 1);
}

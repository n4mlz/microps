use crate::{DeviceMeta, DeviceState, debugdump};

/// Backend hooks for platform-specific device behavior.
pub trait DeviceBackend: core::fmt::Debug {
    fn open(&mut self, _meta: &DeviceMeta, _state: &DeviceState) {}

    fn close(&mut self, _meta: &DeviceMeta, _state: &DeviceState) {}

    fn output(
        &mut self,
        meta: &DeviceMeta,
        state: &DeviceState,
        frame_type: u16,
        data: &[u8],
        dst: Option<&[u8]>,
    );
}

/// Architecture-independent dummy device used in step01.
#[derive(Debug, Default, Clone, Copy)]
pub struct DummyDevice;

impl DummyDevice {
    pub fn new() -> Self {
        Self
    }
}

impl DeviceBackend for DummyDevice {
    fn output(
        &mut self,
        _meta: &DeviceMeta,
        _state: &DeviceState,
        _frame_type: u16,
        data: &[u8],
        _dst: Option<&[u8]>,
    ) {
        debugdump(data);
    }
}

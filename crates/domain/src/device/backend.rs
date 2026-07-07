use crate::{DeviceMeta, DeviceState};

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

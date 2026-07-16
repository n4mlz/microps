use crate::{DeviceBackend, DeviceError, DeviceMeta, DeviceState, debug, debugdump};

/// Loopback device that logs transmitted frames.
#[derive(Debug, Default, Clone, Copy)]
pub struct LoopbackDevice;

impl LoopbackDevice {
    pub fn new() -> Self {
        Self
    }
}

impl DeviceBackend for LoopbackDevice {
    fn output(
        &mut self,
        meta: &DeviceMeta,
        _state: &DeviceState,
        frame_type: u16,
        data: &[u8],
        _dst: Option<&[u8]>,
    ) -> Result<(), DeviceError> {
        debug!(
            "dev={}, type=0x{frame_type:04x}, len={}",
            meta.name(),
            data.len()
        );
        debugdump(data);
        Ok(())
    }

    fn input(
        &mut self,
        _meta: &DeviceMeta,
        _state: &DeviceState,
    ) -> Result<Option<crate::ReceivedFrame<'_>>, DeviceError> {
        Ok(None)
    }
}

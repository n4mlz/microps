use crate::{DeviceBackend, DeviceMeta, DeviceState, debug, debugdump, net_input};

/// Loopback device that feeds transmitted frames back into the stack.
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
        dst: Option<&[u8]>,
    ) {
        debug!(
            "dev={}, type=0x{frame_type:04x}, len={}",
            meta.name(),
            data.len()
        );
        debugdump(data);
        net_input(meta, frame_type, data, dst);
    }
}

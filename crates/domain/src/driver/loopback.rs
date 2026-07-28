use crate::{DeviceBackend, debug, debugdump};

#[derive(Debug, Default)]
pub struct LoopbackDevice;

impl LoopbackDevice {
    pub fn new() -> Self {
        Self
    }
}

impl DeviceBackend for LoopbackDevice {
    fn output(&mut self, frame_type: u16, data: &[u8], _dst: Option<&[u8]>) {
        debug!("type=0x{frame_type:04x}, len={}", data.len());
        debugdump(data);
    }
}

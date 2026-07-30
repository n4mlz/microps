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

        // Step 5 does not reintroduce loopback frames into the stack. When
        // deferred input is added, share only the input queue with this
        // backend (not the Stack itself); Stack will process the queue after
        // releasing the device borrow.
    }
}

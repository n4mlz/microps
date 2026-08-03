use alloc::vec::Vec;

use crate::{
    DeviceBackend, DeviceError, DeviceKey, InputQueue, Platform, ReceivedFrame, debug, debugdump,
};

pub struct LoopbackDevice<P: Platform> {
    input_queue: InputQueue<P>,
    device_key: Option<DeviceKey>,
}

impl<P: Platform> LoopbackDevice<P> {
    pub fn new(input_queue: InputQueue<P>) -> Self {
        Self {
            input_queue,
            device_key: None,
        }
    }
}

impl<P: Platform> DeviceBackend<P> for LoopbackDevice<P> {
    fn set_device_key(&mut self, device: DeviceKey) {
        self.device_key = Some(device);
    }

    fn output(
        &mut self,
        frame_type: u16,
        data: &[u8],
        _dst: Option<&[u8]>,
    ) -> Result<(), DeviceError> {
        debug!("type=0x{frame_type:04x}, len={}", data.len());
        debugdump(data);
        let device = self.device_key.ok_or(DeviceError::MissingDeviceKey)?;
        self.input_queue
            .push(ReceivedFrame::new(device, frame_type, data));
        Ok(())
    }

    /// Loopback output is already copied into the shared input queue.
    fn input(&mut self) -> Result<Option<(u16, Vec<u8>)>, DeviceError> {
        Ok(None)
    }
}

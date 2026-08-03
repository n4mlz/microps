use crate::{
    DeviceBackend, DeviceError, DeviceKey, InputQueue, IrqLine, Platform, ReceivedFrame, debug,
    debugdump,
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

    fn input(&mut self, frame_type: u16, data: &[u8]) -> Result<(), DeviceError> {
        let device = self.device_key.ok_or(DeviceError::MissingDeviceKey)?;
        self.input_queue
            .push(ReceivedFrame::new(device, frame_type, data));
        // SoftInput handlers process the queue through Stack and therefore
        // lock Stack. This method must only be called when that Stack lock is
        // not held by the caller.
        P::raise(IrqLine::SoftInput).map_err(|_| DeviceError::InputIrq)
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
        self.input(frame_type, data)?;
        Ok(())
    }
}

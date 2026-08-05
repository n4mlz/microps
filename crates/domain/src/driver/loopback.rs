use crate::{DeviceBackend, DeviceError, DeviceKey, Platform, ReceivedFrame, debug, debugdump};

pub struct LoopbackDevice {
    device_key: Option<DeviceKey>,
}

impl LoopbackDevice {
    pub fn new() -> Self {
        Self { device_key: None }
    }

    fn input<P: Platform + 'static>(
        &mut self,
        frame_type: u16,
        data: &[u8],
    ) -> Result<(), DeviceError> {
        let device = self.device_key.ok_or(DeviceError::MissingDeviceKey)?;
        P::stack()
            .input_queue
            .push(ReceivedFrame::new(device, frame_type, data));
        Ok(())
    }
}

impl Default for LoopbackDevice {
    fn default() -> Self {
        Self::new()
    }
}

impl<P: Platform + 'static> DeviceBackend<P> for LoopbackDevice {
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
        self.input::<P>(frame_type, data)?;
        Ok(())
    }
}

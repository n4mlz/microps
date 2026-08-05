use super::{DeviceError, DeviceKey};
use crate::{Platform, protocol::MacAddr};

pub trait DeviceBackend<P: Platform>: Send {
    fn open(&mut self) -> Result<(), DeviceError> {
        Ok(())
    }

    fn close(&mut self) -> Result<(), DeviceError> {
        Ok(())
    }

    fn set_device_key(&mut self, _device: DeviceKey) {}

    fn hardware_address(&self) -> Option<MacAddr> {
        None
    }

    fn output(
        &mut self,
        frame_type: u16,
        data: &[u8],
        dst: Option<&[u8]>,
    ) -> Result<(), DeviceError>;

    /// Reads available frames from the device and queues them.
    fn input(&mut self) -> Result<(), DeviceError> {
        Ok(())
    }
}

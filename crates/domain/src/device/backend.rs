use super::{DeviceError, DeviceKey};
use crate::Platform;

pub trait DeviceBackend<P: Platform>: Send {
    fn open(&mut self) {}

    fn close(&mut self) {}

    fn set_device_key(&mut self, _device: DeviceKey) {}

    fn output(
        &mut self,
        frame_type: u16,
        data: &[u8],
        dst: Option<&[u8]>,
    ) -> Result<(), DeviceError>;

    /// Queues one received frame and raises the logical soft-input IRQ.
    fn input(&mut self, frame_type: u16, data: &[u8]) -> Result<(), DeviceError>;
}

use alloc::vec::Vec;

use super::{DeviceError, DeviceKey};
use crate::Platform;

pub trait DeviceBackend<P: Platform> {
    fn open(&mut self) {}

    fn close(&mut self) {}

    fn set_device_key(&mut self, _device: DeviceKey) {}

    fn output(
        &mut self,
        frame_type: u16,
        data: &[u8],
        dst: Option<&[u8]>,
    ) -> Result<(), DeviceError>;

    /// Returns `(frame_type, data)` for one received frame. `data` is owned
    /// by the backend and `None` means that no frame is currently available.
    fn input(&mut self) -> Result<Option<(u16, Vec<u8>)>, DeviceError>;
}

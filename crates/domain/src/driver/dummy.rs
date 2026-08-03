use crate::{DeviceBackend, DeviceError, Platform, debugdump};

#[derive(Debug, Default, Clone, Copy)]
pub struct DummyDevice;

impl<P: Platform> DeviceBackend<P> for DummyDevice {
    fn output(
        &mut self,
        _frame_type: u16,
        data: &[u8],
        _dst: Option<&[u8]>,
    ) -> Result<(), DeviceError> {
        debugdump(data);
        Ok(())
    }
}

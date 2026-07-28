use crate::{DeviceBackend, debugdump};

#[derive(Debug, Default, Clone, Copy)]
pub struct DummyDevice;

impl DeviceBackend for DummyDevice {
    fn output(&mut self, _frame_type: u16, data: &[u8], _dst: Option<&[u8]>) {
        debugdump(data);
    }
}

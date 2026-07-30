pub trait DeviceBackend: core::fmt::Debug {
    fn open(&mut self) {}

    fn close(&mut self) {}

    fn output(&mut self, frame_type: u16, data: &[u8], dst: Option<&[u8]>);
}

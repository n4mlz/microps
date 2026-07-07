use crate::{DeviceMeta, Platform, debug, debugdump, error, info};

/// Temporary ingress hook for loopback-style delivery.
///
/// Replace this with a real input path once packet parsing and dispatch exist.
pub fn net_input(meta: &DeviceMeta, frame_type: u16, data: &[u8], _dst: Option<&[u8]>) {
    debug!(
        "dev={}, type=0x{frame_type:04x}, len={}",
        meta.name(),
        data.len()
    );
    debugdump(data);
}

/// Protocol stack lifecycle.
pub struct Stack;

impl Stack {
    pub fn init<P: Platform>() -> Result<(), P::Error> {
        info!("initialize...");
        let result = P::init();
        if result.is_ok() {
            info!("success");
        } else {
            error!("failure");
        }
        result
    }

    pub fn shutdown<P: Platform>() {
        info!("shutting down...");
        P::shutdown();
        info!("success");
    }
}

use crate::{Runtime, error, info};

/// Protocol stack state machine.
pub struct Stack;

impl Stack {
    pub fn init<P: Runtime>() -> Result<(), P::Error> {
        info!("initialize...");
        let result = P::init();
        if result.is_ok() {
            info!("success");
        } else {
            error!("failure");
        }
        result
    }

    pub fn poll<P: Runtime>() -> Result<(), P::Error> {
        P::poll()
    }

    pub fn shutdown<P: Runtime>() {
        info!("shutting down...");
        P::shutdown();
        info!("success");
    }
}

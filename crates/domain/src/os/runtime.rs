use super::Lock;

/// Platform-specific lifecycle required by the stack.
pub trait Platform: super::Irq {
    type Error;
    type Mutex<T>: Lock<T> + Send + Sync
    where
        T: Send;

    fn init() -> Result<(), <Self as Platform>::Error> {
        Ok(())
    }

    fn shutdown();
}

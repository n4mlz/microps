use super::Lock;

/// Platform-specific lifecycle required by the stack.
pub trait Platform {
    type Error;
    type Mutex<T>: Lock<T> + Send + Sync
    where
        T: Send;

    fn init() -> Result<(), Self::Error> {
        Ok(())
    }

    fn shutdown();
}

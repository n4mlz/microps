use super::Lock;

/// Platform-specific lifecycle required by the stack.
pub trait Platform: super::Irq + super::Random {
    type Error;
    type Mutex<T>: Lock<T> + Send + Sync
    where
        T: Send;

    /// The one stack instance owned by this platform.
    fn stack() -> &'static crate::Stack<Self>
    where
        Self: Sized;

    fn init() -> Result<(), <Self as Platform>::Error> {
        Ok(())
    }

    fn shutdown();
}

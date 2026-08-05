use super::Lock;

pub trait Platform: super::Irq + super::Random {
    type Error;
    type Mutex<T>: Lock<T> + Send + Sync
    where
        T: Send;

    fn stack() -> &'static crate::Stack<Self>
    where
        Self: Sized;

    fn init() -> Result<(), <Self as Platform>::Error> {
        Ok(())
    }

    fn shutdown();
}

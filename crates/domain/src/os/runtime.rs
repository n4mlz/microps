/// Platform-specific lifecycle required by the stack.
pub trait Runtime {
    type Error;

    fn init() -> Result<(), Self::Error> {
        Ok(())
    }

    fn poll() -> Result<(), Self::Error>;
    fn shutdown();
}

/// Platform-specific lifecycle required by the stack.
pub trait Platform {
    type Error;

    fn init() -> Result<(), Self::Error> {
        Ok(())
    }

    fn shutdown();
}

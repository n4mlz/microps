pub trait Random {
    type Error;

    fn init() -> Result<(), Self::Error> {
        Ok(())
    }

    fn random16() -> Result<u16, Self::Error>;
}

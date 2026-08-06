pub trait Random {
    type Error;

    fn init() -> Result<(), Self::Error> {
        Ok(())
    }

    fn random16() -> Result<u16, Self::Error>;

    fn random32() -> Result<u32, Self::Error> {
        Ok((u32::from(Self::random16()?) << 16) | u32::from(Self::random16()?))
    }
}

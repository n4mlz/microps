use core::ops::Deref;

/// Platform-provided lock acquisition.
///
/// The returned guard releases the lock when dropped.
pub trait Lock {
    type Error;
    type Guard: Deref;

    fn init() -> Result<(), Self::Error> {
        Ok(())
    }

    fn acquire() -> Result<Self::Guard, Self::Error>;
}

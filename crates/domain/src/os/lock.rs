use core::{fmt::Debug, ops::DerefMut};

/// Platform-provided lock acquisition.
///
/// The returned guard releases the lock when dropped.
pub trait Lock<T: ?Sized> {
    type Error: Debug;
    type Guard<'a>: DerefMut<Target = T>
    where
        Self: 'a,
        T: 'a;

    fn new(value: T) -> Self
    where
        Self: Sized,
        T: Sized;

    fn acquire(&self) -> Result<Self::Guard<'_>, Self::Error>;
}

use core::{fmt::Debug, ops::DerefMut};

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

    fn wait<'a>(&'a self, guard: Self::Guard<'a>) -> Result<Self::Guard<'a>, Self::Error>;

    fn wake_all(&self);
}

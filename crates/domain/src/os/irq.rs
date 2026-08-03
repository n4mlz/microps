use alloc::boxed::Box;

/// Logical interrupt lines used by the network stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IrqLine {
    DeviceInput,
    SoftInput,
}

/// Platform-provided interrupt controller.
pub trait Irq {
    type Error;

    fn register(
        line: IrqLine,
        handler: Box<dyn Fn(IrqLine) + Send + Sync>,
    ) -> Result<(), Self::Error>;
    /// Raises an interrupt line.
    ///
    /// In particular, `SoftInput` must only be raised after releasing the
    /// stack lock; its handler takes that same lock to drain the input queue.
    fn raise(line: IrqLine) -> Result<(), Self::Error>;

    fn init() -> Result<(), Self::Error> {
        Ok(())
    }

    fn run() -> Result<(), Self::Error> {
        Ok(())
    }

    fn shutdown() {}
}

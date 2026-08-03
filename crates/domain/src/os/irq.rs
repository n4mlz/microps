/// Logical interrupt lines used by the network stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IrqLine {
    SoftInput,
}

/// Platform-provided interrupt controller.
pub trait Irq {
    type Error;

    fn register(line: IrqLine, handler: fn(IrqLine, usize), arg: usize) -> Result<(), Self::Error>;
    fn raise(line: IrqLine) -> Result<(), Self::Error>;

    fn init() -> Result<(), Self::Error> {
        Ok(())
    }

    fn run() -> Result<(), Self::Error> {
        Ok(())
    }

    fn shutdown() {}
}

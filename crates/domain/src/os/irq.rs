/// Platform-provided interrupt controller.
pub trait Irq {
    type Error;

    fn register(irq: usize, handler: fn(usize, usize), arg: usize) -> Result<(), Self::Error>;
    fn raise(irq: usize) -> Result<(), Self::Error>;

    fn init() -> Result<(), Self::Error> {
        Ok(())
    }

    fn run() -> Result<(), Self::Error> {
        Ok(())
    }

    fn shutdown() {}
}

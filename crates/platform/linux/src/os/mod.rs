mod irq;
mod lock;
mod random;
mod runtime;
mod stdout;

pub use lock::LinuxMutex;
pub use runtime::should_terminate;

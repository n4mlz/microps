use std::{sync::OnceLock, time::Instant};

use microps::Time;

use crate::LinuxPlatform;

static START_TIME: OnceLock<Instant> = OnceLock::new();

impl Time for LinuxPlatform {
    fn monotonic_time_microseconds() -> u64 {
        START_TIME.get_or_init(Instant::now).elapsed().as_micros() as u64
    }
}

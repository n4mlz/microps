pub trait Time {
    fn monotonic_time_microseconds() -> u64;

    fn monotonic_time_seconds() -> u64 {
        Self::monotonic_time_microseconds() / 1_000_000
    }
}

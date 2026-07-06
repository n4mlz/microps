use std::{fmt, io, io::Write};

use microps::Stdout;

use crate::LinuxPlatform;

impl Stdout for LinuxPlatform {
    fn write(args: fmt::Arguments<'_>) {
        let mut out = io::stdout().lock();
        let _ = out.write_fmt(args);
        let _ = out.flush();
    }
}

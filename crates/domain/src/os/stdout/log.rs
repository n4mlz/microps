#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let mut writer = $crate::Writer;
        let _ = write!(&mut writer, $($arg)*);
    }};
}

#[macro_export]
macro_rules! println {
    () => {{
        $crate::print!("\n");
    }};
    ($($arg:tt)*) => {{
        $crate::print!("{}\n", format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {{
        $crate::println!("[Info] {}", format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {{
        $crate::println!("[Warn] {}", format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {{
        $crate::println!("[Error] {}", format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {{
        $crate::println!("[Debug] {}", format_args!($($arg)*));
    }};
}

/// Dump bytes in a classic hex/ascii table.
pub fn debugdump(data: &[u8]) {
    let size = data.len();

    println!("+------+-------------------------------------------------+------------------+");
    for offset in (0..size).step_by(16) {
        print!("| {offset:4x} | ");
        for index in 0..16 {
            if let Some(byte) = data.get(offset + index) {
                print!("{byte:02x} ");
            } else {
                print!("   ");
            }
        }
        print!("| ");
        for index in 0..16 {
            if let Some(byte) = data.get(offset + index) {
                if byte.is_ascii_graphic() {
                    print!("{}", char::from(*byte));
                } else {
                    print!(".");
                }
            } else {
                print!(" ");
            }
        }
        println!(" |");
    }
    println!("+------+-------------------------------------------------+------------------+");
}

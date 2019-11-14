use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering::Relaxed;

static LOG_LEVEL : AtomicU8 = AtomicU8::new(0);

pub const LOG_LEVEL_DEBUG: u8 = 2;

pub fn get_log_level() -> u8 {
    LOG_LEVEL.load(Relaxed)
}

pub fn set_log_level(level: u8) {
    LOG_LEVEL.store(level, Relaxed);
}

macro_rules! debug {
    ( $( $args:expr ),* ) => { if crate::log::get_log_level() >= 2 { use std::io::Write; let _ = writeln!(std::io::stderr().lock(), " [.] {}", format!( $($args),* )); } }
}
macro_rules! verbose {
    ( $( $args:expr ),* ) => { if crate::log::get_log_level() >= 1 { eprintln!(" [.] {}", format!( $($args),* )); } }
}
macro_rules! info {
    ( $( $args:expr ),* ) => { eprintln!(" [.] {}", format!( $($args),* )); }
}
macro_rules! warn {
    ( $( $args:expr ),* ) => { eprintln!(" [!] {}", format!( $($args),* )); }
}

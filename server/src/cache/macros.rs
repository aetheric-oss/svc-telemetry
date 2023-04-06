/// Writes an error! message to the app::cache logger
#[macro_export]
macro_rules! cache_error {
    ($($arg:tt)+) => {
        log::error!(target: "app::cache", $($arg)+);
    };
}

/// Writes a debug! message to the app::cache logger
#[macro_export]
macro_rules! cache_debug {
    ($($arg:tt)+) => {
        log::debug!(target: "app::cache", $($arg)+);
    };
}

/// Writes a info! message to the app::cache logger
#[macro_export]
macro_rules! cache_info {
    ($($arg:tt)+) => {
        log::info!(target: "app::cache", $($arg)+);
    };
}

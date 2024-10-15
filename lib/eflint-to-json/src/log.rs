//  LOG.rs
//    by Lut99
//
//  Created:
//    11 Oct 2024, 16:05:00
//  Last edited:
//    11 Oct 2024, 16:07:32
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines macros that log in various ways.
//


/***** LIBRARY *****/
/// Depending on the backend, logs a DEBUG-level statement appropriately or not.
#[cfg(not(any(feature = "log", feature = "tracing")))]
macro_rules! debug {
    ($($t:tt)*) => {};
}
#[cfg(all(feature = "log", not(feature = "tracing")))]
macro_rules! debug {
    ($($t:tt)*) => {
        ::log::debug!($($t)*)
    };
}
#[cfg(all(not(feature = "log"), feature = "tracing"))]
macro_rules! debug {
    ($($t:tt)*) => {
        ::tracing::debug!($($t)*)
    };
}
#[cfg(all(feature = "log", feature = "tracing"))]
macro_rules! debug {
    ($($t:tt)*) => {
        {
            ::log::debug!($($t)*);
            ::tracing::debug!($($t)*);
        }
    };
}
pub(crate) use debug;

/// Depending on the backend, logs an INFO-level statement appropriately or not.
#[cfg(not(any(feature = "log", feature = "tracing")))]
macro_rules! info {
    ($($t:tt)*) => {};
}
#[cfg(all(feature = "log", not(feature = "tracing")))]
macro_rules! info {
    ($($t:tt)*) => {
        ::log::info!($($t)*)
    };
}
#[cfg(all(not(feature = "log"), feature = "tracing"))]
macro_rules! info {
    ($($t:tt)*) => {
        ::tracing::info!($($t)*)
    };
}
#[cfg(all(feature = "log", feature = "tracing"))]
macro_rules! info {
    ($($t:tt)*) => {
        {
            ::log::info!($($t)*);
            ::tracing::info!($($t)*);
        }
    };
}
pub(crate) use info;

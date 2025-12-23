#[cfg(feature = "tee")]
pub mod client;

#[cfg(feature = "tee")]
pub use client::TeeService;

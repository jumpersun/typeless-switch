pub mod codec;
#[cfg(feature = "desktop")]
pub mod desktop;
pub mod engine;
pub mod files;
pub mod platform;
pub mod vault;

pub type Result<T> = std::result::Result<T, String>;

#[cfg(feature = "desktop")]
pub mod presentation;

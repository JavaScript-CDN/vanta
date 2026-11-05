//! Launching engine process trees (file 00 §7, file 04 §11).

pub mod args;
pub mod forwarder;
pub mod prefs;
pub mod supervisor;
#[cfg(windows)] mod job;

pub use args::{LaunchSpec, build_args};
pub use forwarder::{Forwarder, Upstream, UpstreamKind};
pub use supervisor::{Supervisor, ExitEvent};

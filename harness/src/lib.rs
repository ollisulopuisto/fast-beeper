//! Test harness crate: real zapfast sources (symlinked) + minimal stubs for
//! leaf modules outside the Beeper adapter's reach. See Cargo.toml.
pub mod account;
pub mod app;
pub mod app_lock;
pub mod archive;
pub mod backend;
pub mod calls;
pub mod group_admin;
pub mod i18n;
pub mod markup;
pub mod mentions;
pub mod model;
pub mod new_group;
pub mod paths;
pub mod privacy;
pub mod safety;
pub mod settings;
pub mod ui;
pub mod updates;
pub mod util;

pub mod merged;

//! Test harness crate: zapfast sources (staged from current src/ by check-source.sh) + minimal stubs for
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
pub mod scheduler;
pub mod scheduler_client;
pub mod settings;
pub mod ui;
pub mod updates;
pub mod util;

pub mod merged;

// Credential access is outside this portable adapter harness; source sign-in harness tests it separately.
pub mod beeper_signin { pub fn stored(_: &str) -> Result<String, String> { Ok(String::new()) } }

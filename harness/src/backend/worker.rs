//! Harness stub of the WhatsApp worker: drains commands until Shutdown so
//! backend.rs's own startup-cancellation test runs unchanged. No network,
//! no storage - the Beeper adapter under test lives in beeper.rs.
use tokio::sync::mpsc;

use super::{Command, Event, Waker};
use crate::paths::AccountDirs;

pub const PINNED_CHATS: usize = 3;
pub const PLUS_PINNED_CHATS: usize = 20;

pub fn host_owned_presence() {}

pub async fn run(
    _dirs: AccountDirs,
    _events: std::sync::mpsc::Sender<Event>,
    _commands: mpsc::UnboundedSender<Command>,
    mut inbox: mpsc::UnboundedReceiver<Command>,
    _waker: Waker,
) {
    while let Some(command) = inbox.recv().await {
        if matches!(command, Command::Shutdown) {
            break;
        }
    }
}

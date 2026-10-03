//! `ripwire-broker memory …`: local operations on a workspace's store. None of them builds a
//! classifier client, starts ripwire or needs a credential (PRD jev-mem §4).

use super::identity;
use super::store::Store;
use crate::cli::{MemoryAction, MemoryCommand};
use crate::state::StateStore;

/// What to print on success, or the error for stderr.
pub fn run(cmd: &MemoryCommand) -> Result<String, String> {
    let dir = cmd
        .state_dir
        .clone()
        .or_else(StateStore::default_dir)
        .ok_or("no state directory: pass --state-dir")?;
    let store = Store::new(&dir, &identity::workspace_id(&cmd.workspace)?);
    match cmd.action {
        MemoryAction::Resume => match store.resume() {
            Ok(true) => Ok("memory collection resumed for this workspace".into()),
            Ok(false) => Ok("memory collection was not revoked for this workspace".into()),
            Err(e) => Err(format!("memory resume: {e:?}")),
        },
    }
}

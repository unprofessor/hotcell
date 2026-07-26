#![allow(dead_code)] // stub modules: unused items are wired up incrementally.

//! hotcell — an AI coding agent sandbox.
//!
//! Module layout mirrors the Allium specification:
//! - [`cell`]: Cell entity, identity, declaration, state transitions.
//! - [`session`]: Session entity and lifecycle.
//! - [`provisioning`]: the provisioning phase and its outcomes.
//! - [`isolation`]: filesystem isolation via bubblewrap.
//! - [`firewall`]: network firewall via an HTTP allowlist proxy.
//! - [`state`]: persistent cell state in the state directory.
//! - [`cellfile`]: reading the Cellfile declaration.
//! - [`cli`]: the developer-facing command-line interface.

mod cellfile;
mod cell;
mod session;
mod provisioning;
mod isolation;
mod firewall;
mod state;
mod cli;

use anyhow::Result;

fn main() -> Result<()> {
    cli::run()
}

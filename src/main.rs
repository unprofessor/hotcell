//! hotcell — an AI coding agent sandbox.
//!
//! Module layout mirrors the Allium specification:
//! - [`cell`]: Cell entity, identity, declaration, state transitions.
//! - [`plan`]: the execution plan — every command a run will execute,
//!   compiled up front and shared between display and execution.
//! - [`provisioning`]: the provisioning phase and its outcomes.
//! - [`isolation`]: filesystem isolation via bubblewrap.
//! - [`firewall`]: network firewall via an HTTP allowlist proxy.
//! - [`state`]: persistent cell state in the state directory.
//! - [`cellfile`]: reading the Cellfile declaration.
//! - [`cli`]: the developer-facing command-line interface.

mod cell;
mod cellfile;
mod cli;
mod firewall;
mod isolation;
mod plan;
mod provisioning;
mod state;

use anyhow::Result;

fn main() -> Result<()> {
    cli::run()
}

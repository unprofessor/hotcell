//! Provisioning phase and its outcomes.
//!
//! Mirrors the `ProvisionSucceeds` / `ProvisionFails` rules: the provisioning
//! process installs packages, seeds files, configures the network firewall
//! and environment, and reports success or failure. The durable state
//! transitions (marking the cell provisioned or failed on disk) are handled by
//! the [`state`](crate::state) module; this module only runs the work and
//! reports the outcome.

use crate::cell::Cell;
use crate::cellfile::CellDeclaration;

/// Outcome of a provisioning attempt.
pub enum ProvisionOutcome {
    Succeeded(CellDeclaration),
    Failed(String),
}

/// Run provisioning for a cell against its Cellfile's declaration.
///
/// TODO(v1): drive `ProvisioningProcess` — install packages, seed files,
/// configure the firewall and environment. For now this is a stub that
/// succeeds immediately with the declaration unchanged.
pub fn provision(_cell: &Cell, declaration: &CellDeclaration) -> ProvisionOutcome {
    ProvisionOutcome::Succeeded(declaration.clone())
}

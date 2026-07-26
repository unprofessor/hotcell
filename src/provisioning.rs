//! Provisioning phase and its outcomes.
//!
//! Mirrors the `ProvisionSucceeds` / `ProvisionFails` rules: the provisioning
//! process installs packages, seeds files, configures the network firewall
//! and environment, and reports success or failure.

use crate::cell::{Cell, CellStatus};
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

/// Apply a provisioning outcome to a cell's state, matching the spec rules.
pub fn apply_outcome(cell: &mut Cell, outcome: ProvisionOutcome, now: u64) -> Option<crate::session::Session> {
    match outcome {
        ProvisionOutcome::Succeeded(declaration) => {
            let program = cell.pending_program.take().unwrap_or_default();
            let args = cell.pending_args.drain(..).collect::<Vec<_>>();
            cell.status = CellStatus::Provisioned;
            cell.provisioned_as = Some(declaration);
            cell.provisioning_error = None;
            // Session is created on provisioning success.
            Some(crate::session::Session {
                program,
                args,
                status: crate::session::SessionStatus::Running,
                started_at: now,
                ended_at: None,
                exit_code: None,
                log_file: None, // set by the runtime before launch.
            })
        }
        ProvisionOutcome::Failed(error) => {
            cell.status = CellStatus::ProvisioningFailed;
            cell.provisioning_error = Some(error);
            cell.pending_program = None;
            cell.pending_args.clear();
            None
        }
    }
}

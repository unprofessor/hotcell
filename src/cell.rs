//! Cell entity, identity, and derived status.
//!
//! Mirrors the `Cell` entity from the specification. A cell's identity is the
//! pair `(Cellfile, name)`; the name defaults to "default". Status is derived
//! from the cell's durable on-disk state, never stored as a field — so it
//! cannot drift from reality if hotcell crashes mid-provisioning.

use std::path::PathBuf;

use crate::cellfile::CellDeclaration;

/// Cell status, derived from durable on-disk state.
///
/// `Provisioning` (the transient phase) is never stored — the durable
/// statuses are reconstructed from the state directory's markers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellStatus {
    Unprovisioned,
    Provisioned,
    ProvisioningFailed,
}

impl CellStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            CellStatus::Unprovisioned => "unprovisioned",
            CellStatus::Provisioned => "provisioned",
            CellStatus::ProvisioningFailed => "provisioning_failed",
        }
    }
}

/// A cell: an isolated environment identified by `(Cellfile, name)`.
///
/// This is an in-memory construct. Durable state (`provisioned_as`, `failed`)
/// is loaded from / written to the state directory by the [`state`](crate::state)
/// module; it is not serialized as a single blob.
#[derive(Debug, Clone)]
pub struct Cell {
    /// Root for cell state (`.cell/...`). The `--cell` override lands here,
    /// independently of where the Cellfile lives.
    pub cellfile_directory: PathBuf,
    /// Directory of the Cellfile itself — where the provision `script` and
    /// relative declaration paths resolve. Distinct from `cellfile_directory`
    /// when `--cell` relocates state.
    pub file_directory: PathBuf,
    pub name: String,

    /// The declaration the cell was last provisioned with. `None` unless the
    /// cell is provisioned. Loaded from `provisioned_as.json`.
    pub provisioned_as: Option<CellDeclaration>,

    /// SHA-256 digest of the provisioning script the cell was last provisioned
    /// with (hex), recorded in `provisioned_as.json`. `None` for provisioners
    /// with no script (e.g. `kind = "none"`) or for records written before
    /// digests were tracked. Used to skip re-provisioning when the script is
    /// unchanged.
    pub provisioned_script_digest: Option<String>,

    /// True when the last provisioning attempt failed and no provisioned
    /// environment exists. Derived from the presence of a `failed` marker.
    pub failed: bool,

    // The fields below are in-process state for the current invocation only.
    // hotcell holds the program/args in memory across the provisioning phase;
    // they are never persisted.
    pub pending_program: Option<String>,
    pub pending_args: Vec<String>,
}

impl Cell {
    /// Status derived from durable state, not stored.
    pub fn status(&self) -> CellStatus {
        if self.provisioned_as.is_some() {
            CellStatus::Provisioned
        } else if self.failed {
            CellStatus::ProvisioningFailed
        } else {
            CellStatus::Unprovisioned
        }
    }

    /// True when the provisioned environment matches the Cellfile's current
    /// declaration. `is_current` in the spec.
    pub fn is_current(&self, current: &CellDeclaration) -> bool {
        self.provisioned_as.as_ref() == Some(current)
    }
}

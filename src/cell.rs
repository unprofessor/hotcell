//! Cell entity, identity, and derived status.
//!
//! Mirrors the `Cell` entity from the specification. A cell's identity is the
//! pair `(Cellfile, name)`; the name defaults to "default". Status is derived
//! from the cell's durable on-disk state, never stored as a field — so it
//! cannot drift from reality if hotcell crashes mid-provisioning.

use std::path::PathBuf;

use crate::cellfile::CellDeclaration;

/// Cell status, matching the spec's transition graph.
///
/// `Provisioning` is purely transient — it is only true while a `hotcell run`
/// invocation is in the provisioning phase, and is never persisted. The
/// durable statuses are derived from the state directory's contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellStatus {
    Unprovisioned,
    Provisioning,
    Provisioned,
    ProvisioningFailed,
}

impl CellStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            CellStatus::Unprovisioned => "unprovisioned",
            CellStatus::Provisioning => "provisioning",
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
    pub cellfile_directory: PathBuf,
    pub name: String,

    /// The declaration the cell was last provisioned with. `None` unless the
    /// cell is provisioned. Loaded from `provisioned_as.json`.
    pub provisioned_as: Option<CellDeclaration>,

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
    pub fn new(cellfile_directory: PathBuf, name: String) -> Self {
        Self {
            cellfile_directory,
            name,
            provisioned_as: None,
            failed: false,
            pending_program: None,
            pending_args: Vec::new(),
        }
    }

    /// Status derived from durable state, not stored. `Provisioning` is never
    /// returned here — it is a transient phase tracked only within a live
    /// `run` invocation.
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

//! Cell entity, identity, and state transitions.
//!
//! Mirrors the `Cell` entity from the specification. A cell's identity is the
//! pair `(Cellfile, name)`; the name defaults to "default".

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::cellfile::{CellDeclaration, Cellfile};

/// Cell status, matching the spec's transition graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cell {
    pub cellfile_directory: PathBuf,
    pub name: String,
    pub status: CellStatus,

    /// The declaration the cell was last provisioned with. Present only when
    /// `status == Provisioned`.
    pub provisioned_as: Option<CellDeclaration>,

    // The fields below are in-process state for the current invocation only.
    // hotcell holds the program/args in memory across the provisioning phase;
    // the provisioning error is surfaced on the console at failure time. None
    // of these are persisted.
    #[serde(skip)]
    pub pending_program: Option<String>,
    #[serde(skip)]
    pub pending_args: Vec<String>,
    #[serde(skip)]
    pub provisioning_error: Option<String>,
}

impl Cell {
    pub fn new(cellfile: &Cellfile, name: &str) -> Self {
        Self {
            cellfile_directory: cellfile.directory.clone(),
            name: name.to_string(),
            status: CellStatus::Unprovisioned,
            provisioned_as: None,
            pending_program: None,
            pending_args: Vec::new(),
            provisioning_error: None,
        }
    }

    /// True when the provisioned environment matches the Cellfile's current
    /// declaration. `is_current` in the spec.
    pub fn is_current(&self, current: &CellDeclaration) -> bool {
        self.provisioned_as.as_ref() == Some(current)
    }
}

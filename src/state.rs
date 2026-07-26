//! Persistent cell state in the state directory.
//!
//! Mirrors the `StateColocatedWithCellfile` guarantee: cell state persists in
//! `{directory}/{state_dir}` alongside the Cellfile, so it follows the
//! Cellfile across directory renames. A cell's identity is `(Cellfile, name)`,
//! not its directory path.
//!
//! There is no single `cell.json` blob. Durable state is represented by the
//! presence of files within the cell's state directory, so status is always
//! derivable from what is actually on disk:
//!
//! - `provisioned_as.json` present  → provisioned
//! - `failed` marker present         → provisioning_failed
//! - neither                         → unprovisioned
//!
//! `provisioning` is never persisted; it is a transient phase within a live
//! `hotcell run` invocation. If hotcell crashes, the cell reverts to whatever
//! durable state remains.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cell::Cell;
use crate::cellfile::{CellDeclaration, Cellfile};

/// Config defaults from the specification.
pub const DEFAULT_CELL_NAME: &str = "default";
pub const STATE_DIR: &str = ".cell";
pub const DEFAULT_PROVISIONER_KIND: &str = "none";

const PROVISIONED_AS_FILE: &str = "provisioned_as.json";
const FAILED_MARKER: &str = "failed";
const FS_DIR: &str = "fs";

/// A cell's state directory: `{cellfile_directory}/{STATE_DIR}/{name}`.
pub fn cell_state_dir(cellfile_directory: &Path, name: &str) -> PathBuf {
    cellfile_directory.join(STATE_DIR).join(name)
}

/// The cell's sandbox rootfs backing directory: `{state_dir}/fs`. This is
/// bind-mounted as `/` inside the sandbox, so everything the provisioner
/// writes here is visible to the agent. State markers (`provisioned_as.json`,
/// `failed`, `session.log`) live in the state directory *outside* `fs`, so
/// the agent cannot see or tamper with them.
pub fn fs_dir(cellfile_directory: &Path, name: &str) -> PathBuf {
    cell_state_dir(cellfile_directory, name).join(FS_DIR)
}

/// Path for a session's log file, derived from the cell's state directory.
pub fn log_file_path(cellfile_directory: &Path, name: &str) -> PathBuf {
    cell_state_dir(cellfile_directory, name).join("session.log")
}

/// The on-disk shape of `provisioned_as.json`: the declaration the cell was
/// last provisioned with, plus a digest of the provisioning inputs used at
/// that time. The digest lets a subsequent run skip re-provisioning when
/// nothing has changed (see [`crate::provisioning`]).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProvisionedRecord {
    declaration: CellDeclaration,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    script_digest: Option<String>,
}

/// Load a cell by `(Cellfile, name)`, deriving its durable state from the
/// state directory. Returns a cell with `provisioned_as = None` and
/// `failed = false` (i.e. unprovisioned) if no state exists yet.
///
/// For backward compatibility, a `provisioned_as.json` that is a bare
/// `CellDeclaration` (written by older hotcell versions, before the digest
/// was recorded) is accepted and treated as having no recorded digest — so
/// the next run re-provisions once and then records a digest.
pub fn load_cell(cellfile: &Cellfile, name: &str) -> anyhow::Result<Cell> {
    let dir = cell_state_dir(&cellfile.directory, name);
    let (provisioned_as, script_digest) = if dir.join(PROVISIONED_AS_FILE).exists() {
        let data = std::fs::read(dir.join(PROVISIONED_AS_FILE))?;
        // Current format: `{ declaration, script_digest }`. Fall back to the
        // legacy bare-declaration format so existing cells re-provision once
        // rather than crashing.
        match serde_json::from_slice::<ProvisionedRecord>(&data) {
            Ok(record) => (Some(record.declaration), record.script_digest),
            Err(_) => {
                let decl = serde_json::from_slice::<CellDeclaration>(&data)?;
                (Some(decl), None)
            }
        }
    } else {
        (None, None)
    };
    let failed = dir.join(FAILED_MARKER).exists();
    Ok(Cell {
        cellfile_directory: cellfile.directory.clone(),
        name: name.to_string(),
        provisioned_as,
        provisioned_script_digest: script_digest,
        failed,
        pending_program: None,
        pending_args: Vec::new(),
    })
}

/// Enter the provisioning phase: clear both the provisioned declaration and
/// the failure marker. A crash during provisioning then leaves the cell
/// unprovisioned rather than stuck in a false state.
pub fn begin_provisioning(cellfile_directory: &Path, name: &str) -> anyhow::Result<()> {
    let dir = cell_state_dir(cellfile_directory, name);
    std::fs::create_dir_all(&dir)?;
    remove_if_exists(dir.join(PROVISIONED_AS_FILE))?;
    remove_if_exists(dir.join(FAILED_MARKER))?;
    Ok(())
}

/// Record a successful provisioning: write the declaration (and the digest
/// of the provisioning inputs, when known) and clear any prior failure
/// marker. The digest is stored inside `provisioned_as.json` alongside the
/// declaration so a later run can skip re-provisioning when nothing changed.
pub fn mark_provisioned(
    cellfile_directory: &Path,
    name: &str,
    declaration: &CellDeclaration,
    script_digest: Option<&str>,
) -> anyhow::Result<()> {
    let dir = cell_state_dir(cellfile_directory, name);
    std::fs::create_dir_all(&dir)?;
    let record = ProvisionedRecord {
        declaration: declaration.clone(),
        script_digest: script_digest.map(str::to_string),
    };
    let data = serde_json::to_vec_pretty(&record)?;
    std::fs::write(dir.join(PROVISIONED_AS_FILE), data)?;
    remove_if_exists(dir.join(FAILED_MARKER))?;
    Ok(())
}

/// Record a provisioning failure: write the failure marker and clear any
/// prior provisioned declaration.
pub fn mark_failed(cellfile_directory: &Path, name: &str) -> anyhow::Result<()> {
    let dir = cell_state_dir(cellfile_directory, name);
    std::fs::create_dir_all(&dir)?;
    remove_if_exists(dir.join(PROVISIONED_AS_FILE))?;
    std::fs::write(dir.join(FAILED_MARKER), "")?;
    Ok(())
}

/// Destroy a cell's state on disk, returning it to unprovisioned.
///
/// Mirrors `DestroyCell`: tear down the provisioned filesystem by removing
/// the state directory entirely. The cell's identity is retained by the
/// directory structure, so a subsequent run recreates it.
pub fn destroy_cell(cellfile_directory: &Path, name: &str) -> anyhow::Result<()> {
    let dir = cell_state_dir(cellfile_directory, name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    Ok(())
}

/// List cell names recorded under a directory's state root.
pub fn list_cell_names(cellfile_directory: &Path) -> anyhow::Result<Vec<String>> {
    let state_root = cellfile_directory.join(STATE_DIR);
    if !state_root.exists() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in std::fs::read_dir(&state_root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    Ok(names)
}

fn remove_if_exists(path: PathBuf) -> anyhow::Result<()> {
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

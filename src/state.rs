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

/// Load a cell by `(Cellfile, name)`, deriving its durable state from the
/// state directory. Returns a cell with `provisioned_as = None` and
/// `failed = false` (i.e. unprovisioned) if no state exists yet.
pub fn load_cell(cellfile: &Cellfile, name: &str) -> anyhow::Result<Cell> {
    let dir = cell_state_dir(&cellfile.directory, name);
    let provisioned_as = if dir.join(PROVISIONED_AS_FILE).exists() {
        let data = std::fs::read(dir.join(PROVISIONED_AS_FILE))?;
        Some(serde_json::from_slice::<CellDeclaration>(&data)?)
    } else {
        None
    };
    let failed = dir.join(FAILED_MARKER).exists();
    Ok(Cell {
        cellfile_directory: cellfile.directory.clone(),
        name: name.to_string(),
        provisioned_as,
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

/// Record a successful provisioning: write the declaration and clear any
/// prior failure marker.
pub fn mark_provisioned(
    cellfile_directory: &Path,
    name: &str,
    declaration: &CellDeclaration,
) -> anyhow::Result<()> {
    let dir = cell_state_dir(cellfile_directory, name);
    std::fs::create_dir_all(&dir)?;
    let data = serde_json::to_vec_pretty(declaration)?;
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

//! Persistent cell state in the state directory.
//!
//! Mirrors the `StateColocatedWithCellfile` guarantee: cell state persists in
//! `{directory}/{state_dir}` alongside the Cellfile, so it follows the
//! Cellfile across directory renames. A cell's identity is `(Cellfile, name)`,
//! not its directory path.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cell::{Cell, CellStatus};
use crate::cellfile::Cellfile;

/// Config defaults from the specification.
pub const DEFAULT_WORKDIR: &str = "/work";
pub const DEFAULT_CELL_NAME: &str = "default";
pub const STATE_DIR: &str = ".cell";

/// A cell's state directory: `{cellfile_directory}/{STATE_DIR}/{name}`.
pub fn cell_state_dir(cellfile_directory: &Path, name: &str) -> PathBuf {
    cellfile_directory.join(STATE_DIR).join(name)
}

/// The on-disk record for a cell.
#[derive(Debug, Serialize, Deserialize)]
pub struct CellRecord {
    pub cell: Cell,
}

/// Load a cell by `(Cellfile, name)` from its state directory, if it exists.
pub fn load_cell(cellfile: &Cellfile, name: &str) -> anyhow::Result<Option<Cell>> {
    let dir = cell_state_dir(&cellfile.directory, name);
    let record_path = dir.join("cell.json");
    if !record_path.exists() {
        return Ok(None);
    }
    let data = std::fs::read(&record_path)?;
    let record: CellRecord = serde_json::from_slice(&data)?;
    Ok(Some(record.cell))
}

/// Persist a cell to its state directory.
pub fn save_cell(cell: &Cell) -> anyhow::Result<()> {
    let dir = cell_state_dir(&cell.cellfile_directory, &cell.name);
    std::fs::create_dir_all(&dir)?;
    let record = CellRecord { cell: cell.clone() };
    let data = serde_json::to_vec_pretty(&record)?;
    std::fs::write(dir.join("cell.json"), data)?;
    Ok(())
}

/// Path for a session's log file, derived from the cell's state directory.
pub fn log_file_path(cell: &Cell) -> PathBuf {
    cell_state_dir(&cell.cellfile_directory, &cell.name).join("session.log")
}

/// Destroy a cell's state on disk, returning it to unprovisioned.
///
/// Mirrors `DestroyCell`: tear down the provisioned filesystem. The cell
/// record itself is retained (now unprovisioned) so its identity persists.
pub fn destroy_cell(cell: &mut Cell) -> anyhow::Result<()> {
    cell.status = CellStatus::Unprovisioned;
    cell.provisioned_as = None;
    cell.provisioning_error = None;
    save_cell(cell)
}

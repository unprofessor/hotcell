//! Developer-facing command-line interface.
//!
//! Mirrors the `DeveloperCLI` surface: status discovery is per-directory
//! (scoped to `$PWD/Cellfile`), and the developer can run and destroy cells.
//! `hotcell run` blocks until the program exits, relaying stdio faithfully
//! and passing the exit code through.

use std::path::PathBuf;
use std::process::Stdio;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};

use crate::cell::{Cell, CellStatus};
use crate::cellfile::Cellfile;
use crate::provisioning;
use crate::state;

#[derive(Parser)]
#[command(name = "hotcell", version, about = "An AI coding agent sandbox")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a program inside a cell, provisioning first if needed.
    Run {
        /// Cell name. Defaults to "default".
        #[arg(long, default_value = state::DEFAULT_CELL_NAME)]
        name: String,
        /// Program to execute inside the cell.
        program: String,
        /// Arguments to pass to the program.
        args: Vec<String>,
    },
    /// Destroy a cell's provisioned filesystem, returning it to unprovisioned.
    Destroy {
        /// Cell name. Defaults to "default".
        #[arg(long, default_value = state::DEFAULT_CELL_NAME)]
        name: String,
    },
    /// List cells for the Cellfile in the current directory.
    Status,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Run { name, program, args } => run_cell(&name, &program, &args),
        Command::Destroy { name } => destroy_cell(&name),
        Command::Status => status(),
    }
}

fn current_cellfile() -> Result<Cellfile> {
    let cwd = std::env::current_dir()?;
    Cellfile::read(&cwd)
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn run_cell(name: &str, program: &str, args: &[String]) -> Result<()> {
    let cellfile = current_cellfile()?;

    // Resolve or create the cell, then drive it into the provisioning phase.
    // Mirrors CreateCellForProvisioning / Reprovision* rules.
    let mut cell = match state::load_cell(&cellfile, name)? {
        Some(c) => c,
        None => Cell::new(&cellfile, name),
    };

    let can_reprovision = matches!(
        cell.status,
        CellStatus::Unprovisioned | CellStatus::ProvisioningFailed | CellStatus::Provisioned
    );
    if !can_reprovision {
        bail!(
            "cell {:?} is already provisioning; wait for it to finish",
            name
        );
    }

    cell.status = CellStatus::Provisioning;
    cell.provisioned_as = None;
    cell.provisioning_error = None;
    cell.pending_program = Some(program.to_string());
    cell.pending_args = args.to_vec();
    state::save_cell(&cell)?;

    // Provisioning phase.
    let outcome = provisioning::provision(&cell, &cellfile.declaration);
    let now = now_unix();
    let mut session = match provisioning::apply_outcome(&mut cell, outcome, now) {
        Some(s) => s,
        None => {
            state::save_cell(&cell)?;
            let err = cell.provisioning_error.clone().unwrap_or_default();
            eprintln!("provisioning failed: {err}");
            std::process::exit(1);
        }
    };
    state::save_cell(&cell)?;

    // Launch the agent inside the isolated sandbox, relaying stdio.
    // Mirrors RelayOnly / CleanEnvironment guarantees.
    let cell_root = state::cell_state_dir(&cellfile.directory, name);
    std::fs::create_dir_all(&cell_root)?;
    session.log_file = Some(state::log_file_path(&cell).to_string_lossy().into_owned());

    let env: Vec<(String, String)> = cell
        .provisioned_as
        .as_ref()
        .map(|d| {
            d.environment
                .iter()
                .map(|v| (v.key.clone(), v.value.clone()))
                .collect()
        })
        .unwrap_or_default();

    // Workdir from the provisioned declaration, falling back to the Cellfile's
    // directory when unset (empty). The spec's config default is "/work", but
    // that is the in-sandbox path; the host-side source is the cellfile dir.
    let workdir = cell
        .provisioned_as
        .as_ref()
        .map(|d| d.workdir.clone())
        .filter(|w| !w.is_empty())
        .unwrap_or_else(|| cellfile.directory.to_string_lossy().into_owned());

    let mut cmd = crate::isolation::build_command(&cell_root, &workdir, &env, program, args);
    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let mut child = cmd.spawn()?;
    let exit_status = child.wait()?;

    // Session completes when the program exits (SessionCompletes rule).
    session.status = crate::session::SessionStatus::Completed;
    session.ended_at = Some(now_unix());
    session.exit_code = Some(exit_status.code().unwrap_or(-1));
    session.log_file = None;

    if !exit_status.success() {
        eprintln!(
            "session completed with errors: exit code {}",
            exit_status.code().unwrap_or(-1)
        );
    }

    let code = exit_status.code().unwrap_or(-1);
    std::process::exit(code);
}

fn destroy_cell(name: &str) -> Result<()> {
    let cellfile = current_cellfile()?;
    let mut cell = match state::load_cell(&cellfile, name)? {
        Some(c) => c,
        None => bail!("no cell named {:?} in {}", name, cellfile.directory.display()),
    };
    // Mirrors DestroyCell: only provisioned or failed cells with no active
    // sessions can be destroyed.
    if !matches!(cell.status, CellStatus::Provisioned | CellStatus::ProvisioningFailed) {
        bail!("cell {:?} is not in a destroyable state ({})", name, cell.status.as_str());
    }
    state::destroy_cell(&mut cell)?;
    println!("destroyed cell {:?}", name);
    Ok(())
}

fn status() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let state_root = cwd.join(state::STATE_DIR);
    if !state_root.exists() {
        println!("no cells in {}", cwd.display());
        return Ok(());
    }
    for entry in std::fs::read_dir(&state_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let _name = entry.file_name().to_string_lossy().into_owned();
        let dir: PathBuf = entry.path();
        let record_path = dir.join("cell.json");
        if !record_path.exists() {
            continue;
        }
        let data = std::fs::read(&record_path)?;
        let record: state::CellRecord = serde_json::from_slice(&data)?;
        let cell = record.cell;
        let cellfile = Cellfile {
            directory: cwd.clone(),
            declaration: Cellfile::read(&cwd)
                .map(|c| c.declaration)
                .unwrap_or_default(),
        };
        let current = cell.is_current(&cellfile.declaration);
        println!(
            "{:<16} {:<20} {}",
            cell.name,
            cell.status.as_str(),
            if current { "current" } else { "stale" }
        );
    }
    Ok(())
}

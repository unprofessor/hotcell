//! Developer-facing command-line interface.
//!
//! Mirrors the `DeveloperCLI` surface: status discovery is per-directory
//! (scoped to `$PWD/Cellfile`), and the developer can run and destroy cells.
//! `hotcell run` blocks until the program exits, relaying stdio faithfully
//! and passing the exit code through.

use std::process::Stdio;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};

use crate::cell::CellStatus;
use crate::cellfile::Cellfile;
use crate::provisioning::{provision, ProvisionOutcome};
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
    let mut cell = state::load_cell(&cellfile, name)?;

    // Carry the program/args through the provisioning phase in memory.
    cell.pending_program = Some(program.to_string());
    cell.pending_args = args.to_vec();

    // Enter the provisioning phase: clear durable markers so a crash leaves
    // the cell unprovisioned rather than stuck.
    state::begin_provisioning(&cellfile.directory, name)?;

    // Provisioning phase.
    let outcome = provision(&cell, &cellfile.declaration);
    let declaration = match outcome {
        ProvisionOutcome::Succeeded(d) => d,
        ProvisionOutcome::Failed(err) => {
            state::mark_failed(&cellfile.directory, name)?;
            eprintln!("provisioning failed: {err}");
            std::process::exit(1);
        }
    };
    state::mark_provisioned(&cellfile.directory, name, &declaration)?;
    cell.provisioned_as = Some(declaration);

    // Launch the agent inside the isolated sandbox, relaying stdio.
    // Mirrors RelayOnly / CleanEnvironment guarantees.
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

    let cell_root = state::cell_state_dir(&cellfile.directory, name);
    std::fs::create_dir_all(&cell_root)?;
    let log_file = state::log_file_path(&cellfile.directory, name);

    let mut cmd = crate::isolation::build_command(&cell_root, &workdir, &env, program, args);
    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    eprintln!("session log: {}", log_file.display());

    let mut child = cmd.spawn()?;
    let exit_status = child.wait()?;

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
    let cell = state::load_cell(&cellfile, name)?;
    // Mirrors DestroyCell: only provisioned or failed cells can be destroyed.
    if !matches!(cell.status(), CellStatus::Provisioned | CellStatus::ProvisioningFailed) {
        bail!(
            "cell {:?} is not in a destroyable state ({})",
            name,
            cell.status().as_str()
        );
    }
    state::destroy_cell(&cellfile.directory, name)?;
    println!("destroyed cell {:?}", name);
    Ok(())
}

fn status() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let cellfile = Cellfile::read(&cwd)?;
    let names = state::list_cell_names(&cwd)?;
    if names.is_empty() {
        println!("no cells in {}", cwd.display());
        return Ok(());
    }
    for name in names {
        let cell = state::load_cell(&cellfile, &name)?;
        let current = cell.is_current(&cellfile.declaration);
        println!(
            "{:<16} {:<20} {}",
            cell.name,
            cell.status().as_str(),
            if current { "current" } else { "stale" }
        );
    }
    Ok(())
}

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
        /// Path to the Cellfile, overriding the conventional `$PWD/Cellfile`.
        /// The cell's state directory lives alongside the given file.
        #[arg(short = 'f', long = "file", value_name = "CELLFILE")]
        file: Option<String>,
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
        Command::Run { name, file, program, args } => {
            run_cell(&name, file.as_deref(), &program, &args)
        }
        Command::Destroy { name } => destroy_cell(&name),
        Command::Status => status(),
    }
}

fn current_cellfile(override_path: Option<&str>) -> Result<Cellfile> {
    match override_path {
        Some(p) => {
            let path = std::path::Path::new(p);
            Ok(Cellfile::read_at(path)?)
        }
        None => {
            let cwd = std::env::current_dir()?;
            Ok(Cellfile::read(&cwd)?)
        }
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn run_cell(name: &str, cellfile_path: Option<&str>, program: &str, args: &[String]) -> Result<()> {
    let cellfile = current_cellfile(cellfile_path)?;

    let declaration = cellfile.declaration.clone();

    let mut cell = state::load_cell(&cellfile, name)?;

    // Carry the program/args through the provisioning phase in memory.
    cell.pending_program = Some(program.to_string());
    cell.pending_args = args.to_vec();

    // Enter the provisioning phase: clear durable markers so a crash leaves
    // the cell unprovisioned rather than stuck.
    state::begin_provisioning(&cellfile.directory, name)?;

    // Provisioning phase: the provisioner seeds the
    // cell's rootfs. Mirrors ProvisionSucceeds / ProvisionFails and the
    // ProvisioningLogsToConsole guarantee.
    let outcome = provision(&cell, &declaration)?;
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
    // Mirrors RelayOnly / CleanEnvironment / ProvisionedEnvironment.
    let provisioned = cell.provisioned_as.as_ref().expect("just provisioned");
    let env: Vec<(String, String)> = provisioned
        .environment
        .iter()
        .map(|v| (v.key.clone(), v.value.clone()))
        .collect();

    // In-sandbox workdir from the provisioned declaration; config default is
    // "/work". The host-side backing lives under the cell's rootfs and is
    // created by the provisioner (or the safety-net below).
    let workdir = if provisioned.workdir.is_empty() {
        "/work".to_string()
    } else {
        provisioned.workdir.clone()
    };

    let cell_fs = state::fs_dir(&cellfile.directory, name);
    // Safety net: ensure the workdir exists even if a provisioner forgot to.
    let workdir_host = crate::provisioning::workdir_host_path(&cell_fs, &workdir);
    std::fs::create_dir_all(&workdir_host)?;

    // Network firewall (HTTP allowlist proxy) is not yet implemented. Until it
    // is, refuse to run cells that declare a non-empty network policy rather
    // than silently granting full egress — the agent profile is offline.
    if !provisioned.network.allowed_endpoints.is_empty() {
        state::mark_failed(&cellfile.directory, name)?;
        eprintln!(
            "network policy with allowed endpoints is not yet supported (firewall unimplemented); cell declared {} endpoint(s)",
            provisioned.network.allowed_endpoints.len()
        );
        std::process::exit(1);
    }

    let log_file = state::log_file_path(&cellfile.directory, name);

    let mut cmd = crate::isolation::build_agent_command(
        &cell_fs, &workdir, &env, program, args,
    );
    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    eprintln!("session log: {}", log_file.display());

    let mut child = cmd.spawn()?;
    let exit_status = child.wait()?
;

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
    let cellfile = current_cellfile(None)?;
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

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
        /// Override the declared provisioner for this run, as `kind[:file]`.
        ///
        /// `kind` is a provisioner kind (e.g. `none`, `shell`). For `shell`,
        /// append `:path` to name the script (relative to the Cellfile
        /// directory), e.g. `--provision shell:provision.sh`. For `none`, the
        /// `:file` part is omitted. Overrides `provision.type` /
        /// `provision.script` in the Cellfile for this invocation only.
        #[arg(long, value_name = "KIND[:FILE]")]
        provision: Option<String>,
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
        Command::Run { name, provision, program, args } => {
            run_cell(&name, provision.as_deref(), &program, &args)
        }
        Command::Destroy { name } => destroy_cell(&name),
        Command::Status => status(),
    }
}

fn current_cellfile() -> Result<Cellfile> {
    let cwd = std::env::current_dir()?;
    Cellfile::read(&cwd)
}

/// Parse a `--provision` override value of the form `kind[:file]` and apply
/// it to an existing provisioner, preserving `host_paths` (which are a
/// Cellfile-declared bootstrap allowance, not something the CLI override
/// touches).
///
/// `none` or `none:` → no-op provisioner. `shell:./provision.sh` → shell
/// provisioner with the given script path (kept as-is; resolved relative to
/// the Cellfile directory by the provisioner). `shell` alone is accepted but
/// will fail later if the shell kind requires a path and none is set.
fn apply_provision_override(
    spec: &str,
    base: &crate::cellfile::Provisioner,
) -> Result<crate::cellfile::Provisioner> {
    use crate::cellfile::Provisioner;

    let (kind, file) = match spec.split_once(':') {
        Some((k, f)) => (k.to_string(), if f.is_empty() { None } else { Some(f.to_string()) }),
        None => (spec.to_string(), None),
    };

    if kind.is_empty() {
        bail!("--provision value must start with a kind, got `{spec}`");
    }

    Ok(Provisioner {
        kind,
        script: file,
        // host_paths are a Cellfile-declared bootstrap allowance; the CLI
        // override only selects which provisioner runs, not what it may read.
        host_paths: base.host_paths.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::apply_provision_override;
    use crate::cellfile::Provisioner;

    fn base_with_paths() -> Provisioner {
        Provisioner {
            kind: "none".into(),
            script: None,
            host_paths: vec!["~/.nvm".into()],
        }
    }

    #[test]
    fn override_none() {
        let p = apply_provision_override("none", &base_with_paths()).unwrap();
        assert_eq!(p.kind, "none");
        assert_eq!(p.script, None);
        assert_eq!(p.host_paths, vec!["~/.nvm"]); // preserved
    }

    #[test]
    fn override_none_with_colon() {
        let p = apply_provision_override("none:", &base_with_paths()).unwrap();
        assert_eq!(p.kind, "none");
        assert_eq!(p.script, None);
    }

    #[test]
    fn override_script_with_file() {
        let p = apply_provision_override("shell:./provision.sh", &base_with_paths()).unwrap();
        assert_eq!(p.kind, "shell");
        assert_eq!(p.script.as_deref(), Some("./provision.sh"));
        assert_eq!(p.host_paths, vec!["~/.nvm"]); // preserved
    }

    #[test]
    fn override_script_without_file() {
        // Accepted; select_provisioner will error later if shell requires a path.
        let p = apply_provision_override("shell", &base_with_paths()).unwrap();
        assert_eq!(p.kind, "shell");
        assert_eq!(p.script, None);
    }

    #[test]
    fn override_rejects_empty_kind() {
        assert!(apply_provision_override(":file", &base_with_paths()).is_err());
    }

    #[test]
    fn override_unknown_kind_is_accepted_here() {
        // Kind validation happens in select_provisioner, not the parser.
        let p = apply_provision_override("frobnicate:f.sh", &base_with_paths()).unwrap();
        assert_eq!(p.kind, "frobnicate");
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn run_cell(name: &str, provision_override: Option<&str>, program: &str, args: &[String]) -> Result<()> {
    let cellfile = current_cellfile()?;

    // Apply a CLI provisioner override (if any) before provisioning. The
    // override is a one-shot deviation from the Cellfile's declared
    // provisioner for this invocation; the recorded `provisioned_as` reflects
    // the override, so a subsequent run without it will re-provision.
    let mut declaration = cellfile.declaration.clone();
    if let Some(spec) = provision_override {
        declaration.provisioner = apply_provision_override(spec, &cellfile.declaration.provisioner)?;
    }

    let mut cell = state::load_cell(&cellfile, name)?;

    // Carry the program/args through the provisioning phase in memory.
    cell.pending_program = Some(program.to_string());
    cell.pending_args = args.to_vec();

    // Enter the provisioning phase: clear durable markers so a crash leaves
    // the cell unprovisioned rather than stuck.
    state::begin_provisioning(&cellfile.directory, name)?;

    // Provisioning phase: the (possibly overridden) provisioner seeds the
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

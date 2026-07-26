//! Provisioning phase and its outcomes.
//!
//! Mirrors the `ProvisionSucceeds` / `ProvisionFails` rules and the
//! `ProvisioningProcess` external entity: a provisioner installs packages,
//! seeds files, configures the network firewall and environment, and reports
//! success or failure. The provisioner is **pluggable** — the Cellfile
//! declares which kind to use, and [`select_provisioner`] builds the right
//! one. The durable state transitions (marking the cell provisioned or
//! failed on disk) are handled by the [`state`](crate::state) module; a
//! provisioner only runs the work and reports the outcome.
//!
//! The provisioner **runs inside the cell's sandbox** under the *provisioning
//! profile* (see [`crate::isolation`]): the cell rootfs is bind-mounted as
//! `/`, and the provisioner gets read-only access to the Cellfile directory
//! and the declared host bootstrap paths, plus network egress. This is a
//! distinct, broader risk profile than the agent's cleanroom profile — the
//! provisioner is trusted, developer-authored code that may bootstrap; the
//! agent is not. Mirrors `ProvisionerRunsInCell`, `BootstrapRiskProfile`,
//! `ProvisionerCannotWriteHost`, and `ProvisionerCleanEnvironment`.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::cell::Cell;
use crate::cellfile::{CellDeclaration, Provisioner as ProvisionerSpec};
use crate::isolation;
use crate::state;

/// Outcome of a provisioning attempt.
pub enum ProvisionOutcome {
    Succeeded(CellDeclaration),
    Failed(String),
}

/// Everything a provisioner needs to bring the cell's environment into being.
pub struct ProvisionContext {
    /// The cell's sandbox rootfs backing directory (`fs_dir`), bind-mounted as
    /// `/` inside the provisioning sandbox. The provisioner writes tools,
    /// files, and the working directory here.
    pub cell_root: PathBuf,
    /// The Cellfile's directory. Bind-mounted read-only inside the
    /// provisioning sandbox at the same path, so the provision script and any
    /// project-relative seeds resolve.
    pub cellfile_directory: PathBuf,
    /// The in-sandbox working directory path (e.g. `/work`).
    pub workdir: String,
    /// The declared environment variables -- the *agent's* environment.
    /// These are NOT passed to the provisioner; the provisioner inherits the
    /// host environment instead. Kept here so the caller can thread them
    /// through to the agent profile after provisioning.
    pub environment: Vec<(String, String)>,
    /// Host paths the provisioner may read (read-only) for bootstrapping.
    /// Bind-mounted at the same path inside the provisioning sandbox.
    pub host_paths: Vec<String>,
}

/// A pluggable provisioner. Implementations bring a cell's environment into
/// being and report the outcome.
///
/// The declaration is passed in because the provisioner echoes it back
/// unchanged on success — the declaration is authoritative for the agent's
/// view (environment, network, workdir); a provisioner only seeds the
/// filesystem. (See the open question on whether provisioners may mutate the
/// declaration; v1 assumes not.)
pub trait Provisioner {
    fn provision(&self, ctx: &ProvisionContext, declaration: &CellDeclaration) -> ProvisionOutcome;
}

/// Resolve the in-sandbox `workdir` to its host-side backing path under the
/// cell root. Used when preparing the rootfs from the host side (e.g. the
/// no-op provisioner creating an empty workdir).
pub fn workdir_host_path(cell_root: &Path, workdir: &str) -> PathBuf {
    let stripped = workdir.strip_prefix('/').unwrap_or(workdir);
    cell_root.join(stripped)
}

/// Select a provisioner implementation for the declared spec.
pub fn select_provisioner(spec: &ProvisionerSpec) -> anyhow::Result<Box<dyn Provisioner>> {
    match spec.kind.as_str() {
        state::DEFAULT_PROVISIONER_KIND => Ok(Box::new(NoneProvisioner)),
        "script" => {
            let script = spec.script.as_ref().ok_or_else(|| {
                anyhow::anyhow!("script provisioner requires `provision.script` in the Cellfile")
            })?;
            Ok(Box::new(ScriptProvisioner {
                script: PathBuf::from(script),
            }))
        }
        other => Err(anyhow::anyhow!("unknown provisioner kind: {other}")),
    }
}

/// Run provisioning for a cell against its Cellfile's declaration.
///
/// Builds the provisioner from the declaration, prepares the rootfs, runs it,
/// and reports the outcome. The caller is responsible for persisting the
/// outcome via the [`state`](crate::state) module.
pub fn provision(cell: &Cell, declaration: &CellDeclaration) -> anyhow::Result<ProvisionOutcome> {
    let provisioner = select_provisioner(&declaration.provisioner)?;

    let cell_root = state::fs_dir(&cell.cellfile_directory, &cell.name);
    std::fs::create_dir_all(&cell_root)?;

    let workdir = if declaration.workdir.is_empty() {
        "/work"
    } else {
        declaration.workdir.as_str()
    }
    .to_string();

    let ctx = ProvisionContext {
        cell_root,
        cellfile_directory: cell.cellfile_directory.clone(),
        workdir,
        environment: declaration
            .environment
            .iter()
            .map(|v| (v.key.clone(), v.value.clone()))
            .collect(),
        host_paths: declaration.provisioner.host_paths.clone(),
    };

    Ok(provisioner.provision(&ctx, declaration))
}

/// The no-op provisioner (`kind = "none"`). It creates an empty working
/// directory on the host side and succeeds immediately. There is no process
/// to run inside the sandbox — "none" means no provisioning work. Useful for
/// cells that need nothing provisioned (e.g. running a base-system program in
/// isolation).
pub struct NoneProvisioner;

impl Provisioner for NoneProvisioner {
    fn provision(&self, ctx: &ProvisionContext, declaration: &CellDeclaration) -> ProvisionOutcome {
        let workdir_path = workdir_host_path(&ctx.cell_root, &ctx.workdir);
        if let Err(e) = std::fs::create_dir_all(&workdir_path) {
            return ProvisionOutcome::Failed(format!(
                "create workdir {}: {e}",
                workdir_path.display()
            ));
        }
        ProvisionOutcome::Succeeded(declaration.clone())
    }
}

/// The shell-script provisioner (`kind = "script"`). It executes the script
/// declared at `provision.script` **inside the provisioning sandbox** — the
/// cell rootfs is `/`, the Cellfile directory and declared host paths are
/// read-only, and network egress is enabled. The script seeds the rootfs
/// (installing tools, copying files, creating the working directory) and must
/// exit zero on success; a nonzero exit fails provisioning.
///
/// Inside the sandbox, the script receives these control variables:
/// - `HOTCELL_CELL_ROOT`: `/` (the rootfs is mounted as the sandbox root).
/// - `HOTCELL_CELLFILE_DIR`: the Cellfile's directory (read-only bind).
/// - `HOTCELL_WORKDIR`: the in-sandbox working directory path (e.g. `/work`).
/// - `HOTCELL_WORKDIR_HOST`: same as `HOTCELL_WORKDIR` (the host backing *is*
///   the in-sandbox path once the rootfs is `/`).
///
/// The script inherits the host environment (so standard tooling works
/// during bootstrap). It does *not* receive the declared `env.*` variables
/// — those are the agent's environment. Its stdout/stderr stream to the
/// developer's console (`ProvisioningLogsToConsole`).
pub struct ScriptProvisioner {
    script: PathBuf,
}

impl Provisioner for ScriptProvisioner {
    fn provision(&self, ctx: &ProvisionContext, declaration: &CellDeclaration) -> ProvisionOutcome {
        let script = if self.script.is_absolute() {
            self.script.clone()
        } else {
            ctx.cellfile_directory.join(&self.script)
        };

        let meta = match std::fs::metadata(&script) {
            Ok(m) => m,
            Err(e) => {
                return ProvisionOutcome::Failed(format!(
                    "provision script not found at {}: {e}",
                    script.display()
                ));
            }
        };
        if meta.permissions().mode() & 0o111 == 0 {
            return ProvisionOutcome::Failed(format!(
                "provision script {} is not executable — try `chmod +x`",
                script.display()
            ));
        }

        // Run the script inside the provisioning-profile sandbox. The rootfs
        // is `/`, so HOTCELL_CELL_ROOT is `/` and HOTCELL_WORKDIR_HOST equals
        // HOTCELL_WORKDIR. The provisioner inherits the host environment
        // (so bootstrap tooling works) and receives only HOTCELL_* controls —
        // NOT the declared agent env, which is a different actor's environment.
        let control_env: Vec<(String, String)> = [
            ("HOTCELL_CELL_ROOT".to_string(), "/".to_string()),
            ("HOTCELL_CELLFILE_DIR".to_string(), ctx.cellfile_directory.to_string_lossy().into_owned()),
            ("HOTCELL_WORKDIR".to_string(), ctx.workdir.clone()),
            ("HOTCELL_WORKDIR_HOST".to_string(), ctx.workdir.clone()),
        ]
        .into_iter()
        .collect();

        let mut cmd = isolation::build_provisioning_command(
            &ctx.cell_root,
            &ctx.cellfile_directory,
            &ctx.host_paths,
            &control_env,
            &script,
        );

        let status = match cmd.status() {
            Ok(s) => s,
            Err(e) => {
                return ProvisionOutcome::Failed(format!(
                    "failed to run provision script {}: {e}",
                    script.display()
                ));
            }
        };

        if !status.success() {
            return ProvisionOutcome::Failed(format!(
                "provision script {} exited with code {}",
                script.display(),
                status.code().unwrap_or(-1)
            ));
        }

        ProvisionOutcome::Succeeded(declaration.clone())
    }
}

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

use sha2::{Digest, Sha256};

use crate::cell::Cell;
use crate::cellfile::{CellDeclaration, Provisioner as ProvisionerSpec};
use crate::isolation;
use crate::plan::PlanStep;
use crate::state;

/// Outcome of a provisioning attempt.
pub enum ProvisionOutcome {
    Succeeded(CellDeclaration),
    Failed(String),
}

/// Everything a provisioner needs to bring the cell's environment into being.
/// The sandbox command itself is NOT here — it comes from the execution
/// plan (see [`crate::plan::build_execution_plan`]) so display and execution
/// share one construction.
pub struct ProvisionContext {
    /// The cell's sandbox rootfs backing directory (`fs_dir`), bind-mounted as
    /// `/` inside the provisioning sandbox. The provisioner writes tools,
    /// files, and the working directory here.
    pub cell_root: PathBuf,
    /// The Cellfile's directory, used to resolve the provision script path
    /// for error messages.
    pub cellfile_directory: PathBuf,
    /// The in-sandbox working directory path (e.g. `/work`).
    pub workdir: String,
}

/// A pluggable provisioner. Implementations bring a cell's environment into
/// being and report the outcome.
///
/// The declaration is passed in because the provisioner echoes it back
/// unchanged on success — the declaration is authoritative for the agent's
/// view (environment, network, workdir); a provisioner only seeds the
/// filesystem. (See the open question on whether provisioners may mutate the
/// declaration; v1 assumes not.)
///
/// `provisioning_step` is the plan's prebuilt bwrap step. A provisioner that
/// runs inside a sandbox (the `shell` kind) executes that step as-is rather
/// than assembling its own command, so the displayed plan and the executed
/// command are the same value. Host-side provisioners (the `none` kind)
/// receive `None` and do their work directly.
pub trait Provisioner {
    fn provision(
        &self,
        ctx: &ProvisionContext,
        declaration: &CellDeclaration,
        provisioning_step: Option<&mut PlanStep>,
    ) -> ProvisionOutcome;
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
        "shell" => {
            let script = spec.script.as_ref().ok_or_else(|| {
                anyhow::anyhow!("shell provisioner requires `provision.script` in the Cellfile")
            })?;
            Ok(Box::new(ScriptProvisioner {
                script: PathBuf::from(script),
            }))
        }
        other => Err(anyhow::anyhow!("unknown provisioner kind: {other}")),
    }
}

/// Compute the SHA-256 digest (lowercase hex) of the provisioning inputs for
/// the declared spec, resolved against the Cellfile directory.
///
/// For the `shell` provisioner this is the digest of the script file's
/// bytes — the thing the provisioner actually executes. For the `none`
/// provisioner there is no script, so the digest is `None`; a matching
/// `None` still lets a re-run skip provisioning when the declaration is
/// unchanged. Returns `None` (rather than erroring) when the script path is
/// not set, so callers can decide how to handle a missing script.
pub fn provision_digest(
    cellfile_directory: &Path,
    spec: &ProvisionerSpec,
) -> anyhow::Result<Option<String>> {
    if spec.kind != "shell" {
        return Ok(None);
    }
    let Some(script) = spec.script.as_ref() else {
        return Ok(None);
    };
    let path = cellfile_directory.join(script);
    let bytes = std::fs::read(&path)?;
    let digest = Sha256::digest(&bytes);
    Ok(Some(format!("{:x}", digest)))
}

/// Validate the declared provisioner kind. Read-only; called by the plan
/// builder so a config error fails before anything is displayed, prompted,
/// or run.
pub fn validate_provisioner(spec: &ProvisionerSpec) -> anyhow::Result<()> {
    match spec.kind.as_str() {
        state::DEFAULT_PROVISIONER_KIND | "shell" => Ok(()),
        other => Err(anyhow::anyhow!("unknown provisioner kind: {other}")),
    }
}

/// Resolve and validate the shell provisioner's script, returning the
/// Cellfile-relative path used inside the sandbox. Enforces the checks the
/// shell provisioner used to perform at execution time (relative path,
/// exists, executable) so the plan fails fast, before display and prompt.
pub fn resolve_provision_script(
    cellfile_directory: &Path,
    spec: &ProvisionerSpec,
) -> anyhow::Result<String> {
    let script = spec.script.as_ref().ok_or_else(|| {
        anyhow::anyhow!("shell provisioner requires `provision.script` in the Cellfile")
    })?;
    let script_path = Path::new(script);
    if script_path.is_absolute() {
        anyhow::bail!(
            "provision.script must be relative to the Cellfile directory, got absolute path `{}`",
            script_path.display()
        );
    }
    let script_host = cellfile_directory.join(script_path);
    let meta = match std::fs::metadata(&script_host) {
        Ok(m) => m,
        Err(e) => {
            return Err(anyhow::anyhow!(
                "provision script not found at {}: {e}",
                script_host.display()
            ));
        }
    };
    if meta.permissions().mode() & 0o111 == 0 {
        anyhow::bail!(
            "provision script {} is not executable — try `chmod +x`",
            script_host.display()
        );
    }
    Ok(script_path.to_string_lossy().into_owned())
}

/// Build the HOTCELL_* control environment (and the cell-local HOME) for the
/// provisioning sandbox. Single source for the plan, so the displayed
/// `--setenv` arguments are exactly what the executed sandbox receives.
pub fn provision_control_env(
    declaration: &CellDeclaration,
    workdir: &str,
) -> Vec<(String, String)> {
    let agent_home = declaration
        .environment
        .iter()
        .find(|v| v.key == "HOME")
        .map(|v| v.value.clone())
        .unwrap_or_else(|| "/home/agent".to_string());
    vec![
        ("HOTCELL_CELL_ROOT".to_string(), "/".to_string()),
        ("HOTCELL_WORKDIR".to_string(), workdir.to_string()),
        ("HOTCELL_WORKDIR_HOST".to_string(), workdir.to_string()),
        ("HOME".to_string(), agent_home),
    ]
}

/// Run provisioning for a cell against its Cellfile's declaration.
///
/// Builds the provisioner from the declaration, prepares the rootfs, runs it,
/// and reports the outcome. The sandbox command comes from the execution
/// plan (the plan's `provisioning` step), never built here — display and
/// execution share one construction. The caller is responsible for
/// persisting the outcome via the [`state`](crate::state) module. After the
/// provisioner runs, the host-path staging directory is removed from the
/// rootfs so no host-path stubs leak into the agent's view.
pub fn provision(
    cell: &Cell,
    declaration: &CellDeclaration,
    plan: &mut crate::plan::ExecutionPlan,
) -> anyhow::Result<ProvisionOutcome> {
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
        cell_root: cell_root.clone(),
        cellfile_directory: cell.cellfile_directory.clone(),
        workdir,
    };

    let outcome = provisioner.provision(&ctx, declaration, plan.provisioning.as_mut());
    // Remove the host-path staging skeleton from the rootfs regardless of
    // outcome — bwrap leaves intermediate bind-target dirs behind, and we
    // don't want them (or the host username they'd embed) visible to the
    // agent.
    isolation::clean_staging(&cell_root);
    Ok(outcome)
}

/// The no-op provisioner (`kind = "none"`). It creates an empty working
/// directory on the host side and succeeds immediately. There is no process
/// to run inside the sandbox — "none" means no provisioning work. Useful for
/// cells that need nothing provisioned (e.g. running a base-system program in
/// isolation).
pub struct NoneProvisioner;

impl Provisioner for NoneProvisioner {
    fn provision(
        &self,
        ctx: &ProvisionContext,
        declaration: &CellDeclaration,
        _provisioning_step: Option<&mut PlanStep>,
    ) -> ProvisionOutcome {
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

/// The shell-script provisioner (`kind = "shell"`). It executes the script
/// declared at `provision.script` **inside the provisioning sandbox** — the
/// cell rootfs is `/`, the Cellfile directory and declared host paths are
/// read-only, and network egress is enabled. The script seeds the rootfs
/// (installing tools, copying files, creating the working directory) and must
/// exit zero on success; a nonzero exit fails provisioning.
///
/// The sandbox command comes from the execution plan (the plan's
/// `provisioning` step); this provisioner executes that exact step so
/// the displayed command and the executed command are the same value.
/// Pre-flight validation (relative path, existence, executability) and
/// the HOTCELL_* control environment are handled by the plan builder
/// ([`crate::plan::build_execution_plan`]); the script's stdout/stderr
/// stream to the developer's console (`ProvisioningLogsToConsole`).
pub struct ScriptProvisioner {
    script: PathBuf,
}

impl Provisioner for ScriptProvisioner {
    fn provision(
        &self,
        ctx: &ProvisionContext,
        declaration: &CellDeclaration,
        provisioning_step: Option<&mut PlanStep>,
    ) -> ProvisionOutcome {
        let Some(step) = provisioning_step else {
            return ProvisionOutcome::Failed(
                "shell provisioner: no provisioning step in the execution plan".to_string(),
            );
        };

        let script_host = ctx.cellfile_directory.join(&self.script);
        let status = match step.command.status() {
            Ok(s) => s,
            Err(e) => {
                return ProvisionOutcome::Failed(format!(
                    "failed to run provision script {}: {e}",
                    script_host.display()
                ));
            }
        };

        if !status.success() {
            return ProvisionOutcome::Failed(format!(
                "provision script {} exited with code {}",
                script_host.display(),
                status.code().unwrap_or(-1)
            ));
        }

        ProvisionOutcome::Succeeded(declaration.clone())
    }
}

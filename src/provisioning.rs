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

/// Run provisioning for a cell against its Cellfile's declaration.
///
/// Builds the provisioner from the declaration, prepares the rootfs, runs it,
/// and reports the outcome. The caller is responsible for persisting the
/// outcome via the [`state`](crate::state) module. After the provisioner
/// runs, the host-path staging directory is removed from the rootfs so no
/// host-path stubs leak into the agent's view.
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
        cell_root: cell_root.clone(),
        cellfile_directory: cell.cellfile_directory.clone(),
        workdir,
        environment: declaration
            .environment
            .iter()
            .map(|v| (v.key.clone(), v.value.clone()))
            .collect(),
        host_paths: declaration.provisioner.host_paths.clone(),
    };

    let outcome = provisioner.provision(&ctx, declaration);
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

/// The shell-script provisioner (`kind = "shell"`). It executes the script
/// declared at `provision.script` **inside the provisioning sandbox** — the
/// cell rootfs is `/`, the Cellfile directory and declared host paths are
/// read-only, and network egress is enabled. The script seeds the rootfs
/// (installing tools, copying files, creating the working directory) and must
/// exit zero on success; a nonzero exit fails provisioning.
///
/// Inside the sandbox, the script receives these control variables:
/// - `HOTCELL_CELL_ROOT`: `/` (the rootfs is mounted as the sandbox root).
/// - `HOTCELL_CELLFILE_DIR`: `/hotcell/cellfile` (the staged Cellfile dir).
/// - `HOTCELL_HOST_ROOT`: `/hotcell/host` (root of the staged host-path tree).
/// - `HOTCELL_HOST_HOME`: `/hotcell/host/<host HOME>` (staged host home, for
///   copying config from `~/.pi` etc.).
/// - `HOTCELL_WORKDIR`: the in-sandbox working directory path (e.g. `/work`).
/// - `HOTCELL_WORKDIR_HOST`: same as `HOTCELL_WORKDIR` (the host backing *is*
///   the in-sandbox path once the rootfs is `/`).
///
/// The script inherits the host environment (so standard tooling works
/// during bootstrap) EXCEPT for `HOME`, which is overridden to a cell-local
/// path (the agent's declared `env.HOME`, defaulting to `/home/agent`) so
/// tools that cache into `$HOME` (npm, nvm, etc.) write inside the cell rootfs
/// rather than leaking a ghost of the host user's home. The script does *not*
/// receive the declared `env.*` variables — those are the agent's environment.
/// Its stdout/stderr stream to the developer's console (`ProvisioningLogsToConsole`).
pub struct ScriptProvisioner {
    script: PathBuf,
}

impl Provisioner for ScriptProvisioner {
    fn provision(&self, ctx: &ProvisionContext, declaration: &CellDeclaration) -> ProvisionOutcome {
        // The script path must be relative to the Cellfile directory; the
        // provisioning sandbox stages the Cellfile dir under /hotcell/cellfile
        // and execs the script there, so an absolute host path would not
        // resolve inside the sandbox.
        if self.script.is_absolute() {
            return ProvisionOutcome::Failed(format!(
                "provision.script must be relative to the Cellfile directory, got absolute path `{}`",
                self.script.display()
            ));
        }
        let script_rel = self.script.to_string_lossy().into_owned();
        let script_host = ctx.cellfile_directory.join(&self.script);

        let meta = match std::fs::metadata(&script_host) {
            Ok(m) => m,
            Err(e) => {
                return ProvisionOutcome::Failed(format!(
                    "provision script not found at {}: {e}",
                    script_host.display()
                ));
            }
        };
        if meta.permissions().mode() & 0o111 == 0 {
            return ProvisionOutcome::Failed(format!(
                "provision script {} is not executable — try `chmod +x`",
                script_host.display()
            ));
        }

        // Run the script inside the provisioning-profile sandbox. The rootfs
        // is `/`, so HOTCELL_CELL_ROOT is `/` and HOTCELL_WORKDIR_HOST equals
        // HOTCELL_WORKDIR. The provisioner inherits the host environment
        // (so bootstrap tooling works) and receives only HOTCELL_* controls —
        // NOT the declared agent env, which is a different actor's environment.
        //
        // HOME is overridden to a cell-local path (the agent's declared HOME,
        // defaulting to /home/agent) so that tools which cache into $HOME
        // (npm -> ~/.npm, nvm -> ~/.nvm, etc.) write inside the cell rootfs
        // rather than creating a ghost of the host user's home (e.g.
        // /home/<hostuser>) that the agent would then see. The original host
        // HOME is exposed via HOTCELL_HOST_HOME (a staged path) so a script
        // can still find host config through its provision.host_path binds.
        // HOTCELL_CELLFILE_DIR and HOTCELL_HOST_ROOT are set by the sandbox
        // builder (see isolation::build_provisioning_command).
        let agent_home = ctx
            .environment
            .iter()
            .find(|(k, _)| k == "HOME")
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| "/home/agent".to_string());
        let control_env: Vec<(String, String)> = [
            ("HOTCELL_CELL_ROOT".to_string(), "/".to_string()),
            ("HOTCELL_WORKDIR".to_string(), ctx.workdir.clone()),
            ("HOTCELL_WORKDIR_HOST".to_string(), ctx.workdir.clone()),
            ("HOME".to_string(), agent_home),
        ]
        .into_iter()
        .collect();

        let mut cmd = isolation::build_provisioning_command(
            &ctx.cell_root,
            &ctx.cellfile_directory,
            &ctx.host_paths,
            &control_env,
            &script_rel,
        );

        let status = match cmd.status() {
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

//! Execution plan for `hotcell run`: every external command the run will
//! execute, compiled up front from read-only inputs.
//!
//! The plan is the single source of truth for both display and execution.
//! The developer is shown exactly this list (via [`ExecutionPlan::render`]),
//! and the run then executes the plan's own [`Command`] values — there is
//! no second construction site that could drift from what was displayed.
//! What you see is the command that runs, value for value.
//!
//! Building a plan has zero side effects: it reads the Cellfile, the cell's
//! durable state, and (for the shell provisioner) the provision script, but
//! creates no directories, starts no processes, and writes nothing. The
//! cell rootfs path and the bridge dir/socket paths are computed here so
//! the displayed `--ro-bind` arguments are the real ones; only the actual
//! creation of those paths is deferred to execution.

use std::borrow::Cow;
use std::path::PathBuf;
use std::process::Command;

use anyhow::Result;
use shlex::try_quote;

use crate::cellfile::Cellfile;
use crate::isolation::{self, AgentBridge, BRIDGE_DIR};
use crate::state;

/// Default in-sandbox working directory when the Cellfile declares none.
pub const DEFAULT_WORKDIR: &str = "/work";

/// One planned command.
pub struct PlanStep {
    /// Short label shown in the rendered plan ("provisioning", "agent").
    pub label: &'static str,
    /// The exact command that will be spawned when this step executes.
    pub command: Command,
}

/// The complete plan for one `hotcell run`.
pub struct ExecutionPlan {
    /// The cell's rootfs backing directory (`fs_dir`), bind-mounted as `/`.
    /// Shared by every step; kept here so execution needs no re-derivation.
    pub cell_fs: PathBuf,
    /// The in-sandbox working directory for the agent step.
    pub workdir: String,

    /// The provisioning bwrap step. `None` when provisioning will not run —
    /// the provisioner is host-side (`none`), or the cell is already
    /// provisioned with current inputs ([`ExecutionPlan::provisioning_skipped`]).
    pub provisioning: Option<PlanStep>,
    /// True when provisioning was omitted only because the cell is already
    /// provisioned with current inputs (as opposed to the `none` provisioner,
    /// which never has a bwrap step).
    pub provisioning_skipped: bool,

    /// The agent's bwrap step. Always present.
    pub agent: PlanStep,
    /// Loopback bridge configuration for firewall egress, built from the
    /// real cell-scoped paths with no side effects. `None` for offline cells.
    pub bridge: Option<AgentBridge>,
}

impl ExecutionPlan {
    /// Render the plan for display: each step as a copy-pasteable shell
    /// invocation — one arg per line with `\` continuations; everything
    /// after a `--` separator stays on one line (the sandboxed command and
    /// its args). Every arg is POSIX-quoted via `shlex`, so the block runs
    /// verbatim when pasted into a shell.
    pub fn render(&self) -> String {
        let mut blocks: Vec<String> = Vec::new();
        match &self.provisioning {
            Some(step) => blocks.push(format!(
                "=== {} bwrap ===\n{}",
                step.label,
                render_step(step)
            )),
            None if self.provisioning_skipped => blocks.push(
                "provisioning: skipped (cell already provisioned with current inputs)".to_string(),
            ),
            None => blocks.push("provisioning: none (no bwrap during provisioning)".to_string()),
        }
        blocks.push(format!(
            "=== {} bwrap ===\n{}",
            self.agent.label,
            render_step(&self.agent)
        ));
        let mut out = blocks.join("\n\n");
        if self.bridge.is_some() {
            out.push_str(
                "\n\n(the agent's HTTP_PROXY/HTTPS_PROXY are set inside the sandbox by the \
                 hotcell forwarder after it binds its port)",
            );
        }
        out
    }
}

/// Format one planned command as a copy-pasteable shell invocation.
fn render_step(step: &PlanStep) -> String {
    let program = step.command.get_program().to_string_lossy().to_string();
    let args: Vec<String> = step
        .command
        .get_args()
        .map(|a| {
            let s = a.to_string_lossy();
            // try_quote fails only on NUL bytes, which cannot occur in an
            // argv string that exec would accept.
            try_quote(&s).unwrap_or(Cow::Borrowed("''")).into_owned()
        })
        .collect();
    let mut lines: Vec<String> = Vec::new();

    if let Some(sep) = args.iter().position(|a| a == "--") {
        // Program line with continuation.
        lines.push(format!("{program} \\"));
        // Each bwrap flag/arg before `--` gets its own line with ` \`.
        for arg in &args[..sep] {
            lines.push(format!("  {} \\", arg));
        }
        // The `--` separator itself gets a continuation backslash.
        lines.push(format!("  {} \\", args[sep]));
        // Everything after `--` is one line (the sandboxed command), no backslash.
        lines.push(format!("  {}", args[(sep + 1)..].join(" ")));
    } else {
        // No `--` separator: every arg is a bwrap option.
        if args.is_empty() {
            lines.push(program);
        } else {
            lines.push(format!("{program} \\"));
            let last = args.len() - 1;
            for (i, arg) in args.iter().enumerate() {
                if i == last {
                    lines.push(format!("  {}", arg));
                } else {
                    lines.push(format!("  {} \\", arg));
                }
            }
        }
    }

    lines.join("\n")
}

/// Compile the plan for a `hotcell run`. All inputs are read-only (Cellfile,
/// cell state, provision script); nothing is created, started, or spawned.
///
/// `provisioning_will_run` must be false when the cell is already provisioned
/// with current inputs — the provisioning step is then omitted so the
/// displayed plan matches the executed run exactly.
pub fn build_execution_plan(
    cellfile: &Cellfile,
    name: &str,
    program: &str,
    args: &[String],
    provisioning_will_run: bool,
) -> Result<ExecutionPlan> {
    let declaration = &cellfile.declaration;

    // Fail fast on config errors, before anything is displayed or prompted.
    crate::provisioning::validate_provisioner(&declaration.provisioner)?;

    let cell_fs = state::fs_dir(&cellfile.directory, name);
    let workdir = if declaration.workdir.is_empty() {
        DEFAULT_WORKDIR
    } else {
        declaration.workdir.as_str()
    };
    let env: Vec<(String, String)> = declaration
        .environment
        .iter()
        .map(|v| (v.key.clone(), v.value.clone()))
        .collect();

    // Only the `shell` provisioner has a bwrap step: it runs inside the
    // sandbox. The `none` provisioner works host-side (a plain mkdir) and
    // never gets one. When the cell is already provisioned with current
    // inputs, provisioning is skipped entirely and the step is omitted so
    // the displayed plan matches the executed run.
    let provisioning = if provisioning_will_run && declaration.provisioner.kind == "shell" {
        let script_rel = crate::provisioning::resolve_provision_script(
            &cellfile.directory,
            &declaration.provisioner,
        )?;
        let control_env = crate::provisioning::provision_control_env(declaration, workdir);
        let command = isolation::build_provisioning_command(
            &cell_fs,
            &cellfile.directory,
            &declaration.provisioner.host_paths,
            &control_env,
            &script_rel,
        );
        Some(PlanStep {
            label: "provisioning",
            command,
        })
    } else {
        None
    };

    // Bridge with the real cell-scoped paths. Creating the directory and the
    // socket is deferred to execution, so plan building stays side-effect-free.
    let bridge = if declaration.network.allowed_endpoints.is_empty() {
        None
    } else {
        Some(AgentBridge {
            host_bridge_dir: cell_fs.join("hotcell-bridge"),
            uds_in_sandbox: format!("{BRIDGE_DIR}/proxy.sock"),
        })
    };

    let agent = PlanStep {
        label: "agent",
        command: isolation::build_agent_command(
            &cell_fs,
            workdir,
            &env,
            bridge.as_ref(),
            program,
            args,
        ),
    };

    Ok(ExecutionPlan {
        cell_fs,
        workdir: workdir.to_string(),
        provisioning_skipped: !provisioning_will_run && declaration.provisioner.kind == "shell",
        provisioning,
        agent,
        bridge,
    })
}

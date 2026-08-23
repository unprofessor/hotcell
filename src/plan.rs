//! Execution plan for `hotcell run`: every external command the run will
//! execute, compiled up front from read-only inputs.
//!
//! The plan is the single source of truth for both display and execution.
//! Each step is composed symbolically as a list of [`Argument`]s — a flag
//! plus its values — the form the developer is shown (via
//! [`ExecutionPlan::render`]), and the same list collapses into the argv of
//! the `bwrap` process that actually runs. There is no second construction
//! site that could drift from what was displayed: what you see is the
//! command that runs, value for value.
//!
//! Building a plan has zero side effects: it reads the Cellfile, the cell's
//! durable state, and (for the shell provisioner) the provision script, but
//! creates no directories, starts no processes, and writes nothing. The
//! cell rootfs path and the bridge dir/socket paths are computed here so
//! the displayed `--ro-bind` arguments are the real ones; only the actual
//! creation of those paths is deferred to execution.

use std::borrow::Cow;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use shlex::try_quote;

use crate::cellfile::{Cellfile, EnvSource};
use crate::isolation::{self, AgentBridge, Argument, BRIDGE_DIR};
use crate::state;

/// Default in-sandbox working directory when the Cellfile declares none.
pub const DEFAULT_WORKDIR: &str = "/work";

/// One planned command: a label plus the `bwrap` arguments it will run
/// with. The arguments are the source of truth — [`PlanStep::command`]
/// collapses them for execution, [`ExecutionPlan::render`] renders them for
/// display — so the two can never diverge.
pub struct PlanStep {
    /// Short label shown in the rendered plan ("provisioning", "agent").
    pub label: &'static str,
    /// The exact bwrap arguments that will be spawned when this step runs.
    pub arguments: Vec<Argument>,
}

impl PlanStep {
    fn new(label: &'static str, arguments: Vec<Argument>) -> PlanStep {
        PlanStep { label, arguments }
    }

    /// Collapse the planned arguments into an executable `Command` (argv
    /// order preserved: flag, its values, next flag, ...).
    pub fn command(&self) -> std::process::Command {
        isolation::collapse(&self.arguments)
    }
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
    /// invocation — one line per argument (`--flag value1 value2`) with `\`
    /// continuations, so line breaks fall only between flags. The final
    /// argument is the command separator: `-- \` on its own line, then the
    /// sandboxed command as one unbroken line. Every flag and value is
    /// POSIX-quoted via `shlex`, so the block runs verbatim when pasted
    /// into a shell.
    pub fn render(&self) -> String {
        let mut blocks: Vec<String> = Vec::new();
        match &self.provisioning {
            Some(step) => blocks.push(format!(
                "=== {} bwrap ===\n{}",
                step.label,
                render_step(&step.arguments)
            )),
            None if self.provisioning_skipped => blocks.push(
                "provisioning: skipped (cell already provisioned with current inputs)".to_string(),
            ),
            None => blocks.push("provisioning: none (no bwrap during provisioning)".to_string()),
        }
        blocks.push(format!(
            "=== {} bwrap ===\n{}",
            self.agent.label,
            render_step(&self.agent.arguments)
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

/// Format one planned command as a copy-pasteable shell invocation: one
/// line per [`Argument`] — `--flag value1 value2` — with a `\` continuation
/// on every line except the last. Because the arguments are structured, each
/// flag always stays with its values, and the command (`Argument { flag:
/// "--", ... }`) is wrapped after the separator: `-- \` on its own line,
/// then `/bin/echo hello world` as the final unbroken line. Every flag and
/// value is POSIX-quoted via `shlex`, so the block runs verbatim when
/// pasted into a shell (line breaks fall only between arguments, and a
/// continuation joins them back into the same argv).
fn render_step(arguments: &[Argument]) -> String {
    if arguments.is_empty() {
        return "bwrap".to_string();
    }
    let mut lines: Vec<String> = vec!["bwrap \\".to_string()];
    let last = arguments.len() - 1;
    for (i, arg) in arguments.iter().enumerate() {
        let is_last = i == last;
        // The command separator gets its own line with a continuation; the
        // sandboxed command follows on its own final, unbroken line:
        // `-- \` then `/bin/echo hello world`.
        if is_last && arg.flag == "--" {
            if arg.values.is_empty() {
                lines.push("  --".to_string());
            } else {
                lines.push("  -- \\".to_string());
                let command = arg
                    .values
                    .iter()
                    .map(|value| quote_arg(value))
                    .collect::<Vec<_>>()
                    .join(" ");
                lines.push(format!("  {command}"));
            }
            continue;
        }
        let line = std::iter::once(&arg.flag)
            .chain(arg.values.iter())
            .map(|s| quote_arg(s))
            .collect::<Vec<_>>()
            .join(" ");
        if is_last {
            lines.push(format!("  {line}"));
        } else {
            lines.push(format!("  {line} \\"));
        }
    }
    lines.join("\n")
}

/// POSIX-quote one flag or value for shell display.
fn quote_arg(s: &str) -> String {
    // try_quote fails only on NUL bytes, which cannot occur in an argv
    // string that exec would accept.
    try_quote(s).unwrap_or(Cow::Borrowed("''")).into_owned()
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

    let workdir = if declaration.workdir.is_empty() {
        DEFAULT_WORKDIR
    } else {
        declaration.workdir.as_str()
    };
    // Only literal values can be plumbed through today. Resolving a
    // passthrough or a secret against the host environment is
    // `resolve-env-passthrough-and-secrets`; until it lands, a Cellfile that
    // declares one fails loudly here rather than handing the agent a cell
    // that is silently missing the variable it was promised.
    let env: Vec<(String, String)> = declaration
        .environment
        .iter()
        .map(|v| match &v.source {
            EnvSource::Literal(value) => Ok((v.key.clone(), value.clone())),
            EnvSource::Passthrough => Err(anyhow!(
                "env `{}`: passthrough from the host environment is not yet implemented \
                 — declare it as `env {}=\"value\"` for now",
                v.key,
                v.key
            )),
            EnvSource::Secret => Err(anyhow!(
                "secret `{}`: secrets are not yet implemented — declare it as \
                 `env {}=\"value\"` for now",
                v.key,
                v.key
            )),
        })
        .collect::<Result<_>>()?;

    // Only the `shell` provisioner has a bwrap step: it runs inside the
    // sandbox. The `none` provisioner works host-side (a plain mkdir) and
    // never gets one. When the cell is already provisioned with current
    // inputs, provisioning is skipped entirely and the step is omitted so
    // the displayed plan matches the executed run.
    //
    // The cell rootfs is used both as a bind source in the plan's arguments
    // and (on the host side) for the workdir safety-net and the bridge dir;
    // make it absolute so the two always agree and the displayed command is
    // unambiguous even when the Cellfile/`--cell` paths were relative.
    //
    // Provisioning inputs (the script path, the staged Cellfile directory)
    // resolve against the Cellfile's OWN directory, not the state root —
    // `--cell` may have relocated the latter.
    let cell_fs = isolation::absolutize(&state::fs_dir(&cellfile.directory, name));
    let provisioning = if provisioning_will_run && declaration.provisioner.kind == "shell" {
        let script_rel = crate::provisioning::resolve_provision_script(
            &cellfile.file_directory,
            &declaration.provisioner,
        )?;
        let control_env = crate::provisioning::provision_control_env(declaration, workdir);
        let arguments = isolation::build_provisioning_command(
            &cell_fs,
            &cellfile.file_directory,
            &declaration.provisioner.host_paths,
            &control_env,
            &script_rel,
        );
        Some(PlanStep::new("provisioning", arguments))
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

    let agent = PlanStep::new(
        "agent",
        isolation::build_agent_command(&cell_fs, workdir, &env, bridge.as_ref(), program, args),
    );

    Ok(ExecutionPlan {
        cell_fs,
        workdir: workdir.to_string(),
        provisioning_skipped: !provisioning_will_run && declaration.provisioner.kind == "shell",
        provisioning,
        agent,
        bridge,
    })
}

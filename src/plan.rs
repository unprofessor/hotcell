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

use crate::cellfile::{Cellfile, EnvSource, EnvVar};
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

    /// Names of env vars declared as `secret`. Their real values live in the
    /// step arguments (so execution is unchanged), but [`ExecutionPlan::render`]
    /// masks each one's `--setenv` value as `<redacted>` for display, and its
    /// non-emptiness drives the redaction footer. Redaction is keyed off the
    /// declared `Secret` source, not a heuristic over the variable name.
    pub secret_keys: Vec<String>,
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
                render_step(&step.arguments, &self.secret_keys)
            )),
            None if self.provisioning_skipped => blocks.push(
                "provisioning: skipped (cell already provisioned with current inputs)".to_string(),
            ),
            None => blocks.push("provisioning: none (no bwrap during provisioning)".to_string()),
        }
        blocks.push(format!(
            "=== {} bwrap ===\n{}",
            self.agent.label,
            render_step(&self.agent.arguments, &self.secret_keys)
        ));
        let mut out = blocks.join("\n\n");
        if self.bridge.is_some() {
            out.push_str(
                "\n\n(the agent's HTTP_PROXY/HTTPS_PROXY are set inside the sandbox by the \
                 hotcell forwarder after it binds its port)",
            );
        }
        if !self.secret_keys.is_empty() {
            out.push_str(
                "\n\n(a declared secret is shown as <redacted>: this block will NOT run \
                 verbatim as shown — the real value is read from hotcell's environment at \
                 run time and never printed)",
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
fn render_step(arguments: &[Argument], secret_keys: &[String]) -> String {
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
        let line = render_argument(arg, secret_keys);
        if is_last {
            lines.push(format!("  {line}"));
        } else {
            lines.push(format!("  {line} \\"));
        }
    }
    lines.join("\n")
}

/// Placeholder shown in place of a declared secret's value. Intentionally
/// unquoted so it reads as a marker rather than a literal value — the
/// redaction footer already tells the developer the block will not run as
/// shown.
const REDACTED: &str = "<redacted>";

/// Render one argument (`--flag v1 v2`) for display, masking the value of a
/// `--setenv NAME VALUE` whose NAME is a declared secret. The real value stays
/// in the argument for execution; only this display string is redacted.
fn render_argument(arg: &Argument, secret_keys: &[String]) -> String {
    if arg.flag == "--setenv" {
        if let [key, _value] = arg.values.as_slice() {
            if secret_keys.iter().any(|k| k == key) {
                return format!("{} {} {}", quote_arg(&arg.flag), quote_arg(key), REDACTED);
            }
        }
    }
    std::iter::once(&arg.flag)
        .chain(arg.values.iter())
        .map(|s| quote_arg(s))
        .collect::<Vec<_>>()
        .join(" ")
}

/// POSIX-quote one flag or value for shell display.
fn quote_arg(s: &str) -> String {
    // try_quote fails only on NUL bytes, which cannot occur in an argv
    // string that exec would accept.
    try_quote(s).unwrap_or(Cow::Borrowed("''")).into_owned()
}

/// Resolve declared env vars into the concrete `(key, value)` pairs the agent
/// is launched with (`--setenv`).
///
/// A `Literal` value is taken verbatim from the Cellfile. A `Passthrough`
/// (`env NAME`) or `Secret` (`secret NAME`) value is looked up in hotcell's
/// own environment via `lookup` — never written in the Cellfile, never part
/// of the declaration. A declared passthrough or secret that `lookup` cannot
/// find is a hard error naming both the variable and the node that declared
/// it (`env` versus `secret`), so a missing key fails the run outright rather
/// than silently handing the agent a cell short a variable it was promised.
///
/// Resolved passthrough/secret values exist only in the returned vector: they
/// are never stored back into `CellDeclaration`, so they cannot reach
/// `provisioned_as.json` or the declaration comparison that decides whether a
/// cell is stale. Rotating a secret on the host therefore does not
/// re-provision the cell.
///
/// `lookup` is injected so this stays a pure function of its inputs; the real
/// caller passes `|key| std::env::var(key).ok()`.
fn resolve_environment(
    environment: &[EnvVar],
    lookup: impl Fn(&str) -> Option<String>,
) -> Result<Vec<(String, String)>> {
    environment
        .iter()
        .map(|var| {
            let value = match &var.source {
                EnvSource::Literal(value) => value.clone(),
                EnvSource::Passthrough => lookup(&var.key).ok_or_else(|| {
                    anyhow!(
                        "env `{}`: declared as a host-environment passthrough, but `{}` \
                         is not set in hotcell's environment",
                        var.key,
                        var.key
                    )
                })?,
                EnvSource::Secret => lookup(&var.key).ok_or_else(|| {
                    anyhow!(
                        "secret `{}`: declared as a host-environment secret, but `{}` is \
                         not set in hotcell's environment",
                        var.key,
                        var.key
                    )
                })?,
            };
            Ok((var.key.clone(), value))
        })
        .collect()
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
    // Resolve declared env vars into the concrete key/value pairs the agent
    // receives as `--setenv`. Literal values come from the Cellfile;
    // passthrough and secret values are read from hotcell's OWN process
    // environment here. A declared var missing from that environment is a
    // hard error — raised at plan-build time, so it surfaces before the plan
    // is displayed and before any state or directory is touched. Resolved
    // values are returned to the caller for the agent command only; they
    // never enter `CellDeclaration`, so they never reach `provisioned_as.json`
    // or the declaration comparison that decides cell identity.
    let env = resolve_environment(&declaration.environment, |key| std::env::var(key).ok())?;

    // Names declared as `secret`, so `render` can mask their `--setenv` values.
    // The resolved values themselves stay only in `env` (and thus in the agent
    // step's arguments, for execution); this is just the key list.
    let secret_keys: Vec<String> = declaration
        .environment
        .iter()
        .filter(|var| matches!(var.source, EnvSource::Secret))
        .map(|var| var.key.clone())
        .collect();

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
        secret_keys,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cellfile::EnvVar;
    use std::collections::HashMap;

    fn var(key: &str, source: EnvSource) -> EnvVar {
        EnvVar {
            key: key.to_string(),
            source,
        }
    }

    /// A lookup backed by a fixed map, standing in for hotcell's own process
    /// environment so these tests never depend on the real one.
    fn lookup_from(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect();
        move |key: &str| map.get(key).cloned()
    }

    #[test]
    fn literal_values_pass_through_verbatim() {
        let env = vec![var("PATH", EnvSource::Literal("/opt/bin".to_string()))];
        let resolved = resolve_environment(&env, |_| None).unwrap();
        assert_eq!(resolved, vec![("PATH".to_string(), "/opt/bin".to_string())]);
    }

    #[test]
    fn passthrough_and_secret_resolve_from_the_host_environment() {
        let env = vec![
            var("PI_PROVIDER", EnvSource::Passthrough),
            var("ANTHROPIC_API_KEY", EnvSource::Secret),
            var("PATH", EnvSource::Literal("/opt/bin".to_string())),
        ];
        let resolved = resolve_environment(
            &env,
            lookup_from(&[
                ("PI_PROVIDER", "anthropic"),
                ("ANTHROPIC_API_KEY", "sk-secret-value"),
            ]),
        )
        .unwrap();
        assert_eq!(
            resolved,
            vec![
                ("PI_PROVIDER".to_string(), "anthropic".to_string()),
                (
                    "ANTHROPIC_API_KEY".to_string(),
                    "sk-secret-value".to_string()
                ),
                ("PATH".to_string(), "/opt/bin".to_string()),
            ]
        );
    }

    #[test]
    fn a_missing_passthrough_is_a_hard_error_naming_the_node() {
        let env = vec![var("PI_PROVIDER", EnvSource::Passthrough)];
        let err = resolve_environment(&env, |_| None).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("env `PI_PROVIDER`"), "{message}");
        assert!(message.contains("not set in hotcell"), "{message}");
    }

    #[test]
    fn a_missing_secret_is_a_hard_error_naming_the_node() {
        let env = vec![var("ANTHROPIC_API_KEY", EnvSource::Secret)];
        let err = resolve_environment(&env, |_| None).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("secret `ANTHROPIC_API_KEY`"), "{message}");
        assert!(message.contains("not set in hotcell"), "{message}");
    }

    #[test]
    fn resolution_stops_at_the_first_missing_var() {
        // A missing var short-circuits with an error rather than silently
        // omitting it, so no partial env ever reaches the agent.
        let env = vec![
            var("PRESENT", EnvSource::Passthrough),
            var("MISSING", EnvSource::Secret),
        ];
        let err =
            resolve_environment(&env, lookup_from(&[("PRESENT", "here")])).unwrap_err();
        assert!(err.to_string().contains("secret `MISSING`"), "{err}");
    }

    /// The literal secret value used across the redaction tests. If it ever
    /// appears in rendered output, redaction has failed.
    const SECRET_VALUE: &str = "sk-ant-super-secret-000";

    /// Build a plan whose agent step sets a literal, a passthrough, and a
    /// secret env var (the secret keyed by `secret_keys`). The `--setenv`
    /// arguments carry the *real* resolved values — exactly what execution
    /// would run — so the render tests exercise display-only redaction.
    fn plan_with_env(secret_keys: Vec<String>) -> ExecutionPlan {
        let cell_fs = PathBuf::from("/var/lib/hotcell/cell/fs");
        let env = vec![
            ("PATH".to_string(), "/opt/bin".to_string()),
            ("PI_PROVIDER".to_string(), "anthropic".to_string()),
            ("ANTHROPIC_API_KEY".to_string(), SECRET_VALUE.to_string()),
        ];
        let agent = PlanStep::new(
            "agent",
            isolation::build_agent_command(
                &cell_fs,
                DEFAULT_WORKDIR,
                &env,
                None,
                "/bin/echo",
                &["hi".to_string()],
            ),
        );
        ExecutionPlan {
            cell_fs,
            workdir: DEFAULT_WORKDIR.to_string(),
            provisioning: None,
            provisioning_skipped: false,
            agent,
            bridge: None,
            secret_keys,
        }
    }

    #[test]
    fn secret_setenv_line_is_present_but_redacted() {
        let rendered = plan_with_env(vec!["ANTHROPIC_API_KEY".to_string()]).render();
        // The line is present so the var is visibly passed, but the value is
        // masked and the real value appears nowhere in the output.
        assert!(
            rendered.contains("--setenv ANTHROPIC_API_KEY <redacted>"),
            "{rendered}"
        );
        assert!(!rendered.contains(SECRET_VALUE), "{rendered}");
    }

    #[test]
    fn literal_and_passthrough_values_are_shown_in_full() {
        let rendered = plan_with_env(vec!["ANTHROPIC_API_KEY".to_string()]).render();
        // Passthrough is not secret; hiding it would obscure what the agent
        // receives. Literals are declared in the Cellfile already.
        assert!(rendered.contains("--setenv PATH /opt/bin"), "{rendered}");
        assert!(
            rendered.contains("--setenv PI_PROVIDER anthropic"),
            "{rendered}"
        );
    }

    #[test]
    fn footer_appears_only_when_a_secret_is_present() {
        let with_secret = plan_with_env(vec!["ANTHROPIC_API_KEY".to_string()]).render();
        assert!(
            with_secret.contains("will NOT run verbatim as shown"),
            "footer missing: {with_secret}"
        );

        // No declared secret -> no footer, and every value shows in full
        // (the block runs verbatim).
        let without_secret = plan_with_env(vec![]).render();
        assert!(!without_secret.contains("<redacted>"), "{without_secret}");
        assert!(
            !without_secret.contains("NOT run verbatim"),
            "{without_secret}"
        );
        assert!(
            without_secret.contains(&format!("--setenv ANTHROPIC_API_KEY {SECRET_VALUE}")),
            "{without_secret}"
        );
    }

    /// End-to-end through `build_execution_plan`: a `secret`-declared var is
    /// resolved from hotcell's own environment, and the rendered plan — the
    /// same string printed on both `run` and `--dry-run`, before either
    /// branches — never contains the resolved value. Uses a uniquely named
    /// env var so it does not collide with other tests.
    #[test]
    fn build_execution_plan_redacts_the_declared_secret() {
        use crate::cellfile::{CellDeclaration, Cellfile};

        const KEY: &str = "HOTCELL_TEST_SECRET_37";
        // SAFETY (edition 2021): single-threaded scope of this value; set and
        // removed within the test. The name is unique to this test.
        std::env::set_var(KEY, SECRET_VALUE);

        let declaration = CellDeclaration {
            environment: vec![var(KEY, EnvSource::Secret)],
            ..CellDeclaration::default()
        };
        let dir = std::env::temp_dir();
        let cellfile = Cellfile {
            file_directory: dir.clone(),
            directory: dir,
            declaration,
        };

        let plan =
            build_execution_plan(&cellfile, "cell37", "/bin/echo", &["hi".to_string()], false)
                .unwrap();
        let rendered = plan.render();

        std::env::remove_var(KEY);

        assert_eq!(plan.secret_keys, vec![KEY.to_string()]);
        assert!(
            rendered.contains(&format!("--setenv {KEY} <redacted>")),
            "{rendered}"
        );
        assert!(!rendered.contains(SECRET_VALUE), "{rendered}");
        assert!(rendered.contains("NOT run verbatim"), "{rendered}");
    }
}

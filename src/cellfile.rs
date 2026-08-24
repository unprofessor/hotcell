//! Cellfile declaration types and parsing.
//!
//! Mirrors the `Cellfile`, `CellDeclaration`, `NetworkPolicy`, `Endpoint`,
//! `FileMapping`, `EnvVar`, and `Provisioner` value types from the
//! specification. The Cellfile is read fresh on every access — the
//! declaration always reflects the current file on disk.
//!
//! On-disk format: a [KDL v2](https://kdl.dev) document. Comments are KDL's
//! (`// line` and `/* block */`), not `#` — a `#` starts a KDL keyword and is
//! a syntax error where a comment used to be. The recognised nodes are:
//!
//! ```kdl
//! workdir "/work"                   // in-sandbox working directory
//!
//! provision "shell" script="./provision.sh" {
//!     host-path "~/.nvm/versions/node"   // repeatable: a host path the
//!     host-path "~/.gitconfig"           // provisioner may read read-only
//! }
//!
//! env PATH="/opt/pi/bin:/usr/bin"   // literal value, written in the file
//! env PI_PROVIDER                   // passthrough: taken from the host env
//! secret ANTHROPIC_API_KEY          // passthrough, redacted in the plan
//!
//! net {
//!     allow "api.anthropic.com" port=443   // repeatable: an allowed endpoint
//!     allow "example.com"                  // port optional: any port
//! }
//! ```
//!
//! The two `env` shapes are told apart by KDL's own args-versus-props
//! distinction, with no extra syntax rule: `env NAME="value"` is a node with
//! one *property* and no arguments, while `env NAME` is a node with one bare
//! *argument* and no properties. A node carrying both is an error rather than
//! a guess. Only the key of a passthrough or secret is ever recorded here —
//! resolving its value against the host environment happens elsewhere, so a
//! secret never enters the declaration and therefore never reaches the
//! execution plan or `provisioned_as.json`.
//!
//! Unknown nodes are errors: the format fails closed, so a typo in a
//! security-relevant directive (`nett { allow ... }`) cannot be silently
//! ignored. `package` and `seed` from the v1 line format are deliberately
//! *not* in this grammar — no provisioner ever acted on them — and naming
//! either yields an error saying so.
//!
//! The format is an implementation detail (excluded from the spec) and may
//! evolve.

use std::fmt::Display;
use std::path::{Path, PathBuf};

use kdl::{KdlDocument, KdlEntry, KdlError, KdlNode};
use serde::{Deserialize, Serialize};

/// `Endpoint` value type: a host and optional port.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Endpoint {
    pub host: String,
    pub port: Option<u16>,
}

/// `NetworkPolicy` value type. An empty `allowed_endpoints` set means the
/// cell is offline — no network access.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct NetworkPolicy {
    pub allowed_endpoints: Vec<Endpoint>,
}

/// `FileMapping` value type: a host source path seeded into the sandbox.
///
/// The grammar has no syntax for these yet — the v1 `seed` key was dropped
/// rather than ported, because no provisioner read it. The type stays because
/// the specification still carries it; whether the capability returns, and in
/// what shape, is `packages-seeds-semantics`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMapping {
    pub source: String,
    pub target: String,
}

/// Where an environment variable's value comes from.
///
/// The declaration records the *source*, never a resolved value: a
/// `Passthrough` or `Secret` var contributes only its key here, so a secret
/// cannot leak into the execution plan, into `provisioned_as.json`, or into
/// the declaration comparison that decides whether a cell needs
/// re-provisioning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnvSource {
    /// `env NAME="value"` — the value is written in the Cellfile.
    Literal(String),
    /// `env NAME` — take the value from the host environment at run time.
    Passthrough,
    /// `secret NAME` — passthrough, and redacted wherever the run is
    /// displayed.
    Secret,
}

/// `EnvVar` value type: a key plus where its value comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvVar {
    pub key: String,
    pub source: EnvSource,
}

/// `Provisioner` value type: declares which provisioner brings the cell's
/// environment into being. `kind` is an open set; `script` is the path to the
/// provisioning script (relative to the Cellfile directory), used only by the
/// `"shell"` kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provisioner {
    pub kind: String,
    pub script: Option<String>,
    pub host_paths: Vec<String>,
}

impl Default for Provisioner {
    fn default() -> Self {
        Self {
            kind: crate::state::DEFAULT_PROVISIONER_KIND.to_string(),
            script: None,
            host_paths: Vec::new(),
        }
    }
}

/// `CellDeclaration` value type: the full content of a Cellfile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CellDeclaration {
    pub packages: Vec<String>,
    pub files: Vec<FileMapping>,
    pub environment: Vec<EnvVar>,
    pub network: NetworkPolicy,
    pub workdir: String,
    #[serde(default)]
    pub provisioner: Provisioner,
}

/// `Cellfile` external entity: a declaration file at
/// `{file_directory}/Cellfile`.
///
/// The two directories serve different roles: [`Cellfile::file_directory`]
/// is where the declaration file actually lives — the base for resolving the
/// `provision` script and relative `host-path` entries — while
/// [`Cellfile::directory`] is the root for cell state (`.cell/...`).
/// They start equal, but `--cell` overrides only the state root, so a cell
/// can be run from a Cellfile in one place with its state kept in another.
#[derive(Debug, Clone)]
pub struct Cellfile {
    /// Directory of the Cellfile itself. Relative paths in the declaration
    /// (the provision script, host paths) resolve against this.
    pub file_directory: PathBuf,
    /// Root for cell state (`.cell/...`). Equals `file_directory` unless
    /// `--cell` overrides it.
    pub directory: PathBuf,
    pub declaration: CellDeclaration,
}

impl Cellfile {
    /// Read the Cellfile from `directory`, fresh from disk. An empty Cellfile
    /// yields a default declaration (no-op provisioner, offline, empty env).
    /// The Cellfile is expected at `{directory}/Cellfile`.
    pub fn read(directory: &Path) -> anyhow::Result<Self> {
        Self::read_at(&directory.join("Cellfile"))
    }

    /// Read the Cellfile at an explicit `path`, fresh from disk. Both the
    /// file directory and the state root start at the file's parent, so
    /// state lives alongside the Cellfile unless `--cell` moves it. An
    /// empty Cellfile yields a default declaration.
    pub fn read_at(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            anyhow::bail!("no Cellfile found at {}", path.display());
        }
        let text = std::fs::read_to_string(path)?;
        let declaration = parse(&text, path)?;
        let directory = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        Ok(Self {
            file_directory: directory.clone(),
            directory,
            declaration,
        })
    }
}

/// Parse Cellfile text into a `CellDeclaration`.
///
/// Every recognised node is handled explicitly and anything else is an
/// error, so the parser fails closed. An empty document is valid and yields
/// the defaults: the no-op provisioner, no declared env, and an empty
/// allowlist (offline). The effective workdir default (`/work`) is applied
/// by the plan builder, which owns the run's defaults.
fn parse(text: &str, path: &Path) -> anyhow::Result<CellDeclaration> {
    let document: KdlDocument = text
        .parse()
        .map_err(|error: KdlError| syntax_error(&error, text, path))?;

    let mut decl = CellDeclaration::default();
    // Byte offsets of the first occurrence of each single-valued node, so a
    // second occurrence can be rejected citing both. `env`, `secret`,
    // `host-path` and `allow` are *not* tracked here — they accumulate by
    // design.
    let mut seen = SingleValued::default();
    for node in document.nodes() {
        match node.name().value() {
            "workdir" => {
                seen.workdir.check(node, "workdir", text, path)?;
                reject_children(node, text, path)?;
                reject_properties(node, text, path)?;
                decl.workdir = only_string_argument(node, "workdir", text, path)?;
            }
            "provision" => {
                seen.provision.check(node, "provision", text, path)?;
                decl.provisioner = parse_provision(node, text, path)?;
            }
            "env" => decl.environment.push(parse_env(node, text, path)?),
            "secret" => {
                reject_children(node, text, path)?;
                reject_properties(node, text, path)?;
                let key = only_string_argument(node, "secret", text, path)?;
                decl.environment.push(EnvVar {
                    key: env_key(&key, node, text, path)?,
                    source: EnvSource::Secret,
                });
            }
            "net" => {
                seen.net.check(node, "net", text, path)?;
                parse_net(node, &mut decl.network, text, path)?;
            }
            // Dropped from the grammar rather than ported: they parsed in the
            // v1 line format but no provisioner ever acted on them. Named
            // explicitly so the error says "gone", not "unknown".
            dropped @ ("package" | "seed") => {
                return Err(error_at(
                    path,
                    text,
                    node,
                    format!(
                        "`{dropped}` is not implemented — it was removed from the Cellfile \
                         grammar because no provisioner acted on it; have the provision \
                         script do this work instead"
                    ),
                ))
            }
            unknown => {
                return Err(error_at(
                    path,
                    text,
                    node,
                    format!("unknown Cellfile node `{unknown}`"),
                ))
            }
        }
    }
    Ok(decl)
}

/// First-occurrence tracker for the single-valued nodes. `workdir`,
/// `provision` and `net` may each appear at most once: a second `workdir` or
/// `provision` would silently override the first (last-wins), and a second
/// `net` would silently *merge* its `allow` children into the first block's
/// allowlist. Both are guesses the grammar refuses to make, so a repeat is a
/// located error naming the node and citing both occurrences.
#[derive(Default)]
struct SingleValued {
    workdir: FirstSeen,
    provision: FirstSeen,
    net: FirstSeen,
}

/// The byte offset of a node's first occurrence, or `None` until it is seen.
#[derive(Default)]
struct FirstSeen(Option<usize>);

impl FirstSeen {
    /// Record `node` as the first occurrence, or fail if one was already
    /// recorded. The error points at the duplicate and cites the first
    /// declaration's `line:col`.
    fn check(&mut self, node: &KdlNode, name: &str, text: &str, path: &Path) -> anyhow::Result<()> {
        match self.0 {
            None => {
                self.0 = Some(node.start());
                Ok(())
            }
            Some(offset) => {
                let (line, column) = line_column(text, offset);
                Err(error_at(
                    path,
                    text,
                    node,
                    format!(
                        "`{name}` may be declared only once, but is declared again here; \
                         first declared at {}:{line}:{column}",
                        path.display()
                    ),
                ))
            }
        }
    }
}

/// Parse `provision "kind" script="..." { host-path "..." }`.
fn parse_provision(node: &KdlNode, text: &str, path: &Path) -> anyhow::Result<Provisioner> {
    let kind = only_string_argument(node, "provision", text, path)?;
    let mut provisioner = Provisioner {
        kind,
        script: None,
        host_paths: Vec::new(),
    };

    for entry in properties(node) {
        let name = property_name(entry);
        match name {
            "script" => provisioner.script = Some(string_value(entry, "script", text, path)?),
            other => {
                return Err(error_at(
                    path,
                    text,
                    entry,
                    format!("unknown property `{other}` on `provision`"),
                ))
            }
        }
    }

    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().value() {
                "host-path" => {
                    reject_children(child, text, path)?;
                    reject_properties(child, text, path)?;
                    provisioner.host_paths.push(only_string_argument(
                        child,
                        "host-path",
                        text,
                        path,
                    )?);
                }
                unknown => {
                    return Err(error_at(
                        path,
                        text,
                        child,
                        format!("unknown node `{unknown}` inside `provision`"),
                    ))
                }
            }
        }
    }
    Ok(provisioner)
}

/// Parse `env NAME="value"` (a literal) or `env NAME` (a passthrough).
///
/// The two shapes are distinguished by KDL's own args-versus-props split:
/// a literal is one property and no arguments, a passthrough is one argument
/// and no properties. A node carrying both is ambiguous and rejected rather
/// than guessed at.
fn parse_env(node: &KdlNode, text: &str, path: &Path) -> anyhow::Result<EnvVar> {
    reject_children(node, text, path)?;
    let arguments = arguments(node);
    let properties = properties(node);

    match (arguments.as_slice(), properties.as_slice()) {
        ([], [property]) => Ok(EnvVar {
            key: env_key(property_name(property), node, text, path)?,
            source: EnvSource::Literal(string_value(property, "env value", text, path)?),
        }),
        ([argument], []) => {
            let key = string_value(argument, "env name", text, path)?;
            Ok(EnvVar {
                key: env_key(&key, node, text, path)?,
                source: EnvSource::Passthrough,
            })
        }
        ([], []) => Err(error_at(
            path,
            text,
            node,
            "`env` needs a variable: `env NAME=\"value\"` or `env NAME`",
        )),
        _ => Err(error_at(
            path,
            text,
            node,
            "`env` takes either a literal `NAME=\"value\"` or a bare `NAME` to pass \
             through from the host environment, not both",
        )),
    }
}

/// Parse `net { allow "host" port=443 }`.
fn parse_net(
    node: &KdlNode,
    policy: &mut NetworkPolicy,
    text: &str,
    path: &Path,
) -> anyhow::Result<()> {
    reject_properties(node, text, path)?;
    if !arguments(node).is_empty() {
        return Err(error_at(
            path,
            text,
            node,
            "`net` takes no arguments — declare endpoints as `net { allow \"host\" }`",
        ));
    }

    let Some(children) = node.children() else {
        // `net { }` with nothing in it is the same as no `net` at all: an
        // empty allowlist, which means offline.
        return Ok(());
    };

    for child in children.nodes() {
        match child.name().value() {
            "allow" => {
                reject_children(child, text, path)?;
                let host = only_string_argument(child, "allow", text, path)?;
                let mut port = None;
                for entry in properties(child) {
                    match property_name(entry) {
                        "port" => port = Some(port_value(entry, text, path)?),
                        other => {
                            return Err(error_at(
                                path,
                                text,
                                entry,
                                format!("unknown property `{other}` on `allow`"),
                            ))
                        }
                    }
                }
                // The host is taken verbatim — no `host:port` splitting, so
                // an IPv6 literal such as `::1` survives intact.
                policy.allowed_endpoints.push(Endpoint { host, port });
            }
            unknown => {
                return Err(error_at(
                    path,
                    text,
                    child,
                    format!("unknown node `{unknown}` inside `net`"),
                ))
            }
        }
    }
    Ok(())
}

/// A node's positional arguments, in source order.
fn arguments(node: &KdlNode) -> Vec<&KdlEntry> {
    node.entries()
        .iter()
        .filter(|entry| entry.name().is_none())
        .collect()
}

/// A node's named properties, in source order.
fn properties(node: &KdlNode) -> Vec<&KdlEntry> {
    node.entries()
        .iter()
        .filter(|entry| entry.name().is_some())
        .collect()
}

/// The name of a property entry. Only call this on entries from
/// [`properties`], which are named by construction.
fn property_name(entry: &KdlEntry) -> &str {
    entry.name().map(|name| name.value()).unwrap_or_default()
}

/// Require exactly one string argument, e.g. the `"/work"` of `workdir`.
fn only_string_argument(
    node: &KdlNode,
    what: &str,
    text: &str,
    path: &Path,
) -> anyhow::Result<String> {
    let arguments = arguments(node);
    let [argument] = arguments.as_slice() else {
        return Err(error_at(
            path,
            text,
            node,
            format!("`{what}` takes exactly one value, got {}", arguments.len()),
        ));
    };
    string_value(argument, what, text, path)
}

/// Read an entry's value as a string. Bare identifiers (`env PI_PROVIDER`)
/// and quoted strings both arrive as KDL strings; numbers and booleans do
/// not, and are rejected rather than stringified.
fn string_value(entry: &KdlEntry, what: &str, text: &str, path: &Path) -> anyhow::Result<String> {
    entry
        .value()
        .as_string()
        .map(str::to_string)
        .ok_or_else(|| {
            error_at(
                path,
                text,
                entry,
                format!("`{what}` must be a string, got `{}`", entry.value()),
            )
        })
}

/// Read a `port=` property as a `u16`. Port 0 is rejected: it means
/// "any port" to the kernel, which the allowlist expresses by omitting the
/// property entirely.
fn port_value(entry: &KdlEntry, text: &str, path: &Path) -> anyhow::Result<u16> {
    let value = entry.value();
    let number = value
        .as_integer()
        .ok_or_else(|| error_at(path, text, entry, format!("invalid port `{value}`")))?;
    u16::try_from(number)
        .ok()
        .filter(|port| *port != 0)
        .ok_or_else(|| {
            error_at(
                path,
                text,
                entry,
                format!("invalid port `{number}` — must be between 1 and 65535"),
            )
        })
}

/// Validate an environment variable name. POSIX forbids `=` and NUL in a
/// name, and an empty name cannot be exported at all; everything else is
/// left to the host.
fn env_key(key: &str, node: &KdlNode, text: &str, path: &Path) -> anyhow::Result<String> {
    if key.is_empty() || key.contains('=') || key.contains('\0') {
        return Err(error_at(
            path,
            text,
            node,
            format!("`{key}` is not a usable environment variable name"),
        ));
    }
    Ok(key.to_string())
}

/// Reject a child block on a node that does not take one.
fn reject_children(node: &KdlNode, text: &str, path: &Path) -> anyhow::Result<()> {
    if node.children().is_some() {
        return Err(error_at(
            path,
            text,
            node,
            format!("`{}` takes no child block", node.name().value()),
        ));
    }
    Ok(())
}

/// Reject `key=value` properties on a node that does not take any.
fn reject_properties(node: &KdlNode, text: &str, path: &Path) -> anyhow::Result<()> {
    if let Some(entry) = properties(node).first() {
        return Err(error_at(
            path,
            text,
            *entry,
            format!(
                "`{}` takes no `{}=` property",
                node.name().value(),
                property_name(entry)
            ),
        ));
    }
    Ok(())
}

/// Anything with a source span: the node or entry an error points at.
trait Spanned {
    fn start(&self) -> usize;
}

impl Spanned for KdlNode {
    fn start(&self) -> usize {
        self.span().offset()
    }
}

impl Spanned for KdlEntry {
    fn start(&self) -> usize {
        self.span().offset()
    }
}

/// Build a `path:line:col: message` error pointing at a piece of the
/// document. Errors are plain `anyhow` values: kdl's miette diagnostics are
/// flattened to text here so hotcell never has to render them, and the
/// `fancy` feature (with its terminal-drawing dependencies) stays out of the
/// build.
fn error_at(
    path: &Path,
    text: &str,
    spanned: &impl Spanned,
    message: impl Display,
) -> anyhow::Error {
    let (line, column) = line_column(text, spanned.start());
    anyhow::anyhow!("{}:{line}:{column}: {message}", path.display())
}

/// Flatten a kdl parse failure into one `anyhow` error, one line per
/// diagnostic, each located as `path:line:col`.
fn syntax_error(error: &KdlError, text: &str, path: &Path) -> anyhow::Error {
    let mut lines: Vec<String> = error
        .diagnostics
        .iter()
        .map(|diagnostic| {
            let (line, column) = line_column(text, diagnostic.span.offset());
            let message = diagnostic
                .message
                .clone()
                .unwrap_or_else(|| "invalid KDL".to_string());
            format!("{}:{line}:{column}: {message}", path.display())
        })
        .collect();
    if lines.is_empty() {
        lines.push(format!("{}: invalid KDL", path.display()));
    }
    if let Some(hint) = old_format_hint(text) {
        lines.push(hint);
    }
    anyhow::anyhow!(lines.join("\n"))
}

/// Recognise a Cellfile still written in the v1 line format and say so,
/// rather than leaving the reader with a bare KDL syntax error. This is a
/// diagnostic only — the old format is gone, and nothing here parses it.
fn old_format_hint(text: &str) -> Option<String> {
    const V1_KEYS: [&str; 7] = [
        "provision.type",
        "provision.script",
        "provision.host_path",
        "workdir",
        "package",
        "net.allow",
        "seed",
    ];

    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        let is_v1 = if line.starts_with('#') {
            // The v1 format's comment character. KDL uses `//`.
            true
        } else if let Some((key, _)) = line.split_once('=') {
            let key = key.trim();
            key.starts_with("env.") || V1_KEYS.contains(&key)
        } else {
            false
        };
        if is_v1 {
            return Some(format!(
                "hint: line {} (`{line}`) looks like the old `key = value` Cellfile \
                 format. Cellfiles are now KDL v2: `provision \"shell\" \
                 script=\"./provision.sh\"`, `workdir \"/work\"`, `env NAME=\"value\"`, \
                 `net {{ allow \"host\" port=443 }}`, and comments start with `//`.",
                index + 1
            ));
        }
    }
    None
}

/// Convert a byte offset into the 1-based line and column a reader can find
/// in an editor. Columns count characters, not bytes.
fn line_column(text: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;
    for (index, character) in text.char_indices() {
        if index >= offset {
            break;
        }
        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> PathBuf {
        PathBuf::from("/tmp/x/Cellfile")
    }

    #[test]
    fn empty_cellfile_is_default() {
        let t = tempfile::tempdir().unwrap();
        std::fs::write(t.path().join("Cellfile"), "").unwrap();
        let d = Cellfile::read(t.path()).unwrap();
        assert_eq!(d.declaration.provisioner.kind, "none");
        assert!(d.declaration.environment.is_empty());
        assert!(d.declaration.network.allowed_endpoints.is_empty());
        assert!(d.declaration.workdir.is_empty());
    }

    #[test]
    fn parses_all_nodes() {
        let text = r#"
// a comment
workdir "/work"

provision "shell" script="./provision.sh" {
    host-path "~/.nvm/versions/node"
    host-path "~/.cargo"
}

env PATH="/opt/node/bin:/usr/bin"
env HOME="/home/agent"
env PI_PROVIDER
secret ANTHROPIC_API_KEY

net {
    allow "api.openai.com" port=443
    allow "api.anthropic.com"
}
"#;
        let decl = parse(text, &dir()).unwrap();
        assert_eq!(decl.provisioner.kind, "shell");
        assert_eq!(decl.provisioner.script.as_deref(), Some("./provision.sh"));
        assert_eq!(
            decl.provisioner.host_paths,
            vec!["~/.nvm/versions/node", "~/.cargo"]
        );
        assert_eq!(decl.workdir, "/work");
        assert_eq!(
            decl.environment,
            vec![
                EnvVar {
                    key: "PATH".to_string(),
                    source: EnvSource::Literal("/opt/node/bin:/usr/bin".to_string()),
                },
                EnvVar {
                    key: "HOME".to_string(),
                    source: EnvSource::Literal("/home/agent".to_string()),
                },
                EnvVar {
                    key: "PI_PROVIDER".to_string(),
                    source: EnvSource::Passthrough,
                },
                EnvVar {
                    key: "ANTHROPIC_API_KEY".to_string(),
                    source: EnvSource::Secret,
                },
            ]
        );
        assert_eq!(decl.network.allowed_endpoints.len(), 2);
        assert_eq!(decl.network.allowed_endpoints[0].host, "api.openai.com");
        assert_eq!(decl.network.allowed_endpoints[0].port, Some(443));
        assert_eq!(decl.network.allowed_endpoints[1].host, "api.anthropic.com");
        assert_eq!(decl.network.allowed_endpoints[1].port, None);
        // Dropped from the grammar; nothing can populate them.
        assert!(decl.packages.is_empty());
        assert!(decl.files.is_empty());
    }

    #[test]
    fn provision_none_and_empty_document_agree() {
        let explicit = parse("provision \"none\"\n", &dir()).unwrap();
        assert_eq!(explicit, parse("", &dir()).unwrap());
        assert_eq!(explicit.provisioner.kind, "none");
        assert!(explicit.provisioner.script.is_none());
    }

    #[test]
    fn env_node_with_both_an_argument_and_a_property_is_an_error() {
        let err = parse("env FOO=\"bar\" BAZ\n", &dir()).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("/tmp/x/Cellfile:1:1:"), "{message}");
        assert!(message.contains("not both"), "{message}");
    }

    #[test]
    fn env_with_nothing_is_an_error() {
        let err = parse("env\n", &dir()).unwrap_err();
        assert!(err.to_string().contains("`env` needs a variable"));
    }

    #[test]
    fn ipv6_endpoints_keep_their_colons() {
        // The v1 format split `host:port` on the last colon, which silently
        // mangled every IPv6 literal. Host and port are separate now.
        let text = r#"
net {
    allow "::1"
    allow "2001:db8::1" port=8443
    allow "fe80::1%eth0"
}
"#;
        let decl = parse(text, &dir()).unwrap();
        assert_eq!(
            decl.network.allowed_endpoints,
            vec![
                Endpoint {
                    host: "::1".to_string(),
                    port: None
                },
                Endpoint {
                    host: "2001:db8::1".to_string(),
                    port: Some(8443)
                },
                Endpoint {
                    host: "fe80::1%eth0".to_string(),
                    port: None
                },
            ]
        );
    }

    #[test]
    fn bad_port_is_an_error() {
        let err = parse("net {\n    allow \"host\" port=99999\n}\n", &dir()).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("invalid port `99999`"), "{message}");
        assert!(message.contains("/tmp/x/Cellfile:2:18:"), "{message}");

        let err = parse("net {\n    allow \"host\" port=\"443\"\n}\n", &dir()).unwrap_err();
        assert!(err.to_string().contains("invalid port"), "{err}");
    }

    #[test]
    fn unknown_nodes_are_errors() {
        let err = parse("frobnicate \"yes\"\n", &dir()).unwrap_err();
        assert!(err
            .to_string()
            .contains("unknown Cellfile node `frobnicate`"));

        let err = parse("net {\n    deny \"host\"\n}\n", &dir()).unwrap_err();
        assert!(err.to_string().contains("unknown node `deny` inside `net`"));

        let err = parse("provision \"shell\" scrpit=\"./p.sh\"\n", &dir()).unwrap_err();
        assert!(err
            .to_string()
            .contains("unknown property `scrpit` on `provision`"));
    }

    #[test]
    fn package_and_seed_report_that_they_are_gone() {
        let err = parse("package \"node\"\n", &dir()).unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("`package` is not implemented"),
            "{message}"
        );

        let err = parse("seed \"./src\"\n", &dir()).unwrap_err();
        assert!(
            err.to_string().contains("`seed` is not implemented"),
            "{err}"
        );
    }

    #[test]
    fn syntax_errors_are_located() {
        let err = parse("workdir \"/work\"\nnet {\n", &dir()).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("/tmp/x/Cellfile:2:"), "{message}");
    }

    #[test]
    fn a_v1_cellfile_gets_a_migration_hint() {
        let text = "\
provision.type = shell
workdir = /work
env.PATH = /usr/bin
";
        let err = parse(text, &dir()).unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("looks like the old `key = value` Cellfile format"),
            "{message}"
        );
        assert!(
            message.contains("line 1 (`provision.type = shell`)"),
            "{message}"
        );
        assert!(message.contains("comments start with `//`"), "{message}");
    }

    #[test]
    fn a_v1_hash_comment_gets_a_migration_hint() {
        let err = parse("# a v1 comment\nworkdir \"/work\"\n", &dir()).unwrap_err();
        assert!(
            err.to_string().contains("comments start with `//`"),
            "{err}"
        );
    }

    #[test]
    fn duplicate_workdir_is_an_error_citing_both() {
        let err = parse("workdir \"/a\"\nworkdir \"/b\"\n", &dir()).unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("`workdir` may be declared only once"),
            "{message}"
        );
        // Points at the second occurrence...
        assert!(message.contains("/tmp/x/Cellfile:2:1:"), "{message}");
        // ...and cites the first.
        assert!(
            message.contains("first declared at /tmp/x/Cellfile:1:1"),
            "{message}"
        );
    }

    #[test]
    fn duplicate_provision_is_an_error_citing_both() {
        // The bug from the ticket: last-wins silently dropped the first
        // block's `host-path "/etc/hostname"`.
        let text = "\
provision \"shell\" script=\"./p.sh\" { host-path \"/etc/hostname\" }
provision \"shell\" script=\"./p.sh\" { host-path \"/etc/machine-id\" }
";
        let err = parse(text, &dir()).unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("`provision` may be declared only once"),
            "{message}"
        );
        assert!(message.contains("/tmp/x/Cellfile:2:1:"), "{message}");
        assert!(
            message.contains("first declared at /tmp/x/Cellfile:1:1"),
            "{message}"
        );
    }

    #[test]
    fn duplicate_net_is_an_error_citing_both() {
        // Two `net` blocks previously *merged* their `allow` children into one
        // allowlist with no warning; now the second block is rejected.
        let text = "\
net { allow \"a.com\" }
net { allow \"b.com\" }
";
        let err = parse(text, &dir()).unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("`net` may be declared only once"),
            "{message}"
        );
        assert!(message.contains("/tmp/x/Cellfile:2:1:"), "{message}");
        assert!(
            message.contains("first declared at /tmp/x/Cellfile:1:1"),
            "{message}"
        );
    }

    #[test]
    fn multi_valued_nodes_still_accumulate() {
        // env, secret, host-path and allow accumulate by design and must keep
        // doing so — the opposite of the single-valued nodes above.
        let text = "\
provision \"shell\" script=\"./p.sh\" {
    host-path \"/etc/hostname\"
    host-path \"/etc/machine-id\"
}
env FOO=\"1\"
env BAR=\"2\"
env PASS_ME
secret TOKEN_A
secret TOKEN_B
net {
    allow \"a.com\"
    allow \"b.com\"
}
";
        let decl = parse(text, &dir()).unwrap();
        assert_eq!(
            decl.provisioner.host_paths,
            vec!["/etc/hostname", "/etc/machine-id"]
        );
        assert_eq!(
            decl.environment,
            vec![
                EnvVar {
                    key: "FOO".to_string(),
                    source: EnvSource::Literal("1".to_string())
                },
                EnvVar {
                    key: "BAR".to_string(),
                    source: EnvSource::Literal("2".to_string())
                },
                EnvVar {
                    key: "PASS_ME".to_string(),
                    source: EnvSource::Passthrough
                },
                EnvVar {
                    key: "TOKEN_A".to_string(),
                    source: EnvSource::Secret
                },
                EnvVar {
                    key: "TOKEN_B".to_string(),
                    source: EnvSource::Secret
                },
            ]
        );
        assert_eq!(
            decl.network.allowed_endpoints,
            vec![
                Endpoint {
                    host: "a.com".to_string(),
                    port: None
                },
                Endpoint {
                    host: "b.com".to_string(),
                    port: None
                },
            ]
        );
    }
}

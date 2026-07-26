//! Cellfile declaration types and parsing.
//!
//! Mirrors the `Cellfile`, `CellDeclaration`, `NetworkPolicy`, `Endpoint`,
//! `FileMapping`, `EnvVar`, and `Provisioner` value types from the
//! specification. The Cellfile is read fresh on every access — the
//! declaration always reflects the current file on disk.
//!
//! On-disk format (v1): a minimal line-based, dotted-key text format. Blank
//! lines and `#` comments are ignored. Keys are dotted identifiers; the
//! recognised keys are:
//!
//! ```text
//! provision.type = shell            # provisioner kind (default: "none")
//! provision.script = ./provision.sh # script path, relative to the Cellfile
//! provision.host_path = ~/.nvm      # repeatable: host path the provisioner
//!                                  # may read (read-only) for bootstrapping
//! workdir = /work                   # in-sandbox working directory
//! package = node                    # repeatable: a package to install
//! package = curl
//! env.KEY = value                   # repeatable: an environment variable
//! net.allow = host:443              # repeatable: an allowed endpoint
//! net.allow = host                  #   (port optional)
//! seed = source => target           # repeatable: a file mapping
//! ```
//!
//! The format is intentionally minimal. It is an implementation detail
//! (excluded from the spec) and may evolve.

use std::path::{Path, PathBuf};

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMapping {
    pub source: String,
    pub target: String,
}

/// `EnvVar` value type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
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

/// `Cellfile` external entity: a declaration file at `{directory}/Cellfile`.
#[derive(Debug, Clone)]
pub struct Cellfile {
    pub directory: PathBuf,
    pub declaration: CellDeclaration,
}

impl Cellfile {
    /// Path to the Cellfile within its directory.
    pub fn path(&self) -> PathBuf {
        self.directory.join("Cellfile")
    }

    /// Read the Cellfile from `directory`, fresh from disk. An empty Cellfile
    /// yields a default declaration (no-op provisioner, offline, empty env).
    /// The Cellfile is expected at `{directory}/Cellfile`.
    pub fn read(directory: &Path) -> anyhow::Result<Self> {
        Self::read_at(&directory.join("Cellfile"))
    }

    /// Read the Cellfile at an explicit `path`, fresh from disk. The cell's
    /// directory (used for state and bind-mounts) is the file's parent, so
    /// state continues to live alongside the Cellfile. An empty Cellfile
    /// yields a default declaration.
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
            directory,
            declaration,
        })
    }
}

/// Parse Cellfile text into a `CellDeclaration`.
fn parse(text: &str, path: &Path) -> anyhow::Result<CellDeclaration> {
    let mut decl = CellDeclaration::default();
    for (i, raw) in text.lines().enumerate() {
        let line_no = i + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = split_kv(raw, line_no, path)?;
        let key = key.trim();
        let value = value.trim();

        if key == "provision.type" {
            decl.provisioner.kind = value.to_string();
        } else if key == "provision.script" {
            decl.provisioner.script = Some(value.to_string());
        } else if key == "provision.host_path" {
            decl.provisioner.host_paths.push(value.to_string());
        } else if key == "workdir" {
            decl.workdir = value.to_string();
        } else if key == "package" {
            decl.packages.push(value.to_string());
        } else if let Some(env_key) = key.strip_prefix("env.") {
            decl.environment.push(EnvVar {
                key: env_key.to_string(),
                value: value.to_string(),
            });
        } else if key == "net.allow" {
            decl.network
                .allowed_endpoints
                .push(parse_endpoint(value, line_no, path)?);
        } else if key == "seed" {
            decl.files.push(parse_file_mapping(value, line_no, path)?);
        } else {
            anyhow::bail!(
                "{}:{}: unknown Cellfile key `{key}`",
                path.display(),
                line_no
            );
        }
    }
    Ok(decl)
}

/// Split a line into `(key, value)` on the first `=`.
fn split_kv(raw: &str, line_no: usize, path: &Path) -> anyhow::Result<(String, String)> {
    let Some(eq) = raw.find('=') else {
        anyhow::bail!("{}:{}: expected `key = value`", path.display(), line_no);
    };
    Ok((raw[..eq].to_string(), raw[eq + 1..].to_string()))
}

/// Parse `host` or `host:port` into an `Endpoint`.
fn parse_endpoint(s: &str, line_no: usize, path: &Path) -> anyhow::Result<Endpoint> {
    if let Some((host, port)) = s.rsplit_once(':') {
        let port: u16 = port.parse().map_err(|_| {
            anyhow::anyhow!("{}:{}: invalid port in `{s}`", path.display(), line_no)
        })?;
        Ok(Endpoint {
            host: host.to_string(),
            port: Some(port),
        })
    } else {
        Ok(Endpoint {
            host: s.to_string(),
            port: None,
        })
    }
}

/// Parse `source => target` into a `FileMapping`.
fn parse_file_mapping(s: &str, line_no: usize, path: &Path) -> anyhow::Result<FileMapping> {
    let Some((source, target)) = s.split_once("=>") else {
        anyhow::bail!(
            "{}:{}: file mapping expects `source => target`, got `{s}`",
            path.display(),
            line_no
        );
    };
    Ok(FileMapping {
        source: source.trim().to_string(),
        target: target.trim().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> PathBuf {
        PathBuf::from("/tmp/x")
    }

    #[test]
    fn empty_cellfile_is_default() {
        let t = tempfile::tempdir().unwrap();
        std::fs::write(t.path().join("Cellfile"), "").unwrap();
        let d = Cellfile::read(t.path()).unwrap();
        assert_eq!(d.declaration.provisioner.kind, "none");
        assert!(d.declaration.packages.is_empty());
    }

    #[test]
    fn parses_all_keys() {
        let text = "\
# a comment
provision.type = shell
provision.script = ./provision.sh
provision.host_path = ~/.nvm/versions/node
provision.host_path = ~/.cargo
workdir = /work

package = node
package = curl

env.PATH = /opt/node/bin:/usr/bin
env.HOME = /home/agent

net.allow = api.openai.com:443
net.allow = api.anthropic.com

seed = ./src => /work/src
seed = ~/.gitconfig => /home/agent/.gitconfig
";
        let decl = parse(text, &dir()).unwrap();
        assert_eq!(decl.provisioner.kind, "shell");
        assert_eq!(decl.provisioner.script.as_deref(), Some("./provision.sh"));
        assert_eq!(
            decl.provisioner.host_paths,
            vec!["~/.nvm/versions/node", "~/.cargo"]
        );
        assert_eq!(decl.workdir, "/work");
        assert_eq!(decl.packages, vec!["node", "curl"]);
        assert_eq!(
            decl.environment
                .iter()
                .map(|e| (e.key.as_str(), e.value.as_str()))
                .collect::<Vec<_>>(),
            vec![("PATH", "/opt/node/bin:/usr/bin"), ("HOME", "/home/agent")]
        );
        assert_eq!(decl.network.allowed_endpoints.len(), 2);
        assert_eq!(decl.network.allowed_endpoints[0].host, "api.openai.com");
        assert_eq!(decl.network.allowed_endpoints[0].port, Some(443));
        assert_eq!(decl.network.allowed_endpoints[1].port, None);
        assert_eq!(decl.files.len(), 2);
        assert_eq!(decl.files[0].source, "./src");
        assert_eq!(decl.files[0].target, "/work/src");
    }

    #[test]
    fn unknown_key_is_an_error() {
        let err = parse("frobnicate = yes", &dir()).unwrap_err();
        assert!(err
            .to_string()
            .contains("unknown Cellfile key `frobnicate`"));
    }

    #[test]
    fn bad_port_is_an_error() {
        let err = parse("net.allow = host:notaport", &dir()).unwrap_err();
        assert!(err.to_string().contains("invalid port"));
    }
}

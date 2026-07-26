//! Cellfile declaration types and parsing.
//!
//! Mirrors the `Cellfile`, `CellDeclaration`, `NetworkPolicy`, `Endpoint`,
//! `FileMapping`, and `EnvVar` value types from the specification. The
//! Cellfile is read fresh on every access — the declaration always reflects
//! the current file on disk.

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

/// `CellDeclaration` value type: the full content of a Cellfile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CellDeclaration {
    pub packages: Vec<String>,
    pub files: Vec<FileMapping>,
    pub environment: Vec<EnvVar>,
    pub network: NetworkPolicy,
    pub workdir: String,
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

    /// Read the Cellfile from `directory`, fresh from disk.
    ///
    /// TODO(v1): define the on-disk Cellfile format. For now this is a stub
    /// that returns a default declaration so the rest of the pipeline can be
    /// wired up.
    pub fn read(directory: &Path) -> anyhow::Result<Self> {
        let path = directory.join("Cellfile");
        if !path.exists() {
            anyhow::bail!("no Cellfile found at {}", path.display());
        }
        // TODO: parse the Cellfile format.
        Ok(Self {
            directory: directory.to_path_buf(),
            declaration: CellDeclaration::default(),
        })
    }
}

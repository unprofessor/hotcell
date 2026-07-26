//! Session entity and lifecycle.
//!
//! Mirrors the `Session` entity from the specification. A session is a single
//! program invocation inside a provisioned cell.

use serde::{Deserialize, Serialize};

/// Session status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Running,
    Completed,
}

impl SessionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SessionStatus::Running => "running",
            SessionStatus::Completed => "completed",
        }
    }
}

/// A session: one program invocation inside a cell.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub program: String,
    pub args: Vec<String>,
    pub status: SessionStatus,
    pub started_at: u64,
    pub ended_at: Option<u64>,
    pub exit_code: Option<i32>,
    pub log_file: Option<String>,
}

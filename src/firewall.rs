//! Network firewall via an HTTP allowlist proxy.
//!
//! Mirrors the `NetworkFirewall` guarantee: the agent can only reach
//! endpoints in the cell's provisioned network policy. All other network
//! connections are blocked. Enforcement is via an HTTP proxy; non-HTTP
//! traffic is blocked by default. A cell with no allowed endpoints has no
//! network access.
//!
//! TODO(v1): implement a local HTTP proxy (hyper) that checks each request's
//! destination host/port against `NetworkPolicy::allowed_endpoints` and
//! refuses everything else. The agent is launched with `HTTP_PROXY` /
//! `HTTPS_PROXY` pointing at this proxy and non-HTTP egress is blocked by
//! the sandbox's network namespace.

use crate::cellfile::NetworkPolicy;

/// A handle to the running firewall proxy for a session.
pub struct FirewallHandle {
    // TODO: hold the proxy's listen address / task handle.
    pub listen_addr: String,
}

/// Start the HTTP allowlist proxy for the given network policy.
///
/// Returns the address the agent should use as its `HTTP_PROXY`. A policy
/// with no allowed endpoints yields a proxy that rejects every request.
pub fn start(_policy: &NetworkPolicy) -> anyhow::Result<FirewallHandle> {
    // TODO(v1): bind a local TCP socket, spawn the proxy server, and return
    // its address. For now this is a stub.
    Ok(FirewallHandle {
        listen_addr: "127.0.0.1:0".to_string(),
    })
}

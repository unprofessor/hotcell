//! Network firewall via an HTTP allowlist proxy.
//!
//! Mirrors the `NetworkFirewall` guarantee: the agent can only reach
//! endpoints in the cell's provisioned network policy. All other network
//! connections are blocked. Enforcement is via a local HTTP proxy that only
//! handles `CONNECT` requests (tunneling); non-`CONNECT` requests are
//! refused. A cell with no allowed endpoints has no network access — the
//! proxy rejects every request.
//!
//! `firewall::start(policy)` binds a local TCP socket, spawns an async hyper
//! server (on its own tokio runtime) that checks each `CONNECT` request's
//! destination host/port against `NetworkPolicy::allowed_endpoints`, tunnels
//! allowed destinations, and refuses everything else with `403 Forbidden`.
//! The agent is expected to be launched with `HTTP_PROXY` / `HTTPS_PROXY`
//! pointing at the returned address; non-HTTP egress is blocked by the
//! sandbox's network namespace (a separate concern).
//!
//! The [`FirewallHandle`] owns the proxy's runtime, so the server lives as
//! long as the handle is held and is torn down when it is dropped.

use std::sync::Arc;
use std::time::Duration;

use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::io::{copy_bidirectional, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UnixListener, UnixStream};
use tokio::runtime::Runtime;

use crate::cellfile::NetworkPolicy;

/// An empty response body (no frames, exact zero length).
type EmptyBody = http_body_util::Empty<Bytes>;

/// A handle to the running firewall proxy for a session.
///
/// Holds the proxy's tokio runtime so the server stays alive for as long as
/// the handle is held. Dropping the handle shuts the proxy down.
/// `listen_addr` is the real address the server bound to (e.g.
/// `127.0.0.1:54321`), suitable for use as the agent's `HTTP_PROXY`.
pub struct FirewallHandle {
    pub listen_addr: String,
    runtime: Option<Runtime>,
}

impl FirewallHandle {
    /// The real address the proxy is listening on (e.g. `127.0.0.1:54321`),
    /// not the placeholder `127.0.0.1:0`.
    pub fn listen_addr(&self) -> &str {
        &self.listen_addr
    }

    /// Start a Unix-domain socket bridge that relays raw bytes between an
    /// in-namespace forwarder and this firewall's TCP proxy.
    ///
    /// The firewall proxy binds the *host's* `127.0.0.1:<port>`, which is
    /// unreachable from a `bwrap --unshare-net` namespace (the namespace has
    /// only its own private loopback). To let the agent reach the proxy with
    /// no network route exposed, a Unix-domain socket is bound at `uds_path`
    /// on the host (in a directory bind-mounted into both sides) and a tiny
    /// in-namespace forwarder relays the agent's loopback TCP to this socket.
    /// The host-side relay here then forwards those bytes to the firewall's
    /// TCP listener, where the real CONNECT allowlist enforcement happens.
    ///
    /// The bridge is a *dumb byte pipe* on both sides; all policy decisions
    /// remain in [`handle_connect`]. `uds_path` is removed first to clear any
    /// stale socket from a previous run. The listener lives for as long as the
    /// handle's runtime (dropped with the handle).
    pub fn start_uds_bridge(&self, uds_path: &std::path::Path) -> anyhow::Result<()> {
        // Clear a stale socket file so the bind does not fail with EADDRINUSE.
        let _ = std::fs::remove_file(uds_path);
        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("firewall runtime already shut down"))?;
        let listener = runtime.block_on(async { UnixListener::bind(uds_path) })?;
        let tcp_addr = self.listen_addr.clone();
        runtime.spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let tcp_addr = tcp_addr.clone();
                        tokio::spawn(async move {
                            if let Err(err) = relay_uds_to_tcp(stream, &tcp_addr).await {
                                tracing::debug!("firewall: uds bridge relay ended: {err}");
                            }
                        });
                    }
                    Err(err) => {
                        tracing::warn!("firewall: uds bridge accept error: {err}");
                        break;
                    }
                }
            }
        });
        Ok(())
    }
}

impl Drop for FirewallHandle {
    fn drop(&mut self) {
        if let Some(rt) = self.runtime.take() {
            // Graceful but bounded shutdown so dropping a handle never hangs
            // on a long-lived accept loop.
            rt.shutdown_timeout(Duration::from_millis(200));
        }
    }
}

/// Start the HTTP allowlist proxy for the given network policy.
///
/// Binds a local TCP socket (OS-assigned port on `127.0.0.1`), spawns the
/// async hyper server on a dedicated tokio runtime, and returns a handle
/// carrying the real listen address. The caller does not need to be running
/// a tokio runtime — `start` creates its own and keeps it alive via the
/// returned handle.
///
/// A policy with no allowed endpoints yields a proxy that rejects every
/// request with `403 Forbidden`.
pub fn start(policy: &NetworkPolicy) -> anyhow::Result<FirewallHandle> {
    // Build the proxy's own runtime. The rest of hotcell is sync (see
    // `cli.rs`), so the proxy owns its runtime and keeps it alive via the
    // returned handle.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()?;

    // Bind on the runtime so the I/O driver owns the listener. `block_on`
    // here is safe: `start` is only called from sync contexts.
    let listener = runtime.block_on(async { TcpListener::bind("127.0.0.1:0").await })?;
    let listen_addr = listener.local_addr()?;

    let policy = Arc::new(policy.clone());
    runtime.spawn(serve(listener, policy));

    Ok(FirewallHandle {
        listen_addr: listen_addr.to_string(),
        runtime: Some(runtime),
    })
}

/// Accept loop: serve each inbound connection on its own task.
async fn serve(listener: TcpListener, policy: Arc<NetworkPolicy>) {
    loop {
        match listener.accept().await {
            Ok((stream, _peer)) => {
                let policy = policy.clone();
                let io = TokioIo::new(stream);
                tokio::spawn(async move {
                    let svc = service_fn(move |req: Request<Incoming>| {
                        let policy = policy.clone();
                        async move { handle_connect(req, policy).await }
                    });
                    if let Err(err) = http1::Builder::new()
                        .keep_alive(true)
                        .preserve_header_case(true)
                        .serve_connection(io, svc)
                        .with_upgrades()
                        .await
                    {
                        tracing::debug!("firewall: connection closed: {err}");
                    }
                });
            }
            Err(err) => {
                tracing::warn!("firewall: accept error: {err}");
            }
        }
    }
}

/// Handle a single proxy request. Only `CONNECT` is supported; every other
/// method is refused with `405 Method Not Allowed`.
async fn handle_connect(
    req: Request<Incoming>,
    policy: Arc<NetworkPolicy>,
) -> Result<Response<EmptyBody>, std::convert::Infallible> {
    if req.method() != Method::CONNECT {
        return Ok(empty_response(StatusCode::METHOD_NOT_ALLOWED));
    }

    // For a CONNECT request the request-target is authority-form
    // (`host:port`); `Uri` exposes it via `host()` / `port_u16()`.
    let host = req.uri().host().map(|h| h.to_string());
    let port = req.uri().port_u16();

    let allowed = match host.as_deref() {
        Some(h) => endpoint_allowed(&policy, h, port),
        None => false,
    };

    if !allowed {
        return Ok(empty_response(StatusCode::FORBIDDEN));
    }

    // Allowed: acknowledge the tunnel and spawn the bidirectional copy once
    // the connection is upgraded. CONNECT defaults to port 443 when none is
    // given (matches `http`'s default for the scheme implied by CONNECT).
    let host = host.unwrap();
    let port = port.unwrap_or(443);

    tokio::spawn(async move {
        let upgraded = match hyper::upgrade::on(req).await {
            Ok(up) => up,
            Err(err) => {
                tracing::warn!("firewall: upgrade failed for {host}:{port}: {err}");
                return;
            }
        };
        let mut client = TokioIo::new(upgraded);

        let mut server = match TcpStream::connect((host.as_str(), port)).await {
            Ok(s) => s,
            Err(err) => {
                tracing::warn!("firewall: connect to {host}:{port} failed: {err}");
                // Best-effort: signal failure to the client by closing.
                let _ = client.shutdown().await;
                return;
            }
        };

        // Tunnel bytes both ways until either side closes.
        if let Err(err) = copy_bidirectional(&mut client, &mut server).await {
            tracing::debug!("firewall: tunnel to {host}:{port} ended: {err}");
        }
    });

    Ok(empty_response(StatusCode::OK))
}

/// Relay bytes bidirectionally between an accepted Unix-domain connection
/// (from the in-namespace forwarder) and the firewall's TCP proxy. The proxy
/// performs the actual CONNECT allowlist enforcement; this is a byte pipe.
///
/// Both streams are tokio-native, so they implement `AsyncRead`/`AsyncWrite`
/// directly — no `TokioIo` wrapping (that adapter is only for bridging
/// hyper's IO traits to tokio's, e.g. for an `Upgraded` body).
async fn relay_uds_to_tcp(mut uds: UnixStream, tcp_addr: &str) -> std::io::Result<()> {
    let mut tcp = TcpStream::connect(tcp_addr).await?;
    copy_bidirectional(&mut uds, &mut tcp).await?;
    Ok(())
}

/// Build a response with an empty body and the given status.
fn empty_response(status: StatusCode) -> Response<EmptyBody> {
    Response::builder()
        .status(status)
        .body(EmptyBody::new())
        .expect("static response")
}

/// Check whether a destination `(host, port)` is permitted by the policy.
///
/// Hosts match case-insensitively. If an endpoint declares a port, the
/// target port must match exactly; if the endpoint has no port, any port is
/// allowed for that host.
fn endpoint_allowed(policy: &NetworkPolicy, host: &str, port: Option<u16>) -> bool {
    policy.allowed_endpoints.iter().any(|ep| {
        ep.host.eq_ignore_ascii_case(host)
            && match ep.port {
                None => true,
                Some(p) => port == Some(p),
            }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cellfile::Endpoint;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;
    use std::time::Duration;

    /// Read an HTTP response head (up to the blank line) from a blocking
    /// stream, byte by byte. Times out after 2s so a broken proxy fails the
    /// test instead of hanging.
    fn read_response_head(stream: &mut TcpStream) -> String {
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut buf = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            let n = stream.read(&mut byte).unwrap();
            if n == 0 {
                break;
            }
            buf.push(byte[0]);
            if buf.ends_with(b"\r\n\r\n") {
                break;
            }
        }
        String::from_utf8_lossy(&buf).to_string()
    }

    /// Send a raw `CONNECT` request and return the response status line +
    /// headers.
    fn send_connect(proxy_addr: &str, target: &str) -> String {
        let mut stream = TcpStream::connect(proxy_addr).unwrap();
        let req = format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n\r\n");
        stream.write_all(req.as_bytes()).unwrap();
        read_response_head(&mut stream)
    }

    /// Run a trivial echo TCP server on an OS-assigned port; returns its
    /// address. The server accepts one connection, echoes bytes back, and
    /// exits. Useful for verifying the tunnel actually forwards data.
    fn spawn_echo_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        thread::spawn(move || {
            if let Ok((mut conn, _)) = listener.accept() {
                conn.set_read_timeout(Some(Duration::from_secs(2))).ok();
                let mut buf = [0u8; 128];
                loop {
                    match conn.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if conn.write_all(&buf[..n]).is_err() {
                                break;
                            }
                        }
                    }
                }
            }
        });
        addr
    }

    fn policy(endpoints: &[(&str, Option<u16>)]) -> NetworkPolicy {
        NetworkPolicy {
            allowed_endpoints: endpoints
                .iter()
                .map(|(h, p)| Endpoint {
                    host: (*h).to_string(),
                    port: *p,
                })
                .collect(),
        }
    }

    #[test]
    fn listen_addr_is_real_bound_address() {
        let handle = start(&policy(&[])).unwrap();
        let addr = handle.listen_addr();
        assert_ne!(addr, "127.0.0.1:0", "handle must expose the real address");
        assert!(
            addr.starts_with("127.0.0.1:") && !addr.ends_with(":0"),
            "real listen address, got {addr}"
        );
        // The port must be bindable / reachable.
        assert!(
            TcpStream::connect(addr).is_ok(),
            "proxy is listening on {addr}"
        );
    }

    #[test]
    fn allows_listed_host_port_and_tunnels() {
        let target = spawn_echo_server();
        // target is "127.0.0.1:<port>"; allow that exact host:port.
        let (th, tp) = target.rsplit_once(':').unwrap();
        let port: u16 = tp.parse().unwrap();
        let handle = start(&policy(&[(th, Some(port))])).unwrap();

        let mut client = TcpStream::connect(handle.listen_addr()).unwrap();
        let req = format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n\r\n");
        client.write_all(req.as_bytes()).unwrap();
        let head = read_response_head(&mut client);
        assert!(
            head.starts_with("HTTP/1.1 200"),
            "allowed CONNECT should be tunneled (200), got: {head}"
        );

        // After 200, the connection is a raw tunnel to the echo server.
        client.set_write_timeout(Some(Duration::from_secs(2))).ok();
        client.set_read_timeout(Some(Duration::from_secs(2))).ok();
        let payload = b"hotcell-tunnel-check";
        client.write_all(payload).unwrap();
        let mut got = [0u8; 128];
        let n = client.read(&mut got).unwrap();
        assert_eq!(
            &got[..n],
            payload,
            "tunnel must forward bytes to the target"
        );
    }

    #[test]
    fn allows_listed_host_with_any_port() {
        let target = spawn_echo_server();
        let (th, _tp) = target.rsplit_once(':').unwrap();
        // Allow the host with no port constraint: any port is permitted.
        let handle = start(&policy(&[(th, None)])).unwrap();

        let head = send_connect(handle.listen_addr(), &target);
        assert!(
            head.starts_with("HTTP/1.1 200"),
            "host-only endpoint should allow any port, got: {head}"
        );
    }

    #[test]
    fn host_match_is_case_insensitive() {
        let target = spawn_echo_server();
        let (th, tp) = target.rsplit_once(':').unwrap();
        let port: u16 = tp.parse().unwrap();
        // Policy stores the host in uppercase; CONNECT uses lowercase.
        let handle = start(&policy(&[(th.to_uppercase().as_str(), Some(port))])).unwrap();

        let head = send_connect(handle.listen_addr(), &target);
        assert!(
            head.starts_with("HTTP/1.1 200"),
            "host matching should be case-insensitive, got: {head}"
        );
    }

    #[test]
    fn denies_when_port_mismatches() {
        let target = spawn_echo_server();
        let (th, _tp) = target.rsplit_once(':').unwrap();
        // Allow the host only on a port we will not connect to.
        let handle = start(&policy(&[(th, Some(999))])).unwrap();

        let head = send_connect(handle.listen_addr(), &target);
        assert!(
            head.starts_with("HTTP/1.1 403"),
            "wrong port should be refused (403), got: {head}"
        );
    }

    #[test]
    fn denies_unlisted_host() {
        let _target = spawn_echo_server();
        // Allow something unrelated; the echo server's host:port is not listed.
        let handle = start(&policy(&[("allowed.example.com", Some(443))])).unwrap();

        let head = send_connect(handle.listen_addr(), "127.0.0.1:1");
        assert!(
            head.starts_with("HTTP/1.1 403"),
            "unlisted destination should be refused (403), got: {head}"
        );
    }

    #[test]
    fn empty_policy_rejects_everything() {
        let handle = start(&policy(&[])).unwrap();

        let head = send_connect(handle.listen_addr(), "127.0.0.1:443");
        assert!(
            head.starts_with("HTTP/1.1 403"),
            "empty policy must reject CONNECT (403), got: {head}"
        );

        let head2 = send_connect(handle.listen_addr(), "api.openai.com:443");
        assert!(
            head2.starts_with("HTTP/1.1 403"),
            "empty policy must reject every CONNECT (403), got: {head2}"
        );
    }

    #[test]
    fn non_connect_method_is_refused() {
        let handle = start(&policy(&[("api.openai.com", None)])).unwrap();

        let mut client = TcpStream::connect(handle.listen_addr()).unwrap();
        let req = "GET http://api.openai.com/ HTTP/1.1\r\nHost: api.openai.com\r\n\r\n";
        client.write_all(req.as_bytes()).unwrap();
        let head = read_response_head(&mut client);
        assert!(
            head.starts_with("HTTP/1.1 405"),
            "non-CONNECT should be refused with 405, got: {head}"
        );
    }

    #[test]
    fn endpoint_allowed_unit() {
        let p = policy(&[("api.openai.com", Some(443)), ("api.anthropic.com", None)]);
        assert!(endpoint_allowed(&p, "api.openai.com", Some(443)));
        assert!(
            endpoint_allowed(&p, "API.OpenAI.COM", Some(443)),
            "case-insensitive"
        );
        assert!(
            !endpoint_allowed(&p, "api.openai.com", Some(8443)),
            "port mismatch"
        );
        assert!(
            endpoint_allowed(&p, "api.anthropic.com", Some(8443)),
            "no port = any port"
        );
        assert!(
            !endpoint_allowed(&p, "evil.example.com", Some(443)),
            "unlisted host"
        );
        assert!(
            !endpoint_allowed(&p, "api.openai.com", None),
            "missing port on port-scoped endpoint"
        );
    }

    // ── Behavior tests (added 2026-08-06, TDD session) ─────────────────────

    /// CONNECT with no explicit port defaults to 443 (authority-form without
    /// a port), matching the documented default in `handle_connect`.
    ///
    /// Requires CAP_NET_BIND_SERVICE to observe end-to-end (binds a real
    /// 127.0.0.1:443 listener); skips gracefully where the sandbox denies
    /// privileged-port binds (rootless containers).
    #[test]
    fn connect_without_port_defaults_to_443() {
        let listener = match TcpListener::bind("127.0.0.1:443") {
            Ok(l) => l,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                eprintln!("skipping: cannot bind 127.0.0.1:443 in this environment ({e})");
                return;
            }
            Err(e) => panic!("bind 127.0.0.1:443 failed unexpectedly: {e}"),
        };
        thread::spawn(move || {
            if let Ok((mut conn, _)) = listener.accept() {
                conn.set_read_timeout(Some(Duration::from_secs(2))).ok();
                let mut buf = [0u8; 128];
                loop {
                    match conn.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if conn.write_all(&buf[..n]).is_err() {
                                break;
                            }
                        }
                    }
                }
            }
        });
        // Policy allows the host with NO port restriction.
        let handle = start(&policy(&[("127.0.0.1", None)])).unwrap();
        // CONNECT without a port: authority-form "127.0.0.1" only.
        let mut client = TcpStream::connect(handle.listen_addr()).unwrap();
        client
            .write_all(b"CONNECT 127.0.0.1 HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .unwrap();
        let head = read_response_head(&mut client);
        assert!(
            head.starts_with("HTTP/1.1 200"),
            "default-port CONNECT should tunnel to 443, got: {head}"
        );
        // The tunnel must reach the 443 listener: echo a payload through it.
        client.set_write_timeout(Some(Duration::from_secs(2))).ok();
        client.set_read_timeout(Some(Duration::from_secs(2))).ok();
        let payload = b"default-port-check";
        client.write_all(payload).unwrap();
        let mut got = [0u8; 128];
        let n = client.read(&mut got).unwrap();
        assert_eq!(
            &got[..n],
            payload,
            "tunnel must forward bytes to 127.0.0.1:443"
        );
    }

    /// Every non-CONNECT method is refused with 405, not just GET.
    #[test]
    fn all_non_connect_methods_refused() {
        let handle = start(&policy(&[("api.openai.com", None)])).unwrap();
        for method in ["POST", "PUT", "DELETE", "PATCH", "OPTIONS", "HEAD"] {
            let mut client = TcpStream::connect(handle.listen_addr()).unwrap();
            let req =
                format!("{method} http://api.openai.com/ HTTP/1.1\r\nHost: api.openai.com\r\n\r\n");
            client.write_all(req.as_bytes()).unwrap();
            let head = read_response_head(&mut client);
            assert!(
                head.starts_with("HTTP/1.1 405"),
                "{method} should be refused with 405, got: {head}"
            );
        }
    }

    /// CONNECT with no authority at all is refused (malformed), never tunneled.
    #[test]
    fn connect_without_authority_refused() {
        let handle = start(&policy(&[("api.openai.com", None)])).unwrap();
        let mut client = TcpStream::connect(handle.listen_addr()).unwrap();
        client
            .write_all(b"CONNECT  HTTP/1.1\r\nHost: \r\n\r\n")
            .unwrap();
        let head = read_response_head(&mut client);
        assert!(
            head.starts_with("HTTP/1.1 4"),
            "authority-less CONNECT must be refused with 4xx, got: {head}"
        );
    }

    /// A denied CONNECT returns 403 with an empty body (no frames).
    #[test]
    fn denied_connect_has_empty_body() {
        let handle = start(&policy(&[("allowed.example.com", Some(443))])).unwrap();
        let mut client = TcpStream::connect(handle.listen_addr()).unwrap();
        client
            .write_all(
                b"CONNECT denied.example.com:443 HTTP/1.1\r\nHost: denied.example.com:443\r\n\r\n",
            )
            .unwrap();
        let head = read_response_head(&mut client);
        assert!(head.starts_with("HTTP/1.1 403"), "got: {head}");
        // Nothing may follow the response head: empty body.
        client
            .set_read_timeout(Some(Duration::from_millis(300)))
            .ok();
        let mut buf = [0u8; 16];
        let n = client.read(&mut buf).unwrap_or(0);
        assert_eq!(
            n, 0,
            "denied CONNECT must carry an empty body, read {n} bytes"
        );
    }

    /// The tunnel carries data in BOTH directions: the upstream may push
    /// bytes without waiting for the client to send anything first.
    #[test]
    fn tunnel_is_bidirectional_streaming() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        thread::spawn(move || {
            if let Ok((mut conn, _)) = listener.accept() {
                // Push a greeting immediately, without reading first.
                let _ = conn.write_all(b"greeting-from-server");
                conn.set_read_timeout(Some(Duration::from_secs(2))).ok();
                let _ = conn.read(&mut [0u8; 128]);
            }
        });
        let (th, tp) = addr.rsplit_once(':').unwrap();
        let port: u16 = tp.parse().unwrap();
        let handle = start(&policy(&[(th, Some(port))])).unwrap();

        let mut client = TcpStream::connect(handle.listen_addr()).unwrap();
        let req = format!("CONNECT {addr} HTTP/1.1\r\nHost: {addr}\r\n\r\n");
        client.write_all(req.as_bytes()).unwrap();
        let head = read_response_head(&mut client);
        assert!(head.starts_with("HTTP/1.1 200"), "got: {head}");

        // Server pushes first: read its greeting before sending anything.
        client.set_read_timeout(Some(Duration::from_secs(2))).ok();
        let mut got = [0u8; 64];
        let n = client.read(&mut got).unwrap();
        assert_eq!(&got[..n], b"greeting-from-server");
    }

    /// Client half-close (EOF on write) is forwarded to the upstream, and
    /// the upstream's post-EOF response still flows back to the client.
    #[test]
    fn client_half_close_forwards_eof_and_response_flows() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        thread::spawn(move || {
            if let Ok((mut conn, _)) = listener.accept() {
                conn.set_read_timeout(Some(Duration::from_secs(2))).ok();
                // Read until EOF (client half-closed its write side).
                let mut buf = [0u8; 128];
                loop {
                    match conn.read(&mut buf) {
                        Ok(0) => break,
                        Ok(_) => {}
                        Err(_) => break,
                    }
                }
                // Send a final response only after seeing EOF.
                let _ = conn.write_all(b"after-eof-response");
            }
        });
        let (th, tp) = addr.rsplit_once(':').unwrap();
        let port: u16 = tp.parse().unwrap();
        let handle = start(&policy(&[(th, Some(port))])).unwrap();

        let mut client = TcpStream::connect(handle.listen_addr()).unwrap();
        let req = format!("CONNECT {addr} HTTP/1.1\r\nHost: {addr}\r\n\r\n");
        client.write_all(req.as_bytes()).unwrap();
        let head = read_response_head(&mut client);
        assert!(head.starts_with("HTTP/1.1 200"), "got: {head}");

        // Send bytes, then half-close our write side.
        client.write_all(b"request").unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();

        // The server's post-EOF response must reach us through the tunnel.
        client.set_read_timeout(Some(Duration::from_secs(2))).ok();
        let mut got = [0u8; 64];
        let mut read = Vec::new();
        loop {
            match client.read(&mut got) {
                Ok(0) => break,
                Ok(n) => read.extend_from_slice(&got[..n]),
                Err(_) => break,
            }
        }
        assert!(
            read.windows(b"after-eof-response".len())
                .any(|w| w == b"after-eof-response"),
            "post-EOF server response must flow through the tunnel, got: {read:?}"
        );
    }

    /// Documents CURRENT behavior: the 200 for CONNECT is sent before the
    /// upstream dial completes, so a refused upstream yields 200-then-EOF.
    /// (Plan Phase 6 wants a deterministic error response instead.)
    #[test]
    fn refused_upstream_yields_200_then_eof() {
        let probe = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = probe.local_addr().unwrap().to_string();
        drop(probe); // nothing is listening anymore
        let (th, tp) = addr.rsplit_once(':').unwrap();
        let port: u16 = tp.parse().unwrap();
        let handle = start(&policy(&[(th, Some(port))])).unwrap();

        let head = send_connect(handle.listen_addr(), &addr);
        assert!(
            head.starts_with("HTTP/1.1 200"),
            "dial is async, so the client sees 200 first, got: {head}"
        );
    }

    // ── RED: planned Phase 1/2 behavior (fails today; TDD targets) ────────

    /// Phase 1 (globs): `*.example.com` must match the apex and any
    /// subdomain. Fails today: no glob support → 403.
    #[test]
    fn glob_host_matches_subdomain_and_apex() {
        let handle = start(&policy(&[("*.example.com", None)])).unwrap();
        for host in ["api.example.com", "example.com", "a.b.example.com"] {
            let head = send_connect(handle.listen_addr(), &format!("{host}:443"));
            assert!(
                head.starts_with("HTTP/1.1 200"),
                "glob *.example.com should allow {host}, got: {head}"
            );
        }
    }

    /// Phase 1 (globs): globs must not cross domain boundaries.
    /// Passes today trivially; guards the future glob matcher.
    #[test]
    fn glob_host_does_not_cross_domains() {
        let handle = start(&policy(&[("*.example.com", None)])).unwrap();
        for host in ["evil-example.com", "notexample.com", "example.com.evil.net"] {
            let head = send_connect(handle.listen_addr(), &format!("{host}:443"));
            assert!(
                head.starts_with("HTTP/1.1 403"),
                "glob *.example.com must NOT allow {host}, got: {head}"
            );
        }
    }

    /// Phase 2 (upstream IP deny / SSRF): an allowlisted hostname that
    /// resolves to loopback must be refused. `localhost` → 127.0.0.1 is the
    /// canonical DNS-rebinding stand-in. Fails today: the proxy tunnels
    /// straight to the resolved address (200).
    #[test]
    fn allowlisted_host_resolving_to_loopback_is_denied() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        // Keep a live listener so any pass is a real tunnel, not a refused
        // connection — the denial must come from the deny list.
        thread::spawn(move || {
            if let Ok((mut conn, _)) = listener.accept() {
                let _ = conn.write_all(b"should-never-arrive");
                let _ = conn.read(&mut [0u8; 16]);
            }
        });
        let handle = start(&policy(&[("localhost", None)])).unwrap();

        let target = addr.replace("127.0.0.1", "localhost");
        let head = send_connect(handle.listen_addr(), &target);
        assert!(
            head.starts_with("HTTP/1.1 403"),
            "localhost (loopback) must be denied by the upstream IP deny list, got: {head}"
        );
    }

    /// Phase 2: the cloud metadata service (IMDS 169.254.169.254) is denied
    /// even when explicitly allowlisted.
    #[test]
    fn imds_denied_even_when_allowlisted() {
        let handle = start(&policy(&[("169.254.169.254", Some(80))])).unwrap();
        let head = send_connect(handle.listen_addr(), "169.254.169.254:80");
        assert!(
            head.starts_with("HTTP/1.1 403"),
            "IMDS must be denied by the upstream IP deny list, got: {head}"
        );
    }

    /// Phase 6 (host normalization): a trailing dot is DNS-equivalent and
    /// must match the same policy entry. Fails today: exact match on the
    /// dotted host → 403.
    #[test]
    fn trailing_dot_host_matches_policy() {
        let handle = start(&policy(&[("example.com", Some(443))])).unwrap();
        let head = send_connect(handle.listen_addr(), "example.com.:443");
        assert!(
            head.starts_with("HTTP/1.1 200"),
            "trailing-dot host should normalize to example.com, got: {head}"
        );
    }

    /// IPv6-literal CONNECT targets surface as the BRACKETED form (`[::1]`),
    /// so bracketed policy hosts match. RED as of 2026-08-06: the tunnel
    /// then fails because `TcpStream::connect(("[::1]", port))` cannot
    /// resolve the bracketed host (getaddrinfo rejects it) — the client
    /// sees 200 followed by EOF. Fix (Phase 2/6): strip brackets before
    /// dialing, or normalize the policy host to the unbracketed form.
    #[test]
    fn bracketed_ipv6_connect_matches_bracketed_policy() {
        let listener = std::net::TcpListener::bind("[::1]:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string(); // "[::1]:PORT"
        thread::spawn(move || {
            if let Ok((mut conn, _)) = listener.accept() {
                let _ = conn.write_all(b"v6-ok");
                let _ = conn.read(&mut [0u8; 16]);
            }
        });
        // Policy host must be the bracketed form to match CONNECT targets.
        let handle = start(&policy(&[("[::1]", None)])).unwrap();
        let mut client = TcpStream::connect(handle.listen_addr()).unwrap();
        let req = format!("CONNECT {addr} HTTP/1.1\r\nHost: {addr}\r\n\r\n");
        client.write_all(req.as_bytes()).unwrap();
        let head = read_response_head(&mut client);
        assert!(
            head.starts_with("HTTP/1.1 200"),
            "bracketed IPv6 policy should match bracketed CONNECT target, got: {head}"
        );
        // And the tunnel must reach the ::1 listener.
        client.set_read_timeout(Some(Duration::from_secs(2))).ok();
        let mut got = [0u8; 16];
        let n = client.read(&mut got).unwrap();
        assert_eq!(&got[..n], b"v6-ok", "tunnel must reach the ::1 listener");
    }
}

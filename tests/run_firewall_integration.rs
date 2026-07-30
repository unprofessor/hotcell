//! End-to-end integration test for the network firewall's allowlist path.
//!
//! Complements `run_loopback_network.rs` (which covers the offline path, a
//! single allowed CONNECT, and the kernel-enforced non-loopback block). This
//! file exercises the **full allowlist decision path through the bridge at
//! the `hotcell run` level**: in one cell run, the agent reaches an
//! *allowlisted* loopback endpoint through the proxy AND is *blocked by the
//! proxy* (HTTP `403 Forbidden`) from reaching a second loopback endpoint
//! that is NOT in the policy.
//!
//! What this proves end-to-end (agent program -> in-namespace `hotcell fwd`
//! forwarder -> UDS bridge -> host-side firewall proxy -> allowlist check):
//! - The allowlist *permits* a listed `host:port`: CONNECT returns `200` and
//!   bytes tunnel through to a real echo server.
//! - The allowlist *denies* a same-host, different-port endpoint: the proxy
//!   itself returns `403` (not a kernel block, not a connection reset — the
//!   policy decision is made inside the firewall and surfaced to the agent).
//!   This is the proxy-level enforcement, distinct from the kernel-level
//!   non-loopback block tested in `run_loopback_network.rs`.
//!
//! Two echo servers are stood up on the host loopback on OS-assigned ports;
//! only one port is added to `net.allow`. The agent reads `HTTP_PROXY`
//! (set by the in-namespace forwarder to its own loopback address) and issues
//! raw `CONNECT` requests through it for both ports, asserting `200`+echo for
//! the allowed one and `403` for the denied one.
//!
//! Relies on `/usr/bin/python3` being present on the host (read-only bound
//! into the sandbox by the agent profile); skips with a message if absent.

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::thread;
use std::time::Duration;

use tempfile::TempDir;

fn hotcell_bin() -> String {
    env!("CARGO_BIN_EXE_hotcell").to_string()
}

/// The in-sandbox path to python3, available because the agent profile
/// read-only binds `/usr` from the host.
const PY: &str = "/usr/bin/python3";

/// If python3 is not present on the host, skip the test (return `true`).
fn skip_if_no_python3() -> bool {
    let out = Command::new("/usr/bin/python3").arg("--version").output();
    match out {
        Ok(o) => o.status.success(),
        Err(_) => {
            eprintln!("skip: /usr/bin/python3 not available on host");
            false
        }
    }
}

/// Write a Cellfile + a trivial provisioner that creates the workdir, in the
/// given temp dir. `net_allow_lines` is appended verbatim (one or more
/// `net.allow = ...` lines).
fn write_cellfile(dir: &TempDir, net_allow_lines: &str) {
    let mut content = String::from(
        "provision.type = shell\n\
         provision.script = ./provision.sh\n\
         workdir = /work\n",
    );
    if !net_allow_lines.is_empty() {
        content.push_str(net_allow_lines);
        content.push('\n');
    }
    fs::write(dir.path().join("Cellfile"), content).unwrap();
    fs::write(
        dir.path().join("provision.sh"),
        "#!/usr/bin/env bash\nset -euo pipefail\nmkdir -p \"$HOTCELL_WORKDIR_HOST\"\n",
    )
    .unwrap();
    fs::set_permissions(
        dir.path().join("provision.sh"),
        PermissionsExt::from_mode(0o755),
    )
    .unwrap();
}

/// Spawn a one-shot echo TCP server on the host loopback; returns its
/// `127.0.0.1:<port>` address. Accepts one connection, echoes `echo:` + each
/// received chunk, then exits.
fn spawn_echo_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    thread::spawn(move || {
        if let Ok((mut conn, _)) = listener.accept() {
            conn.set_read_timeout(Some(Duration::from_secs(5))).ok();
            let mut buf = [0u8; 128];
            loop {
                match conn.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if conn.write_all(b"echo:").is_err() || conn.write_all(&buf[..n]).is_err() {
                            break;
                        }
                    }
                }
            }
        }
    });
    addr
}

/// Run hotcell with a python3 `-c` script as the agent program; return the
/// captured stdout/stderr/exit.
fn run_agent(cell_dir: &TempDir, script: &str) -> (std::process::Output, String, String) {
    let output = Command::new(hotcell_bin())
        .current_dir(cell_dir.path())
        .args(["run", "--", PY, "-c", script])
        .output()
        .expect("run hotcell");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output, stdout, stderr)
}

/// End-to-end allowlist path: in a single cell run, an allowlisted loopback
/// endpoint is reachable through the proxy (CONNECT `200` + bytes tunnel to
/// an echo server), while a second, non-allowlisted loopback endpoint on the
/// same host but a different port is blocked by the proxy with `403`.
///
/// This exercises the full bridge (agent -> `hotcell fwd` forwarder -> UDS
/// bridge -> host firewall proxy -> allowlist decision) and proves the proxy
/// itself makes the deny decision (as opposed to the kernel non-loopback
/// block covered in `run_loopback_network.rs`).
#[test]
fn allowlisted_reachable_and_non_allowlisted_blocked() {
    if !skip_if_no_python3() {
        return;
    }

    // Two echo servers on distinct OS-assigned loopback ports. Only the
    // allowed port is added to the policy; the denied port is left out.
    let allowed_addr = spawn_echo_server();
    let denied_addr = spawn_echo_server();
    let allowed_port = allowed_addr.rsplit_once(':').unwrap().1;
    let denied_port = denied_addr.rsplit_once(':').unwrap().1;
    // Sanity: the two servers must be on different ports, otherwise the test
    // would be meaningless.
    assert_ne!(
        allowed_port, denied_port,
        "test requires two distinct echo ports"
    );

    let dir = TempDir::new().expect("create temp dir");
    // Allowlist only the allowed port on 127.0.0.1. The firewall matches host
    // strings, so use "127.0.0.1" (the host the CONNECT targets).
    write_cellfile(&dir, &format!("net.allow = 127.0.0.1:{allowed_port}"));

    // The agent reads HTTP_PROXY (set by the in-namespace forwarder to its own
    // loopback address), issues CONNECT to each echo endpoint through it, and
    // asserts: allowed -> 200 + echo round-trip; denied -> 403.
    let script = format!(
        r#"
import os, socket, sys

def connect_via_proxy(target):
    proxy = os.environ["HTTP_PROXY"]
    # proxy is "http://127.0.0.1:<port>"; pull the trailing port.
    fwd_port = int(proxy.rsplit(":", 1)[1].rstrip("/"))
    s = socket.create_connection(("127.0.0.1", fwd_port), timeout=5)
    s.sendall(f"CONNECT {{target}} HTTP/1.1\r\nHost: {{target}}\r\n\r\n".encode())
    hdr = b""
    while not hdr.endswith(b"\r\n\r\n"):
        b = s.recv(1)
        if not b:
            break
        hdr += b
    status = hdr.splitlines()[0].decode() if hdr else "(empty)"
    return s, status

ok = True

# (1) Allowed endpoint: CONNECT must return 200 and bytes must tunnel to the
# echo server and back.
allowed = "127.0.0.1:{allowed_port}"
s, status = connect_via_proxy(allowed)
print("allowed status:", status)
if not status.startswith("HTTP/1.1 200"):
    print("FAIL: allowed CONNECT not 200"); ok = False
else:
    s.sendall(b"hotcell-e2e")
    # The echo server writes "echo:" and the payload in two writes; TCP may
    # deliver them separately, so accumulate until we have the full echo or
    # the read times out.
    s.settimeout(5)
    got = b""
    while b"echo:hotcell-e2e" not in got:
        try:
            chunk = s.recv(128)
        except socket.timeout:
            break
        if not chunk:
            break
        got += chunk
    s.close()
    if got != b"echo:hotcell-e2e":
        print(f"FAIL: echo round-trip mismatch: {{got!r}}"); ok = False
    else:
        print("ok: allowed endpoint reachable via proxy")

# (2) Non-allowlisted endpoint (same host, different port): the proxy itself
# must refuse the CONNECT with 403 — proving the deny is a policy decision
# made inside the firewall, not a kernel block or connection reset.
denied = "127.0.0.1:{denied_port}"
s, status = connect_via_proxy(denied)
print("denied status:", status)
s.close()
if not status.startswith("HTTP/1.1 403"):
    print("FAIL: denied CONNECT not 403"); ok = False
else:
    print("ok: non-allowlisted endpoint blocked by proxy (403)")

print("RESULT", "ok" if ok else "fail")
sys.exit(0 if ok else 1)
"#
    );

    let (output, stdout, stderr) = run_agent(&dir, &script);
    assert!(
        output.status.success(),
        "end-to-end allowlist test failed: exit {:?}\n\
         stdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stdout.contains("RESULT ok"),
        "stdout: {stdout}\nstderr: {stderr}"
    );
}

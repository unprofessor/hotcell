//! Integration tests for the loopback-only network bridge (`loopback-only-net`).
//!
//! Covers the network-isolation guarantees for the agent profile:
//! - An **offline** cell (empty network policy) has no network at all: no
//!   `HTTP_PROXY` is set and non-loopback egress is `ENETUNREACH`.
//! - A **networked** cell's agent can reach an allowed endpoint *only* through
//!   the loopback proxy (an HTTP `CONNECT` to an allowed `127.0.0.1:<port>`
//!   is tunneled with `200 OK` and bytes flow through).
//! - A **networked** cell's agent still *cannot* reach a non-loopback
//!   endpoint directly — `--unshare-net` gives the namespace only a private
//!   loopback, so any non-loopback TCP attempt is kernel-enforced
//!   `ENETUNREACH` (errno 101) regardless of the policy.
//!
//! The bridge under test: the agent runs in a `bwrap --unshare-net` namespace
//! (loopback only); an in-namespace forwarder (`hotcell fwd`, the hotcell
//! binary re-invoked and bind-mounted read-only) listens on the namespace's
//! `127.0.0.1` and relays bytes to a Unix-domain socket bridged to the
//! host-side firewall proxy. The agent's `HTTP_PROXY` points at that
//! forwarder, so from the agent's view the proxy is on its own loopback and
//! nothing else is reachable.
//!
//! These tests rely on `/usr/bin/python3` being present on the host (it is
//! bind-mounted read-only into the sandbox via the agent profile's base
//! binds); they skip with a clear message if it is absent.

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
/// The base-system binds expose host `/usr` into the sandbox, so the host
/// having python3 is the test's only non-bwrap dependency.
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
/// given temp dir. `net_allow` is the `net.allow` line content (or empty for
/// an offline cell).
fn write_cellfile(dir: &TempDir, net_allow: &str) {
    let mut content = String::from(
        "provision.type = shell\n\
         provision.script = ./provision.sh\n\
         workdir = /work\n",
    );
    if !net_allow.is_empty() {
        content.push_str(net_allow);
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
/// received chunk, then exits. Used as the allowed CONNECT target.
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
/// captured stdout/stderr/exit. The script is passed as a single arg so no
/// shell quoting is involved.
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

/// (a) An offline cell (empty policy) has no network at all: no proxy env is
/// set and a non-loopback TCP attempt is `ENETUNREACH` (errno 101).
#[test]
fn offline_cell_has_no_network() {
    if !skip_if_no_python3() {
        return;
    }
    let dir = TempDir::new().expect("create temp dir");
    write_cellfile(&dir, "");

    // Assert: HTTP_PROXY/HTTPS_PROXY are unset (no forwarder for offline),
    // and a direct non-loopback TCP connect is ENETUNREACH (101). The
    // namespace's only interface is loopback, so this is kernel-enforced.
    let script = r#"
import os, socket, sys
ok = True
if "HTTP_PROXY" in os.environ or "HTTPS_PROXY" in os.environ:
    print("FAIL: proxy env set on offline cell"); ok = False
try:
    s = socket.create_connection(("1.1.1.1", 80), timeout=3)
    print("FAIL: non-loopback connected on offline cell"); ok = False
    s.close()
except OSError as e:
    if e.errno != 101:
        print(f"FAIL: expected ENETUNREACH(101), got errno={e.errno}: {e}"); ok = False
    else:
        print("ok: offline non-loopback ENETUNREACH")
print("RESULT", "ok" if ok else "fail")
sys.exit(0 if ok else 1)
"#;

    let (output, stdout, stderr) = run_agent(&dir, script);
    assert!(
        output.status.success(),
        "offline cell should have no network: exit {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(stdout.contains("RESULT ok"), "stdout: {stdout}");
}

/// (b) A networked cell's agent can reach an allowed loopback endpoint through
/// the proxy: an HTTP CONNECT to `127.0.0.1:<echoport>` (allowed by policy)
/// returns `200 OK` and bytes tunnel through to the echo server.
#[test]
fn networked_agent_reaches_loopback_proxy() {
    if !skip_if_no_python3() {
        return;
    }
    let echo_addr = spawn_echo_server();
    let echo_port = echo_addr.rsplit_once(':').unwrap().1;
    let dir = TempDir::new().expect("create temp dir");
    write_cellfile(&dir, &format!("net.allow = 127.0.0.1:{echo_port}"));

    // The agent reads HTTP_PROXY (set by the in-namespace forwarder to its own
    // loopback address), issues CONNECT to the allowed echo endpoint, expects
    // 200, then sends a payload and expects it echoed back through the tunnel.
    let script = format!(
        r#"
import os, socket, sys
proxy = os.environ["HTTP_PROXY"]
fwd_port = int(proxy.split(":")[2])
s = socket.create_connection(("127.0.0.1", fwd_port), timeout=5)
target = "127.0.0.1:{echo_port}"
s.sendall(f"CONNECT {{target}} HTTP/1.1\r\nHost: {{target}}\r\n\r\n".encode())
hdr = b""
while not hdr.endswith(b"\r\n\r\n"):
    b = s.recv(1)
    if not b: break
    hdr += b
status = hdr.splitlines()[0].decode() if hdr else "(empty)"
s.sendall(b"hotcell-bridge")
got = s.recv(128)
s.close()
ok = status.startswith("HTTP/1.1 200") and got == b"echo:hotcell-bridge"
print("status:", status)
print("echo:", got)
print("RESULT", "ok" if ok else "fail")
sys.exit(0 if ok else 1)
"#
    );

    let (output, stdout, stderr) = run_agent(&dir, &script);
    assert!(
        output.status.success(),
        "networked agent should reach allowed endpoint via proxy: exit {:?}\n\
         stdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stdout.contains("RESULT ok"),
        "stdout: {stdout}\nstderr: {stderr}"
    );
}

/// (c) A networked cell's agent CANNOT reach a non-loopback endpoint directly.
/// `--unshare-net` gives the namespace only a private loopback interface, so
/// any non-loopback TCP attempt is kernel-enforced `ENETUNREACH` (errno 101)
/// — independent of the policy and with no route to tamper with. This is the
/// airtight half of acceptance #2.
#[test]
fn networked_agent_cannot_reach_non_loopback() {
    if !skip_if_no_python3() {
        return;
    }
    // Give a non-empty policy so the forwarder + bridge are actually started
    // (exercising the full networked path) while asserting non-loopback is
    // still blocked.
    let dir = TempDir::new().expect("create temp dir");
    write_cellfile(&dir, "net.allow = api.openai.com:443");

    let script = r#"
import os, socket, sys
ok = True
# Direct non-loopback TCP must fail with ENETUNREACH (101), not connect.
try:
    s = socket.create_connection(("1.1.1.1", 80), timeout=3)
    print("FAIL: non-loopback connected"); ok = False
    s.close()
except OSError as e:
    if e.errno != 101:
        print(f"FAIL: expected ENETUNREACH(101), got errno={e.errno}: {e}"); ok = False
    else:
        print("ok: non-loopback ENETUNREACH")
# Sanity: a non-loopback by hostname is also blocked (DNS would need network,
# but resolution failure or connect failure both prove no egress).
try:
    s = socket.create_connection(("example.com", 80), timeout=3)
    print("FAIL: example.com connected"); ok = False
    s.close()
except OSError:
    print("ok: example.com unreachable")
print("RESULT", "ok" if ok else "fail")
sys.exit(0 if ok else 1)
"#;

    let (output, stdout, stderr) = run_agent(&dir, script);
    assert!(
        output.status.success(),
        "networked agent must NOT reach non-loopback: exit {:?}\n\
         stdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stdout.contains("RESULT ok"),
        "stdout: {stdout}\nstderr: {stderr}"
    );
}

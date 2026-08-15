# hotcell — an AI coding agent sandbox

**hotcell** isolates AI coding agents (e.g. [Pi](https://pi.dev), Claude Code,
Copilot) in lightweight, disposable sandboxes. Each agent runs inside a
[bubblewrap](https://github.com/containers/bubblewrap) container with a
private filesystem, a clean environment, and an HTTP-allowlist-only network
egress path — so the agent can talk to its model provider but nothing else.

## Status

| Capability | Status |
| ----------- | -------- |
| Filesystem isolation (bubblewrap) | ✅ Stable |
| Shell / no-op provisioner | ✅ Stable |
| Cell persistence & state directory | ✅ Stable |
| HTTP CONNECT allowlist firewall | ✅ Stable |
| Offline cells (no network) | ✅ Stable |
| Cellfile override flags (`--file`, `--cell`, `--name`) | ✅ Stable |
| Policy v2: host globs, CIDRs, warn mode | 🔜 Post-v1 |
| Rego policy evaluation | 🔜 Gated |
| TLS MITM / L7 method-path-headers | 🔜 Gated |

## Install

### Prerequisites

- **bubblewrap** (`bwrap`) — [install](https://github.com/containers/bubblewrap)
  via your package manager (`apt install bubblewrap`, `brew install bubblewrap`,
  etc.). Required for sandbox isolation.
- **Rust** 1.81+ — needed only to build hotcell itself.

### Build from source

```bash
git clone https://github.com/unprofessor/hotcell.git
cd hotcell
cargo install --path .
```

This installs `hotcell` to `~/.cargo/bin/hotcell`.

## Quick start

Create a `Cellfile` in any directory:

```ini
# Cellfile
provision.type = none
workdir = /work

# Run this cell:
#   hotcell run -- echo hello from hotcell
```

Then run a program inside it:

```bash
hotcell run -- echo hello from hotcell
```

The output shows provisioning (skipped for `type = none`) and the program's
output. Subsequent runs reuse the provisioned state.

## Cellfile format

A Cellfile is a plain key-value file stored as `Cellfile` in the project
directory. Keys use dot-separated namespacing.

| Key | Description | Example |
| ----- | ------------- | --------- |
| `provision.type` | Provisioner kind: `none` (no-op) or `shell` | `provision.type = shell` |
| `provision.script` | Script path (relative to Cellfile dir) for `shell` provisioner | `provision.script = ./provision.sh` |
| `provision.host_path` | Host files/dirs the provisioner may read (repeatable) | `provision.host_path = ~/.nvm/versions/node` |
| `workdir` | Working directory inside the cell | `workdir = /work` |
| `env.*` | Environment variables passed to the agent | `env.PATH = /opt/pi/bin:/usr/bin:/bin` |
| `net.allow` | Allowed egress endpoints (`host` or `host:port`, repeatable) | `net.allow = generativelanguage.googleapis.com:443` |
| `seed` | Source → target file mapping (provisioner-interpreted) | `seed = ./config.json => /app/config.json` |
| `package` | Package to install (provisioner-interpreted) | `package = nodejs` |

A completely empty Cellfile is valid — it gives you a no-op provisioner,
offline, with a working directory at `/work`.

## Commands

### `hotcell run`

Provision a cell (if needed) and run a program inside it.

```bash
hotcell run [--name <name>] [--file <path>] [--cell <dir>] [-y] [--dry-run] -- <program> [args...]
```

- `--name` — cell name (default: `default`). Multiple cells can share one
  Cellfile.
- `--file` — path to an explicit Cellfile (default: `$PWD/Cellfile`).
- `--cell` — override where cell state lives (independent of the Cellfile
  location).
- `-y, --no-confirm` — skip the confirmation prompt (the plan is still
  displayed).
- `--dry-run` — print the execution plan and exit without running anything;
  no state is created or modified.
- `--` — everything after this is the program and its arguments.

Before any sandbox starts, hotcell prints its **execution plan**: every
`bwrap` command it will run — one flag (with its values) per line,
backslash continuations, POSIX quoting, and all host paths shown absolute —
so the block can be copy-pasted verbatim into a terminal. The commands
shown are the commands run: execution walks the same plan that was
displayed. A `[y/N]` confirmation prompt then gates the run unless
`--dry-run` (prints and exits) or `-y` (runs immediately) is given.

**Exit code:** mirrors the program's exit code. If provisioning fails, exits
with 1.

### `hotcell status`

List cells and their provisioning status:

```bash
hotcell status [--file <path>] [--cell <dir>]
```

Shows each cell name and one of `unprovisioned`, `provisioned`,
`provisioning_failed`.

### `hotcell destroy`

Tear down a cell's provisioned filesystem, returning it to unprovisioned:

```bash
hotcell destroy [--name <name>] [--file <path>] [--cell <dir>] [--force]
```

Prompts for confirmation unless `--force` is passed. The Cellfile and state
directory structure remain — only the sandbox rootfs is removed.

### Global flags

- `-f, --file` — applies to every subcommand; state lives alongside this file.
- `-c, --cell` — overrides the state root directory.
- `-V, --version` — print version.

## Network firewall

hotcell's network isolation is kernel-enforced (bubblewrap's `--unshare-net`)
plus a host-side HTTP CONNECT proxy:

- **No `net.allow` entries** → the agent is fully offline. No proxy starts.
- **One or more `net.allow` entries** → an HTTP CONNECT proxy starts on a
  loopback address. The agent connects through it, and only the listed
  hostnames are reachable.

The proxy is **hostname-allowlist only** for v1 — port, protocol, URL-path,
and L7 rules are post-v1. Non-HTTP protocols (raw TCP/UDP) are blocked
because the cell cannot reach non-loopback addresses through the
unshared-net namespace.

For a self-hosting example that runs Pi inside a hotcell, see
[examples/pi-bootstrap](examples/pi-bootstrap/).

## How it works

1. hotcell reads a `Cellfile` from the current (or specified) directory.
2. It **provisions** the cell: runs the declared provisioner (shell script or
   no-op) to populate the sandbox rootfs.
3. It starts the **firewall proxy** (if network is allowed).
4. It **launches the agent** inside `bwrap --unshare-net` with:
   - The provisioned rootfs mounted as `/`
   - `HOME`, `PATH`, and other env vars set from the Cellfile
   - `HTTPS_PROXY` pointing at the host-side firewall
   - A UDS bridge for loopback-like connectivity
5. When the agent exits, hotcell tears down the firewall and returns the
   agent's exit code.

## Specification

The formal Allium specification lives in [`hotcell.allium`](hotcell.allium).
It defines the entity model, state machine, and guarantees — the code
implements it.

## License

MIT

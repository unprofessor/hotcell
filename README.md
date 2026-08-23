# hotcell — an AI coding agent sandbox

**hotcell** runs AI coding agents (e.g. [Pi](https://pi.dev), Claude Code,
Copilot) inside [bubblewrap](https://github.com/containers/bubblewrap)
sandboxes. Each agent gets a private filesystem, an environment containing
only what its Cellfile declares, and no network route off the machine except
an HTTP CONNECT proxy that allows the hostnames you list.

## Install

### Prerequisites

- **bubblewrap** (`bwrap`) — [install](https://github.com/containers/bubblewrap)
  via your package manager (`apt install bubblewrap`, `brew install bubblewrap`,
  etc.). Required for sandbox isolation.
- **Rust** 1.81+ — needed only to build hotcell itself.

### From git

```bash
cargo install --git https://github.com/unprofessor/hotcell
```

This puts `hotcell` in `~/.cargo/bin`.

### From a checkout

To hack on hotcell, clone it and install from there:

```bash
git clone https://github.com/unprofessor/hotcell.git
cd hotcell
cargo install --path .
```

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
| `seed` | Source → target file mapping. **Parsed but not implemented** — no provisioner reads it | `seed = ./config.json => /app/config.json` |
| `package` | Package to install. **Parsed but not implemented** — no provisioner reads it | `package = nodejs` |

A completely empty Cellfile is valid — it gives you a no-op provisioner,
offline, with a working directory at `/work`.

## Commands

```bash
hotcell run -- pi                 # provision if needed, then run pi in the cell
hotcell run --dry-run -- pi       # print the plan, run nothing
hotcell run --name scratch -- sh  # a second cell from the same Cellfile
hotcell status                    # list cells and their provisioning state
hotcell destroy                   # tear down a cell's rootfs
```

Everything after `--` is the program and its arguments; without it, hotcell
parses the program's flags as its own. Run `hotcell --help` or
`hotcell <command> --help` for the full flag list.

Before any sandbox starts, hotcell prints its **execution plan**: every
`bwrap` command it will run, one flag per line, POSIX-quoted with all host
paths absolute, so the block can be copy-pasted verbatim into a terminal.
Execution runs that same plan, argument for argument. A `[y/N]` prompt then
gates the run — `--dry-run` prints the plan and exits, `-y` skips the
prompt.

`hotcell run` exits with the program's exit code, or 1 if provisioning
failed.

## Network firewall

hotcell's network isolation is kernel-enforced (bubblewrap's `--unshare-net`)
plus a host-side HTTP CONNECT proxy:

- **No `net.allow` entries** → the agent is fully offline. No proxy starts.
- **One or more `net.allow` entries** → an HTTP CONNECT proxy starts on a
  loopback address. The agent connects through it, and only the listed
  hostnames are reachable.

### Known limits

The proxy matches the hostname in the agent's `CONNECT` request (and the
port, when one is declared). Everything else is post-v1:

- **Resolved addresses are not checked.** The proxy dials from the host's
  network namespace, so an allowlisted name that resolves to a loopback,
  link-local, or LAN address reaches it. Only allowlist hosts you would be
  willing to let the agent reach *through* your machine.
- **No URL-path, method, or header rules**, and no globs or CIDRs — an
  allowlisted host is allowed in full. TLS is tunneled, not inspected.
- **A host declared without a port allows every port** on that host.

Non-HTTP protocols (raw TCP/UDP) are blocked because the cell cannot reach
non-loopback addresses through the unshared-net namespace.

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
   - A forwarder on the namespace's own loopback, which `HTTP_PROXY` and
     `HTTPS_PROXY` point at, relaying to the host firewall over a Unix
     socket — the agent itself never has a route off loopback
5. When the agent exits, hotcell tears down the firewall and returns the
   agent's exit code.

## Specification

The Allium specification in [`hotcell.allium`](hotcell.allium) defines the
entity model, state machine, and guarantees. It is the reference the code is
written against, and it currently runs ahead of the code in places — sessions
are not modelled at runtime, and `provisioner_override` is unimplemented.

## License

MIT

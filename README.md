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
- **Rust** 1.95+ — needed only to build hotcell itself.

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

```kdl
// Cellfile — a no-op cell with a working directory at /work
workdir "/work"
```

Then run a program inside it:

```bash
hotcell run -- echo hello from hotcell
```

The output shows provisioning (a no-op here) and the program's output.
Subsequent runs reuse the provisioned state.

## Cellfile format

A Cellfile is a [KDL v2](https://kdl.dev) document named `Cellfile` in the
project directory. Comments start with `//`. A completely empty Cellfile is
valid — it gives you a no-op provisioner, offline, with the default working
directory (`/work`).

A worked example (a shell-provisioned cell that reaches one host):

```kdl
workdir "/work"                          // working directory inside the cell

provision "shell" script="./provision.sh" {
    host-path "~/.nvm/versions/node"     // repeatable: a host path the
    host-path "~/.gitconfig"             // provisioner may read, read-only
}

env PATH="/opt/node/bin:/usr/bin:/bin"   // literal value, written here
env PI_PROVIDER                          // passthrough from hotcell's own env
secret ANTHROPIC_API_KEY                 // passthrough, redacted in the plan

net {
    allow "api.anthropic.com" port=443   // repeatable: an allowed endpoint
    allow "example.com"                   // port optional: any port
}
```

The nodes:

| Node | Description |
| ---- | ----------- |
| `workdir "DIR"` | Working directory inside the cell. Defaults to `/work`. |
| `provision "none"` | The no-op provisioner. This is the default when no `provision` node is present. |
| `provision "shell" script="PATH"` | Runs `PATH` (relative to the Cellfile directory) inside the sandbox to populate the cell. |
| `host-path "PATH"` | Inside a `provision` block: a host path the provisioner may read, read-only, to bootstrap from. Repeatable. The agent itself never sees these. |
| `env NAME="value"` | An environment variable with a literal value written in the Cellfile. Repeatable. |
| `env NAME` | A passthrough: hotcell reads `NAME` from its own environment at run time and gives it to the agent. Repeatable. |
| `secret NAME` | Like `env NAME`, but redacted wherever the run is printed. Repeatable. |
| `net { allow "HOST" port=N }` | An allowed egress endpoint. `port` is optional — omit it to allow any port. Repeatable. See [Network firewall](#network-firewall). |

The agent runs with a clean environment: only the variables the Cellfile
declares reach it. Unknown nodes are errors — a typo in a security-relevant
directive (`nett { allow ... }`) fails the run rather than being silently
ignored.

### Environment variables and secrets

An `env` value can be written in the Cellfile (`env NAME="value"`) or taken
from hotcell's own environment at run time. A bare `env NAME` is a
**passthrough** and `secret NAME` is the same thing **redacted** — both let a
value stay out of the Cellfile. Export it in the shell you launch hotcell from;
if a declared passthrough or secret is **not set** in hotcell's environment,
the run fails before any sandbox starts, rather than handing the agent a cell
missing a variable it was promised.

A `secret` differs from a passthrough only in how it is shown: it appears in
the execution plan as `--setenv NAME <redacted>`, with a footer noting the plan
will not run verbatim as printed — so the value stays out of terminal
scrollback and captured logs. The resolved value still reaches the agent, but a
passthrough or secret is **never written to cell state**: the on-disk record
(`.cell/<name>/provisioned_as.json`) keeps only the variable's name and source,
so a secret is not persisted and rotating it does not re-provision the cell.

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
paths absolute, so the block can be copy-pasted verbatim into a terminal —
unless a `secret` is declared, whose value the plan shows as `<redacted>`
(see [Environment variables and secrets](#environment-variables-and-secrets)).
Execution runs that same plan, argument for argument. A `[y/N]` prompt then
gates the run — `--dry-run` prints the plan and exits, `-y` skips the
prompt.

`hotcell run` exits with the program's exit code, or 1 if provisioning
failed.

## Network firewall

hotcell's network isolation is kernel-enforced (bubblewrap's `--unshare-net`)
plus a host-side HTTP CONNECT proxy:

- **No `net` block (or an empty one)** → the agent is fully offline. No proxy
  starts.
- **One or more `allow` endpoints** → an HTTP CONNECT proxy starts on a
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

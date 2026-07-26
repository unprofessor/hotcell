#!/usr/bin/env bash
# Provisioner script: bootstrap a self-contained Pi Agent inside a hotcell.
#
# This script runs INSIDE the provisioning sandbox. hotcell exposes these env
# vars (see src/provisioning.rs):
#   HOTCELL_CELL_ROOT     "/" — the cell rootfs is mounted as the sandbox root
#   HOTCELL_CELLFILE_DIR  the Cellfile's directory (read-only bind)
#   HOTCELL_WORKDIR       the in-sandbox working directory (e.g. /work)
#   HOTCELL_WORKDIR_HOST  same as HOTCELL_WORKDIR (rootfs is "/")
#
# The provisioner has read-only access to the host paths declared via
# `provision.host_path` in the Cellfile (here: the nvm node tree, ~/.pi,
# ~/.agents/skills, ~/.gitconfig). It copies/installs Pi's *own* tools into
# the cell rootfs. The agent later sees only the rootfs — none of these host
# binds.
#
# The only host content that enters the sandbox beyond these declared paths is
# the read-only base system (/usr, /lib, /bin, /etc) layered by hotcell's
# bwrap invocation.

set -euo pipefail

# Inside the sandbox the rootfs is "/", so HOTCELL_CELL_ROOT is "/".
cell_root="${HOTCELL_CELL_ROOT:-/}"
workdir_host="${HOTCELL_WORKDIR_HOST:?HOTCELL_WORKDIR_HOST must be set}"
home_host="${HOME:?HOME must be set (inherited from host for bootstrap)}"

echo ">> provisioning pi agent into cell rootfs (${cell_root})"

# --- working directory -------------------------------------------------------
mkdir -p "${workdir_host}"

# --- node: copy the host's node install tree into the cell -------------------
# `node` resolves via the inherited host PATH to the nvm tree, which is
# bind-mounted read-only at its original path. We copy the whole prefix so the
# sandbox has a self-contained node (the agent won't see the nvm bind).
node_bin="$(command -v node)"
node_real="$(readlink -f "${node_bin}")"
node_prefix="$(cd "$(dirname "${node_real}")/.." && pwd)"

echo ">> copying node from ${node_prefix} -> ${cell_root}/opt/node"
mkdir -p "${cell_root}/opt"
rm -rf "${cell_root}/opt/node"
cp -a "${node_prefix}" "${cell_root}/opt/node"

# --- pi: install into the cell's own npm global prefix -----------------------
pi_prefix="${cell_root}/opt/pi"
echo ">> installing @earendil-works/pi-coding-agent -> ${pi_prefix}"
rm -rf "${pi_prefix}"
mkdir -p "${pi_prefix}"
PATH="${cell_root}/opt/node/bin:${PATH}" \
    npm install -g --prefix "${pi_prefix}" @earendil-works/pi-coding-agent

# --- agent home: seed ~/.pi config and skills from the host (if present) -----
# These host paths are declared as provision.host_path in the Cellfile, so they
# are visible read-only here. Remove this block for a hermetic, fresh-config
# cell.
home_agent="${cell_root}/home/agent"
mkdir -p "${home_agent}"
if [ -d "${home_host}/.pi" ]; then
    echo ">> seeding ~/.pi -> ${home_agent}/.pi"
    cp -a "${home_host}/.pi" "${home_agent}/.pi"
fi
if [ -d "${home_host}/.agents/skills" ]; then
    mkdir -p "${home_agent}/.agents"
    echo ">> seeding ~/.agents/skills -> ${home_agent}/.agents/skills"
    cp -a "${home_host}/.agents/skills" "${home_agent}/.agents/skills"
fi
if [ -f "${home_host}/.gitconfig" ]; then
    cp -a "${home_host}/.gitconfig" "${home_agent}/.gitconfig"
fi

echo ">> provisioning complete"

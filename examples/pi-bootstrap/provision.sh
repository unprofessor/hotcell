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
# HOME is overridden to a cell-local path (the agent's env.HOME) so tools
# like npm cache inside the cell, not into a ghost of the host user's home.
# Host paths declared via provision.host_path are staged read-only under
# /hotcell/host/<original path>; HOTCELL_HOST_HOME points at the staged host
# home so we can copy config out of it.
cell_root="${HOTCELL_CELL_ROOT:-/}"
workdir_host="${HOTCELL_WORKDIR_HOST:?HOTCELL_WORKDIR_HOST must be set}"
host_home="${HOTCELL_HOST_HOME:?HOTCELL_HOST_HOME must be set}"

echo ">> provisioning pi agent into cell rootfs (${cell_root})"

# --- working directory -------------------------------------------------------
mkdir -p "${workdir_host}"

# --- node: copy the host's node install tree into the cell -------------------
# The nvm node tree is staged read-only under HOTCELL_HOST_HOME (i.e.
# /hotcell/host/<host home>/.nvm/versions/node/<ver>), not at its original
# host path, so `command -v node` won't find it. Resolve it through the staged
# host home and copy the whole prefix so the sandbox has a self-contained node.
node_prefix=$(echo "${HOTCELL_HOST_HOME}/.nvm/versions/node/"*)
if [ ! -x "${node_prefix}/bin/node" ]; then
    echo "!! no node found under ${HOTCELL_HOST_HOME}/.nvm/versions/node/" >&2
    exit 1
fi

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
# are visible read-only here (at their original absolute host paths under
# HOTCELL_HOST_HOME). Remove this block for a hermetic, fresh-config cell.
home_agent="${cell_root}/home/agent"
mkdir -p "${home_agent}"
if [ -d "${host_home}/.pi" ]; then
    echo ">> seeding ~/.pi -> ${home_agent}/.pi"
    cp -a "${host_home}/.pi" "${home_agent}/.pi"
fi
if [ -d "${host_home}/.agents/skills" ]; then
    mkdir -p "${home_agent}/.agents"
    echo ">> seeding ~/.agents/skills -> ${home_agent}/.agents/skills"
    cp -a "${host_home}/.agents/skills" "${home_agent}/.agents/skills"
fi
if [ -f "${host_home}/.gitconfig" ]; then
    cp -a "${host_home}/.gitconfig" "${home_agent}/.gitconfig"
fi

echo ">> provisioning complete"

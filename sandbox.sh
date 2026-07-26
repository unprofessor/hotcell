#!/usr/bin/env bash
set -euo pipefail

bwrap \
    --ro-bind /usr /usr \
    --ro-bind /lib /lib \
    --ro-bind /lib64 /lib64 \
    --ro-bind /bin /bin \
    --ro-bind /etc /etc \
    --ro-bind ~/.nvm/versions/node ~/.nvm/versions/node \
    --ro-bind ~/.bun/bin ~/.bun/bin \
    --ro-bind ~/.local/bin ~/.local/bin \
    --ro-bind ~/.cargo/bin ~/.cargo/bin \
    --bind ~/.pi ~/.pi \
    --bind ~/.gitconfig ~/.gitconfig \
    --bind ~/.config/git ~/.config/git \
    --bind "$(pwd)" "$(pwd)" \
    --tmpfs /tmp \
    --proc /proc \
    --dev /dev \
    --unshare-pid \
    --share-net \
    --die-with-parent \
    -- $1

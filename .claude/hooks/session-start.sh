#!/bin/bash
# Bootstraps a Claude-Code-on-the-web session into a known-good state so the
# linter, tests, and build work immediately. Idempotent; non-interactive.
set -euo pipefail

# Local (non-remote) sessions already have the user's environment.
if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

# Pinned components for the lint gate (rust-toolchain.toml also declares these).
rustup component add rustfmt clippy >/dev/null 2>&1 || true

# System libraries winit/wgpu/rodio link against — the same apt list CI installs.
# Without these a fresh container fails at link time (e.g. ALSA headers for `rodio`,
# issue #262). Best-effort and non-fatal: apt may be absent, restricted by the
# network policy, or the packages already present.
if command -v apt-get >/dev/null 2>&1; then
  apt_sudo=""
  if [ "$(id -u)" -ne 0 ] && command -v sudo >/dev/null 2>&1; then apt_sudo="sudo"; fi
  $apt_sudo apt-get update -qq >/dev/null 2>&1 || true
  $apt_sudo apt-get install -y -qq \
    pkg-config libxkbcommon-dev libwayland-dev libudev-dev libasound2-dev \
    >/dev/null 2>&1 || true
fi

# Warm the crate cache (cached in the container) so build/clippy/test are fast.
cargo fetch --quiet || true

# Prebuild the zero-dep size-gate tool so `cargo run -p lint` is instant.
cargo build --quiet --manifest-path tools/lint/Cargo.toml || true

echo "session-start: rust toolchain, components, and crate cache ready"

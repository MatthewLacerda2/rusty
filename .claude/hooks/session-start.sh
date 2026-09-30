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

# Activate the committed commit hook (formatting + size gate), the same one-time
# step `make setup` is locally (#485).
git config core.hooksPath .githooks || true

# `make gates` ends on cargo-deny. Fetch its prebuilt Linux binary (seconds) rather
# than compile it (minutes); best-effort — the gate names the install if it's missing.
if ! command -v cargo-deny >/dev/null 2>&1 && [ "$(uname -sm)" = "Linux x86_64" ]; then
  # 0.20+: older releases cannot parse the advisory DB's CVSS 4.0 entries.
  deny_ver=0.20.2
  deny_tmp="$(mktemp -d)"
  curl -sSfL "https://github.com/EmbarkStudios/cargo-deny/releases/download/${deny_ver}/cargo-deny-${deny_ver}-x86_64-unknown-linux-musl.tar.gz" \
    | tar xz -C "$deny_tmp" 2>/dev/null \
    && install -m 755 "$deny_tmp"/cargo-deny-*/cargo-deny "$HOME/.cargo/bin/" 2>/dev/null || true
  rm -rf "$deny_tmp"
fi

echo "session-start: rust toolchain, components, and crate cache ready"

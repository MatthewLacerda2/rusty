#!/usr/bin/env bash
# Readies a fresh cloud container so the build, the tests and `make gates` work
# immediately (#826; modelled on scorsese's, which was modelled on this one).
#
# Claude Code runs this at the start of every session (`.claude/settings.json`).
# Locally it does nothing: the machine is the operator's, already set up. In a
# cloud container (`CLAUDE_CODE_REMOTE=true`) it does, the same way every time,
# what each cloud coder used to be told in its brief.
#
# Three properties, all load-bearing:
#
# - Idempotent. Every step checks before it acts, so a resumed session, or a
#   second run by hand, costs seconds and changes nothing.
# - Best-effort. A step that fails prints what to do instead and the next one
#   runs; the hook always exits 0. The gate that needs the tool says so itself.
# - Timed. Each line carries how long its step took, so a slow container is
#   diagnosable from the session log.
#
# Run it by hand as a fresh container would:
#   CLAUDE_CODE_REMOTE=true CLAUDE_PROJECT_DIR=$PWD .claude/hooks/session-start.sh

set -u

[ "${CLAUDE_CODE_REMOTE:-}" = "true" ] || exit 0
cd "${CLAUDE_PROJECT_DIR:-$(dirname "$0")/../..}" || exit 0

# cargo-deny must be 0.20+: older releases cannot parse the advisory database's
# CVSS 4.0 entries. CI installs the latest nextest; this pin only has to be new
# enough for `.config/nextest.toml`.
NEXTEST=0.9.146
DENY=0.20.2

SUDO=
[ "$(id -u)" -eq 0 ] || SUDO=sudo
BIN="${CARGO_HOME:-$HOME/.cargo}/bin"
failed=0

# `step NAME FUNCTION`: run it, time it, and on failure say so without stopping.
step() {
    local start=$SECONDS out
    if out=$("$2" 2>&1); then
        printf 'session-start: %-10s ok   %3ss  %s\n' "$1" $((SECONDS - start)) "${out##*$'\n'}"
    else
        failed=1
        printf 'session-start: %-10s FAIL %3ss\n%s\n' "$1" $((SECONDS - start)) "$out"
    fi
}

# Exported for every later Bash call, through the file Claude Code sources after
# this hook. Run by hand there is no such file, so the lines are printed at the
# end instead, for the caller to run.
ENV_FILE=${CLAUDE_ENV_FILE:-$(mktemp)}
export_var() {
    grep -qxF "export $1=$2" "$ENV_FILE" 2>/dev/null || echo "export $1=$2" >> "$ENV_FILE"
    echo "$1=$2"
}

path() {
    case ":$PATH:" in
        *":$BIN:"*) echo "$BIN already on PATH" ;;
        *) export_var PATH "$BIN:\$PATH" ;;
    esac
}

toolchain() {
    # rust-toolchain.toml's channel, with the lint gate's components.
    rustup component add rustfmt clippy >/dev/null && rustc --version
}

apt_libs() {
    # What winit, wgpu and rodio link against (ci.yml's apt list; without ALSA a
    # fresh container fails at link time, #262), plus Mesa's lavapipe, the
    # software Vulkan driver CI renders with (#489). Without it GPU tests skip
    # and a local run is green without ever rendering (#785).
    local pkgs="pkg-config libxkbcommon-dev libwayland-dev libudev-dev libasound2-dev mesa-vulkan-drivers libvulkan1"
    # shellcheck disable=SC2086
    dpkg -s $pkgs >/dev/null 2>&1 \
        || { $SUDO apt-get update -qq && $SUDO apt-get install -y -qq --no-install-recommends $pkgs; } >/dev/null \
        || { echo "install them: sudo apt-get install -y $pkgs"; return 1; }
    echo "system libraries and lavapipe"
}

fetch() {
    # Warm the crate cache so build, clippy and test start compiling at once.
    cargo fetch --quiet && echo "crate cache warm"
}

lint_tool() {
    # The zero-dep size-gate tool, so `cargo run -p lint` is instant.
    cargo build --quiet --manifest-path tools/lint/Cargo.toml && echo "tools/lint built"
}

hooks() {
    # The committed commit hook (formatting + size gate): `make setup` locally (#485).
    git config core.hooksPath .githooks && echo "core.hooksPath -> .githooks"
}

# `prebuilt NAME VERSION URL`: the release tarball into ~/.cargo/bin, unless the
# right version is already there. Seconds, where `cargo install` is minutes; the
# GitHub release downloads pass the container's proxy (`get.nexte.st` does not).
prebuilt() {
    if "$BIN/$1" --version 2>/dev/null | grep -qF "$2"; then echo "$1 $2"; return; fi
    local tmp
    tmp=$(mktemp -d)
    curl -sSfL "$3" | tar xz -C "$tmp" \
        && install -m 755 "$(find "$tmp" -type f -name "$1" | head -1)" "$BIN/$1" \
        || { rm -rf "$tmp"; echo "install it: cargo install --locked $1@$2"; return 1; }
    rm -rf "$tmp"
    echo "$1 $2"
}
nextest() {
    # CI runs nextest (#484): per-test processes and the `gpu` test group.
    prebuilt cargo-nextest "$NEXTEST" "https://github.com/nextest-rs/nextest/releases/download/cargo-nextest-$NEXTEST/cargo-nextest-$NEXTEST-x86_64-unknown-linux-gnu.tar.gz"
}
deny() {
    # `make gates` ends on cargo-deny.
    prebuilt cargo-deny "$DENY" "https://github.com/EmbarkStudios/cargo-deny/releases/download/$DENY/cargo-deny-$DENY-x86_64-unknown-linux-musl.tar.gz"
}

step path path
case ":$PATH:" in *":$BIN:"*) ;; *) PATH="$BIN:$PATH" ;; esac  # for the steps below too
step toolchain toolchain
step apt-libs apt_libs
step fetch fetch
step lint-tool lint_tool
step hooks hooks
step nextest nextest
step deny deny

if [ -z "${CLAUDE_ENV_FILE:-}" ]; then
    if [ -s "$ENV_FILE" ]; then
        echo "session-start: not run by Claude Code, so export these yourself:"
        cat "$ENV_FILE"
    fi
    rm -f "$ENV_FILE"
fi
[ "$failed" -eq 0 ] || echo "session-start: a step failed -- the lines above say what to do; nothing else was skipped."
exit 0

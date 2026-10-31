#!/usr/bin/env bash
set -Eeuo pipefail

REPO="https://github.com/Spruhantom/Tomos-display-manager.git"
BRANCH="main"
INSTALL_DIR="/usr/local/bin"
TMP_DIR="$(mktemp -d)"
cleanup() { rm -rf "$TMP_DIR"; }
trap cleanup EXIT

say() { printf '\n[TDM installer] %s\n' "$*"; }
fail() { printf '[TDM installer] ERROR: %s\n' "$*" >&2; exit 1; }
has() { command -v "$1" >/dev/null 2>&1; }

[[ "$(id -u)" -ne 0 ]] || fail "Run this installer as a normal user, not with sudo. It will request sudo only when needed."
has sudo || fail "sudo is required to install system packages and binaries."
has curl || fail "curl is required."

if [[ ! -r /etc/os-release ]]; then
    fail "Cannot identify this Linux distribution."
fi
. /etc/os-release
ID_LIKE="${ID_LIKE:-}"
DISTRO="${ID:-unknown}"

say "Detected: ${PRETTY_NAME:-$DISTRO}"

case "$DISTRO $ID_LIKE" in
    *arch*|*manjaro*|*endeavouros*)
        has pacman || fail "pacman was not found."
        say "Installing Arch build dependencies..."
        sudo pacman -Syu --needed --noconfirm base-devel pkgconf gtk4 git curl
        ;;
    *debian*|*ubuntu*|*linuxmint*|*pop*)
        has apt-get || fail "apt-get was not found."
        say "Installing Debian/Ubuntu build dependencies..."
        sudo apt-get update
        sudo apt-get install -y build-essential pkg-config libgtk-4-dev git curl
        ;;
    *fedora*)
        has dnf || fail "dnf was not found."
        say "Installing Fedora build dependencies..."
        sudo dnf install -y gcc gcc-c++ make pkgconf-pkg-config gtk4-devel git curl
        ;;
    *)
        fail "Unsupported distribution: $DISTRO. Supported: Arch-based, Debian/Ubuntu-based, and Fedora."
        ;;
esac

if ! has rustup; then
    say "Installing the official Rust toolchain for the current user..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
fi
export PATH="$HOME/.cargo/bin:$PATH"

has rustup || fail "rustup was not installed or is not available in PATH."
if ! has cargo || ! has rustc; then
    say "Installing the stable Rust toolchain..."
    rustup toolchain install stable --profile minimal
    rustup default stable
fi

RUST_VERSION="$(rustc --version | awk '{print $2}')"
if [[ "$(printf '%s\n' 1.83.0 "$RUST_VERSION" | sort -V | head -n1)" != "1.83.0" ]]; then
    say "Updating Rust to stable (TDM requires Rust 1.83 or newer)..."
    rustup update stable
    rustup default stable
fi
has cargo || fail "Rust installation did not provide cargo."
has rustc || fail "Rust installation did not provide rustc."

has git || fail "git is required."
say "Downloading TomOS Display Manager source..."
git clone --depth 1 --branch "$BRANCH" "$REPO" "$TMP_DIR/source"

cd "$TMP_DIR/source"
say "Building TDM and its GTK4 greeter..."
cargo build --release --bins

[[ -x target/release/tdm ]] || fail "Build did not produce the tdm binary."
[[ -x target/release/tdm-greeter ]] || fail "Build did not produce the tdm-greeter binary."

say "Installing binaries into $INSTALL_DIR..."
sudo install -Dm755 target/release/tdm "$INSTALL_DIR/tdm"
sudo install -Dm755 target/release/tdm-greeter "$INSTALL_DIR/tdm-greeter"

say "Installation complete."
printf '  tdm --help\n  tdm list-sessions\n'
printf '\nTDM is experimental. This installer does NOT enable greetd or replace your current display manager.\n'
printf 'Keep your existing login manager and a recovery TTY available until TDM has been tested.\n'

#!/bin/sh
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/MarcusDunn/openapi-forge/main/install.sh | sh
#
# Inspect before running:
#   curl -fsSL https://raw.githubusercontent.com/MarcusDunn/openapi-forge/main/install.sh | less
#
# Environment variables:
#   FORGE_VERSION      Pin to a specific version (e.g. "0.1.11"). Default: latest.
#   FORGE_INSTALL_DIR  Where to put the binary.
#                      Default: $XDG_BIN_HOME > ~/.local/bin > /usr/local/bin.
set -eu

default_install_dir() {
  if [ -n "${XDG_BIN_HOME:-}" ]; then echo "$XDG_BIN_HOME"; return; fi
  if [ -n "${HOME:-}" ]; then echo "$HOME/.local/bin"; return; fi
  echo "/usr/local/bin"
}

err() { printf "error: %s\n" "$1" >&2; exit 1; }
info() { printf "  %s\n" "$1" >&2; }
warn() { printf "warning: %s\n" "$1" >&2; }

detect_target() {
  os=$(uname -s)
  arch=$(uname -m)
  case "$os" in
    Linux)
      case "$arch" in
        x86_64|amd64) TARGET="x86_64-unknown-linux-gnu" ;;
        *) err "unsupported architecture: $arch (Linux builds are x86_64 only)" ;;
      esac ;;
    Darwin)
      case "$arch" in
        arm64|aarch64) TARGET="aarch64-apple-darwin" ;;
        *) err "unsupported architecture: $arch (macOS builds are ARM64 only)" ;;
      esac ;;
    *) err "unsupported OS: $os (detected by uname -s)" ;;
  esac
}

resolve_version() {
  if [ -n "$VERSION" ]; then return; fi
  api_url="https://api.github.com/repos/$REPO/releases/latest"
  VERSION=$(curl -fsSL --retry 3 --retry-connrefused --connect-timeout 10 "$api_url" \
    | grep '"tag_name"' | head -1 | sed 's/.*"v\([^"]*\)".*/\1/') \
    || err "failed to fetch latest version from $api_url"
  [ -n "$VERSION" ] || err "could not determine latest release from $api_url"
}

download() {
  tmp=$(mktemp -d 2>/dev/null || mktemp -d -t 'forge-install')
  trap 'rm -rf "$tmp"' EXIT INT TERM

  base_url="https://github.com/$REPO/releases/download/v${VERSION}"
  archive="forge-v${VERSION}-${TARGET}.tar.gz"
  checksums="forge-v${VERSION}-SHA256SUMS"

  info "downloading $archive ..."
  curl -fsSL --retry 3 --retry-connrefused --connect-timeout 10 \
    -o "$tmp/$archive" "$base_url/$archive" \
    || err "download failed: $base_url/$archive"
  curl -fsSL --retry 3 --retry-connrefused --connect-timeout 10 \
    -o "$tmp/$checksums" "$base_url/$checksums" \
    || err "download failed: $base_url/$checksums"
}

verify_checksum() {
  info "verifying SHA-256 checksum ..."
  expected=$(grep "$archive" "$tmp/$checksums") \
    || err "archive not listed in checksums file"

  cd "$tmp"
  if command -v sha256sum >/dev/null 2>&1; then
    echo "$expected" | sha256sum -c - >/dev/null 2>&1
  elif command -v shasum >/dev/null 2>&1; then
    echo "$expected" | shasum -a 256 -c - >/dev/null 2>&1
  else
    warn "neither sha256sum nor shasum found, skipping checksum verification"
    return
  fi
  if [ $? -ne 0 ]; then
    err "checksum mismatch for $archive"
  fi
  info "checksum OK"
}

verify_attestation() {
  if ! command -v gh >/dev/null 2>&1; then
    warn "gh CLI not found, skipping SLSA attestation check"
    warn "install gh (https://cli.github.com) to verify build provenance"
    return
  fi
  info "verifying SLSA build provenance ..."
  if gh attestation verify "$tmp/$archive" --repo "$REPO" >/dev/null 2>&1; then
    info "attestation OK"
  else
    err "attestation verification failed -- the binary may have been tampered with"
  fi
}

install_binary() {
  info "extracting ..."
  tar xzf "$tmp/$archive" -C "$tmp"
  src="$tmp/forge-v${VERSION}-${TARGET}/forge"

  mkdir -p "$INSTALL_DIR" 2>/dev/null || true
  if [ -w "$INSTALL_DIR" ]; then
    mv "$src" "$INSTALL_DIR/forge"
    chmod 755 "$INSTALL_DIR/forge"
  elif command -v sudo >/dev/null 2>&1; then
    info "writing to $INSTALL_DIR (requires sudo)"
    sudo mkdir -p "$INSTALL_DIR"
    sudo mv "$src" "$INSTALL_DIR/forge"
    sudo chmod 755 "$INSTALL_DIR/forge"
  else
    err "$INSTALL_DIR is not writable and sudo is not available; set FORGE_INSTALL_DIR to a writable path"
  fi
}

main() {
  REPO="MarcusDunn/openapi-forge"
  VERSION="${FORGE_VERSION:-}"
  INSTALL_DIR="${FORGE_INSTALL_DIR:-$(default_install_dir)}"

  printf "forge installer\n\n" >&2
  detect_target
  resolve_version
  info "forge v${VERSION} for ${TARGET}"
  download
  verify_checksum
  verify_attestation
  install_binary
  printf "\n  forge v%s installed to %s/forge\n" "$VERSION" "$INSTALL_DIR" >&2

  case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *) warn "$INSTALL_DIR is not in your PATH" ;;
  esac
}

main "$@"

#!/usr/bin/env bash
set -euo pipefail

# mdhtml installer (DIST-01)
#
# Downloads the mdhtml CLI for the current platform/architecture from a GitHub
# release, verifies the SHA-256 checksum before installing, and installs it
# into a per-user bin directory on PATH. Idempotent: re-running completes what
# is missing and never replaces a newer installed version.
#
# Overrides:
#   MDHTML_REPO         owner/repo of the GitHub project (default: derived from
#                       the git remote when run inside a clone, else feliperun/md.html)
#   MDHTML_VERSION      release tag to install, e.g. v1.0.0 (default: latest release)
#   MDHTML_INSTALL_DIR  per-user bin directory (default: $HOME/.local/bin)

REPO="${MDHTML_REPO:-}"
if [ -z "$REPO" ]; then
  remote="$(git config --get remote.origin.url 2>/dev/null || true)"
  case "$remote" in
    *github.com*)
      REPO="$(printf '%s' "$remote" | sed -E 's#^.*github\.com[:/]##')"
      REPO="${REPO%.git}"
      ;;
  esac
fi
REPO="${REPO:-feliperun/md.html}"

version_ge() {
  local a="$1" b="$2" ia ib na nb
  while [ -n "$a" ] || [ -n "$b" ]; do
    ia="${a%%.*}"; ib="${b%%.*}"
    [ -z "$ia" ] && ia=0
    [ -z "$ib" ] && ib=0
    if [ "$ia" -gt "$ib" ]; then return 0; fi
    if [ "$ia" -lt "$ib" ]; then return 1; fi
    na="${a#*.}"; nb="${b#*.}"
    [ "$na" = "$a" ] && na=""
    [ "$nb" = "$b" ] && nb=""
    a="$na"; b="$nb"
  done
  return 0
}

OS="$(uname -s)"
ARCH="$(uname -m)"
case "$OS" in
  Darwin)
    case "$ARCH" in
      arm64) TARGET="darwin-arm64" ;;
      x86_64) TARGET="darwin-x64" ;;
      *) echo "install.sh: unsupported macOS architecture: $ARCH" >&2; exit 1 ;;
    esac
    ;;
  Linux)
    case "$ARCH" in
      x86_64)
        if ldd --version 2>/dev/null | grep -qi musl; then
          TARGET="linux-x64-musl"
        else
          TARGET="linux-x64-gnu"
        fi
        ;;
      *) echo "install.sh: unsupported Linux architecture: $ARCH" >&2; exit 1 ;;
    esac
    ;;
  MINGW* | MSYS* | CYGWIN*)
    case "$ARCH" in
      x86_64) TARGET="windows-x64" ;;
      *) echo "install.sh: unsupported Windows architecture: $ARCH" >&2; exit 1 ;;
    esac
    ;;
  *) echo "install.sh: unsupported platform: $OS" >&2; exit 1 ;;
esac

if [ -n "${MDHTML_VERSION:-}" ]; then
  TAG="v${MDHTML_VERSION#v}"
else
  redirect="$(curl -fsS -o /dev/null -w '%{redirect_url}' "https://github.com/${REPO}/releases/latest")"
  TAG="$(printf '%s' "$redirect" | sed -n 's#.*/releases/tag/##p')"
  [ -n "$TAG" ] || { echo "install.sh: could not resolve the latest release of ${REPO}" >&2; exit 1; }
fi
VER="${TAG#v}"

INSTALL_DIR="${MDHTML_INSTALL_DIR:-$HOME/.local/bin}"
BINARY="mdhtml"
[ "$TARGET" = "windows-x64" ] && BINARY="mdhtml.exe"

archive="mdhtml-${VER}-${TARGET}.tar.gz"
base_url="https://github.com/${REPO}/releases/download/${TAG}"
installed="$INSTALL_DIR/$BINARY"

if [ -x "$installed" ]; then
  installed_ver="$("$installed" --version 2>/dev/null | awk '{print $2}' || true)"
  if [ -n "$installed_ver" ] && version_ge "$installed_ver" "$VER"; then
    echo "install.sh: mdhtml ${installed_ver} already installed at ${installed}; nothing to do"
    exit 0
  fi
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

echo "install.sh: downloading ${base_url}/${archive}"
curl -fsSL -o "$work/$archive" "${base_url}/${archive}"
curl -fsSL -o "$work/$archive.sha256" "${base_url}/${archive}.sha256"

if command -v sha256sum >/dev/null 2>&1; then
  (cd "$work" && sha256sum -c "$archive.sha256")
else
  (cd "$work" && shasum -a 256 -c "$archive.sha256")
fi

tar -xzf "$work/$archive" -C "$work"
mkdir -p "$INSTALL_DIR"
install -m 0755 "$work/$BINARY" "$installed"
echo "install.sh: installed mdhtml ${VER} at ${installed}"

case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *) echo "install.sh: add ${INSTALL_DIR} to your PATH, e.g. export PATH=\"${INSTALL_DIR}:\$PATH\"" ;;
esac

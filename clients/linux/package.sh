#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LINUX_DIR="$ROOT_DIR/clients/linux"
DIST_DIR="$LINUX_DIR/dist"
BUILD_DIR="$LINUX_DIR/build/package"
# gotk3 v0.6.4's GTK 3.22 GDK binding does not compile with current Go/cgo.
GUI_TAGS="${GUI_TAGS:-gtk_3_20}"

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "clients/linux/package.sh must be run on Linux." >&2
  exit 1
fi

if ! pkg-config --exists gtk+-3.0 ayatana-appindicator3-0.1; then
  echo "Missing GTK 3 or Ayatana AppIndicator development files." >&2
  echo "Ubuntu/Debian: sudo apt install libgtk-3-dev libayatana-appindicator3-dev pkg-config" >&2
  echo "Fedora: sudo dnf install gtk3-devel libayatana-appindicator-gtk3-devel pkgconf-pkg-config" >&2
  echo "Arch: sudo pacman -S gtk3 libayatana-appindicator pkgconf" >&2
  exit 1
fi

VERSION="${VERSION:-$(git -C "$ROOT_DIR" describe --tags --always --dirty 2>/dev/null || echo dev)}"
ARCH="$(dpkg --print-architecture 2>/dev/null || uname -m)"
case "$ARCH" in
  x86_64) ARCH="amd64" ;;
  aarch64|arm64) ARCH="arm64" ;;
esac

PKG_NAME="copi-linux-${VERSION}-${ARCH}"
STAGE="$BUILD_DIR/$PKG_NAME"

rm -rf "$STAGE"
mkdir -p \
  "$STAGE/usr/bin" \
  "$STAGE/usr/share/applications" \
  "$STAGE/usr/share/icons/hicolor/scalable/apps" \
  "$DIST_DIR"

cd "$ROOT_DIR"
go build -trimpath -ldflags="-s -w" -o "$STAGE/usr/bin/copi" ./cmd/copi
go build -tags "$GUI_TAGS" -trimpath -ldflags="-s -w" -o "$STAGE/usr/bin/copi-linux-gui" ./clients/linux/cmd/copi-linux-gui

install -m 0644 "$LINUX_DIR/share/applications/com.cyole.copi.desktop" "$STAGE/usr/share/applications/com.cyole.copi.desktop"
install -m 0644 "$LINUX_DIR/share/icons/hicolor/scalable/apps/com.cyole.copi.svg" "$STAGE/usr/share/icons/hicolor/scalable/apps/com.cyole.copi.svg"

tar -C "$BUILD_DIR" -czf "$DIST_DIR/$PKG_NAME.tar.gz" "$PKG_NAME"
echo "Wrote $DIST_DIR/$PKG_NAME.tar.gz"

if command -v dpkg-deb >/dev/null 2>&1; then
  DEBIAN="$STAGE/DEBIAN"
  mkdir -p "$DEBIAN"
  installed_size="$(du -sk "$STAGE/usr" | awk '{print $1}')"
  cat >"$DEBIAN/control" <<EOF
Package: copi
Version: ${VERSION#v}
Section: utils
Priority: optional
Architecture: $ARCH
Maintainer: Copi <noreply@example.com>
Depends: libgtk-3-0, libayatana-appindicator3-1
Installed-Size: $installed_size
Description: Clipboard sync tray client
 Copi syncs clipboard content through a relay service or local network peers.
EOF
  dpkg-deb --build "$STAGE" "$DIST_DIR/${PKG_NAME}.deb"
  echo "Wrote $DIST_DIR/${PKG_NAME}.deb"
fi

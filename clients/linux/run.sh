#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LINUX_DIR="$ROOT_DIR/clients/linux"
BUILD_DIR="$LINUX_DIR/build"
CLI_PATH="$BUILD_DIR/copi"
GUI_PATH="$BUILD_DIR/copi-linux-gui"
# gotk3 v0.6.4's GTK 3.22 GDK binding does not compile with current Go/cgo.
GUI_TAGS="${GUI_TAGS:-gtk_3_20}"

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "clients/linux/run.sh must be run on Linux." >&2
  exit 1
fi

if ! pkg-config --exists gtk+-3.0; then
  echo "Missing GTK 3 development files." >&2
  echo "Ubuntu/Debian: sudo apt install libgtk-3-dev pkg-config" >&2
  echo "Fedora: sudo dnf install gtk3-devel pkgconf-pkg-config" >&2
  exit 1
fi

if [[ "${1:-}" == "--no-build" ]]; then
  if [[ ! -x "$GUI_PATH" ]]; then
    echo "Missing $GUI_PATH. Run clients/linux/run.sh once without --no-build first." >&2
    exit 1
  fi
else
  mkdir -p "$BUILD_DIR"
  cd "$ROOT_DIR"
  go build -o "$CLI_PATH" ./cmd/copi
  go build -tags "$GUI_TAGS" -o "$GUI_PATH" ./clients/linux/cmd/copi-linux-gui
fi

export COPI_CLI="${COPI_CLI:-$CLI_PATH}"
export COPI_ICON="${COPI_ICON:-$LINUX_DIR/assets/com.cyole.copi-symbolic.svg}"
exec "$GUI_PATH"

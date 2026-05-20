#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LINUX_DIR="$ROOT_DIR/clients/linux"
BUILD_DIR="$LINUX_DIR/build"
CLI_PATH="$BUILD_DIR/copi"

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "clients/linux/run.sh must be run on Linux." >&2
  exit 1
fi

python3 - <<'PY'
import sys

try:
    import gi
    gi.require_version("Gtk", "3.0")
    from gi.repository import Gtk  # noqa: F401
except Exception as error:
    print("Missing GTK Python bindings:", error, file=sys.stderr)
    print("Ubuntu/Debian: sudo apt install python3-gi gir1.2-gtk-3.0 gir1.2-ayatanaappindicator3-0.1", file=sys.stderr)
    print("Fedora: sudo dnf install python3-gobject gtk3 libayatana-appindicator-gtk3", file=sys.stderr)
    sys.exit(1)
PY

if [[ "${1:-}" != "--no-build" ]]; then
  mkdir -p "$BUILD_DIR"
  cd "$ROOT_DIR"
  go build -o "$CLI_PATH" ./cmd/copi
fi

export COPI_CLI="${COPI_CLI:-$CLI_PATH}"
exec python3 "$LINUX_DIR/copi_gtk.py"

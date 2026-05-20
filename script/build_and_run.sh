#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-run}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROJECT="$ROOT_DIR/clients/macos/Copi.xcodeproj"
SCHEME="Copi"
DERIVED_DATA="$ROOT_DIR/build/macos"
APP_BUNDLE="$DERIVED_DATA/Build/Products/Debug/Copi.app"

pkill -x Copi >/dev/null 2>&1 || true

case "$MODE" in
  run|--verify|verify|--logs|logs|--telemetry|telemetry)
    xcodebuild \
      -project "$PROJECT" \
      -scheme "$SCHEME" \
      -configuration Debug \
      -derivedDataPath "$DERIVED_DATA" \
      build
    ;;
  --debug|debug)
    xcodebuild \
      -project "$PROJECT" \
      -scheme "$SCHEME" \
      -configuration Debug \
      -derivedDataPath "$DERIVED_DATA" \
      build
    lldb -- "$APP_BUNDLE/Contents/MacOS/Copi"
    exit $?
    ;;
  *)
    echo "usage: $0 [run|--debug|--logs|--telemetry|--verify]" >&2
    exit 2
    ;;
esac

open_app() {
  /usr/bin/open -n "$APP_BUNDLE"
}

case "$MODE" in
  run)
    open_app
    ;;
  --verify|verify)
    open_app
    sleep 1
    pgrep -x Copi >/dev/null
    ;;
  --logs|logs)
    open_app
    /usr/bin/log stream --info --style compact --predicate 'process == "Copi"'
    ;;
  --telemetry|telemetry)
    open_app
    /usr/bin/log stream --info --style compact --predicate 'process == "Copi"'
    ;;
esac

#!/bin/zsh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"

if ! command -v cargo >/dev/null 2>&1; then
  if [[ -f "$HOME/.cargo/env" ]]; then
    source "$HOME/.cargo/env"
  else
    echo "Rust is not installed. Run: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
  fi
fi

export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-11.0}"
export CARGO_TARGET_DIR="$ROOT/target"
PROFILE="${1:-release}"

# Host is already x86_64-apple-darwin on this Intel Mac.
if [[ "$PROFILE" == "debug" ]]; then
  cargo build
  BIN="$ROOT/target/debug/eco-view"
else
  cargo build --release
  BIN="$ROOT/target/release/eco-view"
fi

if [[ ! -x "$BIN" ]]; then
  echo "Build finished but $BIN is missing."
  exit 1
fi

APP="$ROOT/outputs/eco-view.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$ROOT/macos/Info.plist" "$APP/Contents/Info.plist"
cp "$BIN" "$APP/Contents/MacOS/eco-view"
chmod +x "$APP/Contents/MacOS/eco-view"

echo "Built $APP"
file "$APP/Contents/MacOS/eco-view"

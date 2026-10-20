#!/bin/sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT_DIR"

VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
if [ -z "$VERSION" ]; then
    echo "Could not read package version from Cargo.toml" >&2
    exit 1
fi
SHORT_VERSION=${VERSION%.*}

cargo build --locked --release

APP="$ROOT_DIR/dist/Preview maker.app"
CONTENTS="$APP/Contents"
rm -rf "$CONTENTS/MacOS" "$CONTENTS/Resources"
mkdir -p "$CONTENTS/MacOS" "$CONTENTS/Resources"
cp target/release/preview-maker "$CONTENTS/MacOS/Preview maker"
sed -e "s/@VERSION@/$VERSION/g" -e "s/@SHORT_VERSION@/$SHORT_VERSION/g" \
    macos/Info.plist.in > "$CONTENTS/Info.plist"
printf 'APPL????' > "$CONTENTS/PkgInfo"
cp macos/Credits.rtf "$CONTENTS/Resources/Credits.rtf"

cp macos/AppIcon.icns "$CONTENTS/Resources/AppIcon.icns"
/usr/bin/plutil -lint "$CONTENTS/Info.plist"
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict "$APP"

printf 'Created %s\n' "$APP"

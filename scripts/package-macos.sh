#!/bin/bash
set -euo pipefail
project_dir="$(cd "$(dirname "$0")/.." && pwd)"
cd "$project_dir"
export CARGO_TARGET_DIR="$project_dir/.work/typeless-switch-target"
version="$(node -p "require('./package.json').version")"
build_id="${TYPELESS_BUILD_ID:-$(date +%Y%m%d-%H%M%S)}"
[[ "$build_id" =~ ^[0-9]{8}-[0-9]{6}$ ]] || { printf "Invalid build ID\n" >&2; exit 1; }
release_dir="$project_dir/releases/$build_id"
stage_dir="$project_dir/.work/package-$build_id"
mkdir -p "$release_dir" "$stage_dir"
./node_modules/.bin/tauri build --ci --bundles app --config '{"bundle":{"macOS":{"signingIdentity":"-"}}}' -- "$@"
app="$CARGO_TARGET_DIR/release/bundle/macos/Typeless Switch.app"
codesign --verify --deep --strict "$app"
ditto "$app" "$stage_dir/Typeless Switch.app"
ln -s /Applications "$stage_dir/Applications"
dmg="$release_dir/Typeless-Switch-$version-$build_id-macOS-AppleSilicon.dmg"
if hdiutil create -ov -volname 'Typeless Switch' -srcfolder "$stage_dir" -format UDZO "$dmg"; then
  hdiutil verify "$dmg"
  installer="$dmg"
else
  # Disk images need macOS disk services, which may be unavailable in a build sandbox.
  rm -f "$dmg"
  installer="$release_dir/Typeless-Switch-$version-$build_id-macOS-AppleSilicon.zip"
  ditto -c -k --sequesterRsrc --keepParent "$app" "$installer"
  unzip -tq "$installer"
fi
shasum -a 256 "$installer" > "$release_dir/SHA256.txt"
# APPs used for development remain in .work; installers only go into releases.
rm -rf "$stage_dir"
printf '%s\n' "$release_dir" > "$project_dir/.work/latest-release-path"
printf 'Installer: %s\n' "$installer"

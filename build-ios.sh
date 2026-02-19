#!/bin/bash
set -e
cd "$(dirname "$0")"

echo "==> Building frontend..."
npm run build

echo "==> Syncing app icons..."
cp src-tauri/icons/ios/*.png src-tauri/gen/apple/Assets.xcassets/AppIcon.appiconset/

echo "==> Building iOS app..."
npx @tauri-apps/cli ios build --config '{"build":{"beforeBuildCommand":""}}'

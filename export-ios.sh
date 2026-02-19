#\!/bin/bash
set -e
cd "$(dirname "$0")"

echo "==> Exporting for App Store..."
rm -rf src-tauri/gen/apple/build/appstore-export
xcodebuild -exportArchive \
  -archivePath src-tauri/gen/apple/build/ghost-auth_iOS.xcarchive \
  -exportPath src-tauri/gen/apple/build/appstore-export \
  -exportOptionsPlist src-tauri/gen/apple/build/ExportOptions-AppStore.plist \
  -allowProvisioningUpdates

echo ""
echo "==> Done\! IPA ready at:"
echo "    src-tauri/gen/apple/build/appstore-export/Ghost Auth.ipa"

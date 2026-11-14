#!/usr/bin/env bash
set -euo pipefail

if test "$#" -ne 1; then
  echo "Usage: $0 <tag>" >&2
  exit 1
fi

TAG="$1"

mkdir -p release-packages

package_group() {
  local source_dir="$1"
  shift

  for pattern in "$@"; do
    while IFS= read -r file; do
      cp "$file" "release-packages/$(basename "$file")"
    done < <(find "$source_dir" -type f -name "$pattern")
  done
}

# Linux packages
package_group "release-assets/linux-x64" "*.deb" "*.deb.sig"
package_group "release-assets/linux-x64" "*.rpm" "*.rpm.sig" "*.AppImage" "*.AppImage.sig" "*.AppImage.tar.gz" "*.AppImage.tar.gz.sig"

# Windows packages
package_group "release-assets/windows-x64" "*.msi" "*.exe" "*.exe.sig"

# Flatpak package
package_group "release-assets/linux-flatpak" "*.flatpak"

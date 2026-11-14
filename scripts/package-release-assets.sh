#!/usr/bin/env bash
set -euo pipefail

if test "$#" -ne 1; then
  echo "Usage: $0 <tag>" >&2
  exit 1
fi

TAG="$1"
VERSION="${TAG#v}"

mkdir -p release-packages

versioned_name() {
  local name="$1"

  case "$name" in
    MatriXgram.flatpak)
      printf 'MatriXgram_%s.flatpak' "$VERSION"
      ;;
    *)
      if [[ "$name" =~ ([_-])v?[0-9]+\.[0-9]+\.[0-9]+([_-]) ]]; then
        printf '%s' "$name" | sed -E "s/([_-])v?[0-9]+\\.[0-9]+\\.[0-9]+([_-])/\\1${VERSION}\\2/"
      else
        echo "Cannot find a version field in installer filename: $name" >&2
        return 1
      fi
      ;;
  esac
}

package_group() {
  local source_dir="$1"
  shift

  for pattern in "$@"; do
    while IFS= read -r -d '' file; do
      local name
      name="$(basename "$file")"
      cp "$file" "release-packages/$(versioned_name "$name")"
    done < <(find "$source_dir" -type f -name "$pattern" -print0)
  done
}

# Linux packages
package_group "release-assets/linux-x64" "*.deb" "*.deb.sig"
package_group "release-assets/linux-x64" "*.rpm" "*.rpm.sig" "*.AppImage" "*.AppImage.sig" "*.AppImage.tar.gz" "*.AppImage.tar.gz.sig"

# Windows packages
package_group "release-assets/windows-x64" "*.msi" "*.exe" "*.exe.sig"

# Flatpak package
package_group "release-assets/linux-flatpak" "*.flatpak"
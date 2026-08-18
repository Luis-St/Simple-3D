#!/usr/bin/env bash
# Build the Debian package around an already-built release binary.
#
# The .deb is an addition to the portable executable, not a replacement for it:
# it is the same single binary, plus the three files a desktop Linux needs to
# show the application in a menu and to open .simple3d files by double click.
#
# Usage: packaging/deb/build.sh <binary> <version> [output-dir]
#
# Nothing here is installed by the caller beyond dpkg-dev, which every Debian
# and Ubuntu runner already has, so the packaging path stays as reproducible as
# the build itself.
set -euo pipefail

binary=$(readlink -f "${1:?usage: build.sh <binary> <version> [output-dir]}")
version="${2:?missing version}"
outdir=$(readlink -f "${3:-dist}")

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(cd -- "$here/../.." && pwd)

package="simple-3d"
appid="net.simple3d.Simple3D"
maintainer="Luis Staudt <mail@luis-st.net>"
arch=$(dpkg --print-architecture)

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
tree="$work/$package"

install -d "$tree/DEBIAN"
install -Dm755 "$binary" "$tree/usr/bin/$package"
install -Dm644 "$here/$appid.desktop" "$tree/usr/share/applications/$appid.desktop"
install -Dm644 "$here/$appid.svg" "$tree/usr/share/icons/hicolor/scalable/apps/$appid.svg"
install -Dm644 "$here/$appid.xml" "$tree/usr/share/mime/packages/$appid.xml"

# gdk-pixbuf identifies an SVG by sniffing the first bytes of the file rather
# than by its extension, and GNOME Shell loads menu icons through gdk-pixbuf.
# Anything long ahead of the `<svg` tag -- a comment, most easily -- pushes it
# past what is sniffed, and the icon silently becomes an empty tile. Cheaper to
# assert here than to discover in the applications menu.
if [ "$(head -c 200 "$here/$appid.svg" | grep -c '<svg')" -eq 0 ]; then
    echo "build.sh: <svg> must start within the first 200 bytes of $appid.svg" >&2
    exit 1
fi

# The desktop and MIME caches are refreshed by the file triggers that
# desktop-file-utils and shared-mime-info already declare on these directories,
# so the package needs no maintainer scripts of its own.

install -d "$tree/usr/share/doc/$package"
{
    printf 'Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n'
    printf 'Upstream-Name: Simple 3D\n'
    printf 'Source: https://github.com/Luis-St/Simple-3D\n\n'
    printf 'Files: *\nCopyright: 2026 Luis Staudt\nLicense: MIT\n'
    # A blank line inside a field ends it, so the licence body is indented and
    # its empty lines are marked with a lone full stop, as the format requires.
    sed -e 's/^/ /' -e 's/^ $/ ./' "$root/LICENSE"
} > "$tree/usr/share/doc/$package/copyright"
chmod 644 "$tree/usr/share/doc/$package/copyright"

{
    printf '%s (%s) stable; urgency=medium\n\n' "$package" "$version"
    printf '  * Release %s. The changes are listed at\n' "$version"
    printf '    https://github.com/Luis-St/Simple-3D/releases/tag/v%s\n\n' "$version"
    printf ' -- %s  %s\n' "$maintainer" "$(date -R)"
} | gzip -9n > "$tree/usr/share/doc/$package/changelog.Debian.gz"
chmod 644 "$tree/usr/share/doc/$package/changelog.Debian.gz"

# The library dependencies are read off the binary rather than written down by
# hand, so a new link-time dependency can never quietly go missing from the
# package. dpkg-shlibdeps insists on being run from a source tree, hence the
# stub control file, and the staged tree laid out where it expects to find it.
install -d "$work/debian"
printf 'Source: %s\n\nPackage: %s\nArchitecture: %s\n' "$package" "$package" "$arch" \
    > "$work/debian/control"
depends=$(cd "$work" && dpkg-shlibdeps -O --ignore-missing-info "$package/usr/bin/$package")
depends="${depends#shlibs:Depends=}"
if [ -z "$depends" ]; then
    echo "build.sh: dpkg-shlibdeps found no dependencies for $binary" >&2
    exit 1
fi

# What dpkg-shlibdeps cannot see: the window and graphics libraries are opened
# with dlopen at startup, not linked, so the binary's NEEDED list is only libc,
# libm and libgcc. They are named here instead. X11 is the hard requirement
# because the Wayland backend falls back to it, which is why Wayland is only
# recommended -- a machine without it still runs, through XWayland or Xorg.
depends="$depends, libx11-6, libxcursor1, libxi6, libxkbcommon0, libegl1, libgl1"
recommends="libwayland-client0, libwayland-egl1"

# Reported by apt before the download, so it is worth being accurate: the size
# dpkg wants is the installed footprint in whole kibibytes.
installed_size=$(du -k -s --apparent-size "$tree/usr" | cut -f1)

cat > "$tree/DEBIAN/control" <<CONTROL
Package: $package
Version: $version
Architecture: $arch
Maintainer: $maintainer
Installed-Size: $installed_size
Depends: $depends
Recommends: $recommends
Section: graphics
Priority: optional
Homepage: https://github.com/Luis-St/Simple-3D
Description: Parametric 3D modelling with exact metric dimensions
 Simple 3D assembles models out of parametric primitives whose dimensions are
 typed in as exact millimetres, and exports them as STL, OBJ, PLY or 3MF for
 slicers. Editing a dimension rewrites that parameter rather than stretching
 anything, so every number typed in is reproduced exactly in the mesh.
 .
 The viewport is a software rasterizer, so no accelerated graphics stack is
 required. The application makes no network connections.
CONTROL
chmod 644 "$tree/DEBIAN/control"

(cd "$tree" && find usr -type f -print0 | sort -z | xargs -0 md5sum > DEBIAN/md5sums)
chmod 644 "$tree/DEBIAN/md5sums"

mkdir -p "$outdir"
deb="$outdir/${package}_${version}_${arch}.deb"
# --root-owner-group keeps the archive owned by root without needing fakeroot,
# so the file installs the same way whoever built it.
dpkg-deb --root-owner-group --build "$tree" "$deb" >/dev/null
echo "$deb"

# Maintainer: Petexy <https://github.com/Petexy>
#
# Release package built from the checkout this file lives in:
#   makepkg -si
#
# The crate takes the Linexin Center's crates from its repository; Cargo
# fetches them, and Cargo.lock pins the commit.
#
# Same name, files and install script as the Python package it replaces,
# except that d-affinity_installer.py becomes d-affinity_installer.so: the
# Center keeps the widget's place and hidden state across the upgrade.

pkgname=affinity-installer2
pkgver=3.0.0
pkgrel=1
pkgdesc='Smart Installer for Affinity suite for Linux'
url='https://github.com/Petexy/Affinity_Installer_For_Linux'
arch=('x86_64')
license=('GPL-3.0-or-later')
# curl downloads the installer (the Python widget used urllib).
depends=('linexin-center>=4.0.0' 'gtk4' 'libadwaita' 'curl')
makedepends=('cargo')
install=affinity-installer2.install

_root="$(realpath "$startdir")"


build() {
  cd "$_root"
  export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$_root/target}"
  # Distribution builds must run on any x86-64 CPU: drop a host-specific
  # target-cpu from makepkg.conf (e.g. CachyOS's "native") and pin the baseline.
  export RUSTFLAGS="${RUSTFLAGS//-C target-cpu=native/} -C target-cpu=x86-64"
  cargo build --release --locked --lib
}

package() {
  cd "$_root"
  local release="${CARGO_TARGET_DIR:-$_root/target}/release"

  install -Dm644 "$release/libaffinity_installer.so" \
    "$pkgdir/usr/share/linexin/widgets/d-affinity_installer.so"

  # Everything else the Python package shipped, with its modes.
  local file
  while IFS= read -r -d '' file; do
    if [[ -x "data/$file" ]]; then
      install -Dm755 "data/$file" "$pkgdir/$file"
    else
      install -Dm644 "data/$file" "$pkgdir/$file"
    fi
  done < <(cd data && find . -type f -printf '%P\0')

  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}

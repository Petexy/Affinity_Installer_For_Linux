# Maintainer: Petexy <https://github.com/Petexy>

pkgname=affinity-installer
pkgver=4.1.0.r
pkgrel=1
_currentdate=$(date +"%Y-%m-%d%H-%M-%S")
pkgdesc='Smart Installer for Affinity suite for Linux'
url='https://github.com/Petexy'
arch=(x86_64)
license=('GPL-3.0')
depends=(
  python-gobject
  gtk4
  libadwaita
  python
  linexin-center
)
makedepends=(
)
install="${pkgname}.install"

package() {
   cp -rf ${srcdir}/* ${pkgdir}/
}

# Maintainer: Petexy <https://github.com/Petexy>

pkgname=affinity-installer
pkgver=2.0.3.r
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
)
makedepends=(
)

package() {
   mkdir -p ${pkgdir}/usr/bin
   cp -rf ${pkgname} ${pkgdir}/usr/bin/${pkgname}
   mkdir -p ${pkgdir}/usr/share/locale
   cp -rf ${srcdir}/locale ${pkgdir}/usr/share/
   cp -rf ${srcdir}/icons ${pkgdir}/usr/share/
   cp -rf ${srcdir}/applications ${pkgdir}/usr/share/
}

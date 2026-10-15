# Maintainer: Petexy <https://github.com/Petexy>

pkgname=affinity-installer2
pkgver=2.0.0.r
pkgrel=1
pkgdesc='Smart Installer for Affinity suite for Linux'
url='https://github.com/Petexy'
arch=('x86_64')
license=('GPL-3.0')
depends=(
  'python-gobject'
  'gtk4'
  'libadwaita'
  'python'
  'linexin-center'
)
install="${pkgname}.install"

package() {
    cd "${srcdir}"

    find . -mindepth 1 -type f | while IFS= read -r _file; do
        local _dest="${_file#./}"
        if [[ "${_dest}" == usr/bin/* ]]; then
            install -Dm755 "${_file}" "${pkgdir}/${_dest}"
        else
            install -Dm644 "${_file}" "${pkgdir}/${_dest}"
        fi
    done
}

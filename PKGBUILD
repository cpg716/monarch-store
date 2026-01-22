# Maintainer: cpg716 <cpg716@github.com>
pkgname=monarch-store
_pkgname=monarch-store
pkgver=0.2.0
pkgrel=1
pkgdesc="A modern, high-performance software store for Arch Linux based distributions."
arch=('x86_64')
url="https://github.com/cpg716/monarch-store"
license=('MIT')
depends=('gtk3' 'webkit2gtk-4.1' 'libappindicator-gtk3' 'librsvg')
provides=("$_pkgname")
conflicts=("$_pkgname")
options=('!strip')

# Download the .deb from your GitHub release
source=("https://github.com/cpg716/monarch-store/releases/download/$pkgver/MonARCH.Store_${pkgver}_amd64.deb")
sha256sums=('9a5ac0dc8918dfbc02b413cacc2330265f57cf2c89838607524c1d51a3c6824a')

package() {
  # Extract the .deb data archive
  tar -xJf data.tar.xz -C "${pkgdir}"
  
  # Ensure the binary is executable
  chmod +x "${pkgdir}/usr/bin/monarch-store"
}

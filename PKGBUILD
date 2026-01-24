# Maintainer: cpg716 <cpg716@github.com>
pkgname=monarch-store
_pkgname=monarch-store
pkgver=0.2.25
pkgrel=1
pkgdesc="A modern, high-performance software store for Arch Linux based distributions."
arch=('x86_64')
url="https://github.com/cpg716/monarch-store"
license=('MIT')
depends=('gtk3' 'webkit2gtk-4.1' 'libappindicator-gtk3' 'librsvg' 'polkit')
provides=("$_pkgname")
conflicts=("$_pkgname")
options=('!strip')

# Download the .deb from your GitHub release
source=("https://github.com/cpg716/monarch-store/releases/download/v${pkgver}/MonARCH.Store_${pkgver}_amd64.deb")
sha256sums=('356ad817875e7ebebb947d295088980cb9a100ea5c03e55013d3be46fb519eef')

package() {
  # Extract the .deb file members (ar is more robust for .deb)
  ar x "MonARCH.Store_${pkgver}_amd64.deb"
  
  # Extract the data archive to the package directory
  if [ -f data.tar.xz ]; then
    tar -xJf data.tar.xz -C "${pkgdir}"
  elif [ -f data.tar.zst ]; then
    tar -xaf data.tar.zst -C "${pkgdir}"
  elif [ -f data.tar.gz ]; then
    tar -xzf data.tar.gz -C "${pkgdir}"
  fi

  # Rename the desktop file to a standard name without spaces
  if [ -f "${pkgdir}/usr/share/applications/MonARCH Store.desktop" ]; then
    mv "${pkgdir}/usr/share/applications/MonARCH Store.desktop" \
       "${pkgdir}/usr/share/applications/monarch-store.desktop"
  fi
  
  # Fix permissions: 755 for directories and binaries, 644 for files
  find "${pkgdir}/usr" -type d -exec chmod 755 {} +
  find "${pkgdir}/usr/bin" -type f -exec chmod 755 {} +
  find "${pkgdir}/usr/share" -type f -exec chmod 644 {} +
}

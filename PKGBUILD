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

  # 1. Standardize Binary Name (Rename whatever Tauri produced to monarch-store)
  # Look in /usr/bin/ for anything and rename to monarch-store
  if [ -d "${pkgdir}/usr/bin" ]; then
    cd "${pkgdir}/usr/bin"
    # Find any file that isn't monarch-store and rename it
    # Tauri v2 often keeps spaces in binary names from productName
    find . -maxdepth 1 -type f ! -name "monarch-store" -exec mv {} monarch-store \;
    chmod +x monarch-store
    cd - > /dev/null
  fi

  # 2. Standardize Desktop File
  # Rename and then update internal fields to point to the new binary name
  if [ -f "${pkgdir}/usr/share/applications/MonARCH Store.desktop" ]; then
    mv "${pkgdir}/usr/share/applications/MonARCH Store.desktop" \
       "${pkgdir}/usr/share/applications/monarch-store.desktop"
  fi

  if [ -f "${pkgdir}/usr/share/applications/monarch-store.desktop" ]; then
    sed -i "s/^Exec=.*/Exec=monarch-store/" "${pkgdir}/usr/share/applications/monarch-store.desktop"
    sed -i "s/^Icon=.*/Icon=monarch-store/" "${pkgdir}/usr/share/applications/monarch-store.desktop"
  fi
  
  # 3. Standardize Icon Name
  if [ -f "${pkgdir}/usr/share/icons/hicolor/512x512/apps/MonARCH Store.png" ]; then
    mv "${pkgdir}/usr/share/icons/hicolor/512x512/apps/MonARCH Store.png" \
       "${pkgdir}/usr/share/icons/hicolor/512x512/apps/monarch-store.png"
  fi

  # Fix permissions: 755 for directories and binaries, 644 for files
  find "${pkgdir}/usr" -type d -exec chmod 755 {} +
  find "${pkgdir}/usr/bin" -type f -exec chmod 755 {} +
  find "${pkgdir}/usr/share" -type f -exec chmod 644 {} +
}

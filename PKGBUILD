# Build from source for perfect library compatibility
pkgname=monarch-store
pkgver=0.2.28
pkgrel=1
pkgdesc="A modern, high-performance software store for Arch Linux based distributions."
arch=('x86_64' 'aarch64')
url="https://github.com/cpg716/monarch-store"
license=('MIT')
depends=('gtk3' 'webkit2gtk-4.1' 'libappindicator-gtk3' 'librsvg' 'polkit')
makedepends=('nodejs' 'npm' 'rust' 'cargo')
source=("git+${url}.git#tag=v${pkgver}")
sha256sums=('SKIP')

build() {
  cd "${srcdir}/${pkgname}"
  
  # 1. Install frontend deps
  npm install
  
  # 2. Build Tauri release (no-bundle since we package manually)
  npx tauri build --no-bundle
}

package() {
  cd "${srcdir}/${pkgname}"
  
  # Create directory structure
  mkdir -p "${pkgdir}/usr/bin"
  mkdir -p "${pkgdir}/usr/share/applications"
  mkdir -p "${pkgdir}/usr/share/icons/hicolor/512x512/apps"
  
  # Find and install the binary (Standardized name)
  # Look in the Tauri build output directory
  local binary_path="src-tauri/target/release/monarch-store"
  if [ ! -f "$binary_path" ]; then
    # Fallback search if name differs
    binary_path=$(find src-tauri/target/release -maxdepth 1 -type f -executable -not -name "*.so" -not -name "*.d" | head -n 1)
  fi
  
  install -Dm755 "$binary_path" "${pkgdir}/usr/bin/monarch-store"
  
  # Install Desktop File
  install -Dm644 "src-tauri/monarch-store.desktop" "${pkgdir}/usr/share/applications/monarch-store.desktop" || \
  cat <<EOF > "${pkgdir}/usr/share/applications/monarch-store.desktop"
[Desktop Entry]
Name=MonARCH Store
Comment=Modern Arch Software Store
Exec=monarch-store
Icon=monarch-store
Terminal=false
Type=Application
Categories=System;Settings;
EOF

  # Install Icons
  install -Dm644 "src-tauri/icons/128x128.png" "${pkgdir}/usr/share/icons/hicolor/128x128/apps/monarch-store.png"
  install -Dm644 "src-tauri/icons/icon.png" "${pkgdir}/usr/share/icons/hicolor/512x512/apps/monarch-store.png" || true
}

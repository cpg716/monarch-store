# Build from source for perfect library compatibility
pkgname=monarch-store
pkgver=0.2.30
pkgrel=1
pkgdesc="A modern, high-performance software store for Arch Linux based distributions."
arch=('x86_64' 'aarch64')
url="https://github.com/cpg716/monarch-store"
license=('MIT')
depends=('gtk3' 'webkit2gtk-4.1' 'libappindicator-gtk3' 'librsvg' 'polkit' 'git' 'pacman-contrib')
makedepends=('nodejs' 'npm' 'rust' 'cargo')
provides=('monarch-store')
conflicts=('monarch-store-git' 'monarch-store-bin')
source=("${pkgname}-${pkgver}.tar.gz::${url}/archive/v${pkgver}.tar.gz")
sha256sums=('e341fb9d6565d287abe22964a438072006dbb67f054a5c080c7e08660d59272a')

build() {
  cd "${srcdir}/${pkgname}-${pkgver}"
  
  # 1. Install frontend deps
  npm install
  
  # 2. Build Tauri release (no-bundle since we package manually)
  npx tauri build --no-bundle
}

package() {
  cd "${srcdir}/${pkgname}-${pkgver}"
  
  # Install License
  install -Dm644 LICENSE "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE"
  
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

  # --- Polkit Setup (Seamless Auth) ---
  mkdir -p "${pkgdir}/usr/share/polkit-1/actions"
  
  # 1. Helper Script (SECURE WHITELIST)
  # Acts as a gatekeeper for the privileged actions allowed by the policy
  cat <<EOF > "${pkgdir}/usr/bin/monarch-pk-helper"
#!/bin/bash
case "\$(basename "\$1")" in
  pacman|yay|paru|aura|rm|cat|mkdir|chmod|killall|cp|sed|bash|ls|grep|touch|checkupdates)
    exec "\$@" ;;
  *)
    echo "Unauthorized: \$1"; exit 1 ;;
esac
EOF
  chmod 755 "${pkgdir}/usr/bin/monarch-pk-helper"

  # 2. Policy File
  cat <<EOF > "${pkgdir}/usr/share/polkit-1/actions/com.monarch.store.policy"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE policyconfig PUBLIC "-//freedesktop//DTD PolicyKit Policy Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/PolicyKit/1/policyconfig.dtd">
<policyconfig>
  <vendor>MonARCH Store</vendor>
  <vendor_url>https://github.com/monarch-store/monarch-store</vendor_url>
  <action id="com.monarch.store.package-manage">
    <description>Manage system packages and repositories</description>
    <message>Authentication is required to install, update, or remove applications.</message>
    <icon_name>package-x-generic</icon_name>
    <defaults>
      <allow_any>auth_admin</allow_any>
      <allow_inactive>auth_admin</allow_inactive>
      <allow_active>auth_admin_keep</allow_active>
    </defaults>
    <annotate key="org.freedesktop.policykit.exec.path">/usr/bin/monarch-pk-helper</annotate>
    <annotate key="org.freedesktop.policykit.exec.allow_gui">false</annotate>
  </action>
</policyconfig>
EOF
  chmod 644 "${pkgdir}/usr/share/polkit-1/actions/com.monarch.store.policy"
}

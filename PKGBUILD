# Maintainer: cpg716 (developer and creator; built with the help of AI coding tools)
# https://github.com/cpg716/monarch-store
pkgname=monarch-store
pkgver=0.3.5_alpha
pkgrel=1
# pkgdesc kept under 80 chars for terminal clarity
pkgdesc="Distro-aware software store for Arch, Manjaro, CachyOS (Tauri)"
arch=('x86_64')
url="https://github.com/cpg716/monarch-store"
license=('MIT')
depends=('webkit2gtk-4.1' 'gtk3' 'openssl' 'polkit' 'pacman-contrib' 'git')
# checkupdates is in pacman-contrib; rate-mirrors/reflector optional for Test Mirrors
optdepends=('rate-mirrors: Test Mirrors with latency (Settings → Repositories)'
            'reflector: alternative for Test Mirrors / mirror ranking')
makedepends=('cargo' 'nodejs' 'npm')
# For -git: SKIP. After pushing tag v${pkgver}, run: ./scripts/release-finalize-pkgbuild.sh
source=("git+https://github.com/cpg716/monarch-store.git")
sha256sums=('SKIP')

prepare() {
  cd "$pkgname"
  # Contain npm cache in $srcdir (Arch: no $HOME pollution)
  export npm_config_cache="$srcdir/.npm"
  # Reproducible install when package-lock.json exists
  npm ci
}

build() {
  cd "$pkgname"
  # Contain Cargo home in $srcdir (Arch: no $HOME pollution)
  export CARGO_HOME="$srcdir/.cargo"
  export npm_config_cache="$srcdir/.npm"
  # RELRO + noexecstack + PIE (workspace Cargo.toml already has release: lto=true, strip=true, panic=abort)
  export RUSTFLAGS="-C link-arg=-Wl,-z,relro,-z,now -C link-arg=-Wl,-z,noexecstack -C relocation-model=pie"
  # Helper built first to same target dir as tauri build (so package() finds it). Do not rely on .cargo/config target-dir.
  (cd src-tauri && CARGO_TARGET_DIR="$srcdir/$pkgname/src-tauri/target" cargo build --release -p monarch-helper)
  npm run tauri build
}

package() {
  cd "$pkgname"

  # 1. Install Binary (workspace build: binary is under monarch-gui or workspace target)
  _bin=src-tauri/target/release/monarch-store
  [ ! -f "$_bin" ] && _bin=src-tauri/monarch-gui/target/release/monarch-store
  install -Dm755 "$_bin" "$pkgdir/usr/bin/monarch-store"

  # 2. Install AppStream metainfo (required for software center integration)
  install -Dm644 "src-tauri/monarch-store.metainfo.xml" "$pkgdir/usr/share/metainfo/monarch-store.metainfo.xml"

  # 3. Install Desktop Entry (from source; Categories set for software center)
  install -Dm644 "src-tauri/monarch-store.desktop" "$pkgdir/usr/share/applications/monarch-store.desktop"
  sed -i 's/^Categories=.*/Categories=System;PackageManager;/' "$pkgdir/usr/share/applications/monarch-store.desktop"
  sed -i 's/^Comment=.*/Comment=Universal Arch Linux App Manager/' "$pkgdir/usr/share/applications/monarch-store.desktop"
  sed -i '/^StartupNotify=/d' "$pkgdir/usr/share/applications/monarch-store.desktop"
  echo 'StartupNotify=true' >> "$pkgdir/usr/share/applications/monarch-store.desktop"

  # 4. Install Icons (Tauri app icons live in src-tauri/monarch-gui/icons)
  install -Dm644 "src-tauri/monarch-gui/icons/128x128.png" "$pkgdir/usr/share/icons/hicolor/128x128/apps/monarch-store.png"
  install -Dm644 "src-tauri/monarch-gui/icons/32x32.png" "$pkgdir/usr/share/icons/hicolor/32x32/apps/monarch-store.png"
  install -Dm644 "src-tauri/monarch-gui/icons/icon.png" "$pkgdir/usr/share/icons/hicolor/512x512/apps/monarch-store.png"
  install -Dm644 "src-tauri/monarch-gui/icons/64x64.png" "$pkgdir/usr/share/icons/hicolor/64x64/apps/monarch-store.png"

  # 5. Install Polkit Actions & Rules
  install -Dm644 "src-tauri/monarch-gui/com.monarch.store.policy" "$pkgdir/usr/share/polkit-1/actions/com.monarch.store.policy"
  install -Dm644 "src-tauri/rules/10-monarch-store.rules" "$pkgdir/usr/share/polkit-1/rules.d/10-monarch-store.rules"

  # 6. Install Privileged Helper & Identity Wrapper Proxy (helper built explicitly in build() so it supports AlpmInstall)
  install -dm755 "$pkgdir/usr/lib/monarch-store"
  _helper=src-tauri/target/release/monarch-helper
  [ ! -f "$_helper" ] && _helper=src-tauri/monarch-gui/target/release/monarch-helper
  install -m755 "$_helper" "$pkgdir/usr/lib/monarch-store/monarch-helper"
  install -m755 "src-tauri/scripts/monarch-wrapper" "$pkgdir/usr/lib/monarch-store/monarch-wrapper"

  # 6.5 Optional: Pacman hook to notify MonARCH to refresh index after terminal pacman -Syu
  install -Dm644 "src-tauri/pacman-hooks/monarch-store-refresh.hook" "$pkgdir/usr/share/libalpm/hooks/monarch-store-refresh.hook"
  install -Dm755 "src-tauri/scripts/monarch-store-refresh-cache" "$pkgdir/usr/bin/monarch-store-refresh-cache"
  install -dm755 "$pkgdir/var/lib/monarch-store"

  # 7. License
  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}

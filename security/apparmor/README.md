# AppArmor profiles for MonARCH Store

**Last updated:** 2025-01-29 (v0.3.5-alpha.1)

Outline profiles for confining the GUI and (optionally) the privileged helper. **Review and test in complain mode before enforcing.**

## Files

| File | Confines | Purpose |
|------|----------|---------|
| `usr.bin.monarch-store` | `/usr/bin/monarch-store` (GUI) | Unprivileged Tauri app: config, cache, pkexec/sudo, network, AUR tools. |
| (future) `usr.lib.monarch-store.monarch-helper` | `/usr/lib/monarch-store/monarch-helper` | Root helper: pacman, ALPM, keyring; restrict to minimal paths. |

## Install (GUI profile)

```bash
sudo cp security/apparmor/usr.bin.monarch-store /etc/apparmor.d/
sudo apparmor_parser -r /etc/apparmor.d/usr.bin.monarch-store
```

Default is **complain** (`flags=(..., complain)`). To enforce:

```bash
# Edit profile: remove "complain" from flags
sudo apparmor_parser -r /etc/apparmor.d/usr.bin.monarch-store
```

## Remove

```bash
sudo apparmor_parser -R /etc/apparmor.d/usr.bin.monarch-store
sudo rm /etc/apparmor.d/usr.bin.monarch-store
```

## Notes

- **Arch Linux**: AppArmor is optional (`apparmor` package). If not used, these files are for reference only.
- **Helper profile**: A separate profile for `monarch-helper` would allow only pacman, keyring paths, and ALPM DBs; not included here to avoid blocking legitimate repair flows until tested.
- **Complain mode**: Use `aa-logprof` or `aa-genprof` to refine the profile from denials before enforcing.

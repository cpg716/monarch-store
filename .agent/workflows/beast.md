---
description: Full Development, Verification, and Documentation cycle for MonARCH Store.
---
# /beast

**Goal:** Execute a comprehensive Development, Verification, and Documentation cycle for the v0.4.6-alpha release.

## Phase 1: The Iron Audit (Dumb View Hygiene)
1. **Grep Check**: Search the `src/` directory for manual `invoke` calls to `get_metadata` or `get_reviews`. These must only exist in `bindings.ts` and be consumed via the `Package` object.
2. **Type Cleanup**: Ensure no local `interface Package` definitions exist in component files. All components must import `Package` from `@/services/bindings`.

## Phase 2: Backend & Specta Validation
3. **Rust Integrity**: Run `cd src-tauri && cargo check` to verify backend stability.
4. **Specta Sync**: Verify that `src/services/bindings.ts` is up to date and consistent with the Rust structs.

## Phase 3: Runtime Verification
5. **Launch**: run `npm run tauri dev` to start the MonARCH Store in development mode.
6. **Interaction**:
    - Perform a search for "Spotify" and verify it merges multiple sources into one card.
    - Open "Heroic Game Launcher" and verify the source selector dropdown is populated.
    - Confirm that icons and descriptions are present without any "pop-in" (Backend Hydration check).

## Phase 4: Final Verification
7. **Cargo Check**: Run `cargo check` in `src-tauri/monarch-gui` and `src-tauri/monarch-helper`.
8. **Manual Audit**: Inspect the Rust logs for any `[REGISTRY]` or `[ALPM]` errors during app startup.

## Phase 5: Documentation Sweep
9. **Release Notes**: Update `RELEASE_NOTES.md` and `docs/RECENT_CHANGES.md` if any new fixes are identified.
10. **Final Audit**: Confirm all modified files adhere to the **Iron Laws** in `.cursorrules`.

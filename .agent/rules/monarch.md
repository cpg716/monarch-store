# MonARCH Iron Laws
- **Contract-First:** The Rust backend is the single source of truth. Do not implement metadata parsing or icon guessing in the React frontend.
- **Stateless Frontend:** The UI is a "Dumb View." All components must rely on hydrated ViewModels provided by `bindings.ts`.
- **Iron Core:** All repository transactions MUST use `SafeUpdateTransaction`. Never propose `pacman -Sy` as a standalone command.
- **Anti-Browser Rule:** NEVER use the browser tool to verify Tauri IPC, Rust backend commands, or "Iron Core" logic. The browser cannot simulate the native environment. verification must be performed via `cargo check` or backend logs.

- **Registry Performance:** All bulk Registry operations MUST use:
    1. **Prepared Statements**: Initialize statements once per batch.
    2. **Chunked Transactions**: Maximum 500-1000 items per transaction to prevent UI freezes.
    3. **Immediate Behavior**: Use `TransactionBehavior::Immediate` for all write transactions to avoid lock failures.
    4. **Normal Sync**: Maintain `PRAGMA synchronous = NORMAL` for high-throughput writes.

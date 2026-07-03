# xenxen Build Errors Log

## Phase 1 Errors

### Error 1: Rust/Cargo not in PATH
**Date:** 2026-07-02
**Symptom:** `cargo: The term 'cargo' is not recognized`
**Root Cause:** Rust was installed via rustup but `.cargo/bin` was never created — symlinks weren't set up
**Fix:** Manually copied `cargo.exe`, `rustc.exe`, `rustdoc.exe` to `~/.cargo/bin/` and added to user PATH

### Error 2: MSVC link.exe not found
**Date:** 2026-07-02
**Symptom:** `error: linker 'link.exe' not found` when building
**Root Cause:** Visual Studio 2019 Build Tools installed but Windows SDK (Include/Lib dirs) missing. `vcvarsall.bat` ran but environment variables didn't propagate to cargo
**Fix:** Switched to `x86_64-pc-windows-gnu` target using TDM-GCC (already installed at `C:\TDM-GCC-64\bin`)

### Error 3: libgcc_eh missing (TDM-GCC)
**Date:** 2026-07-02
**Symptom:** `ld.exe: cannot find -lgcc_eh`
**Root Cause:** TDM-GCC 10.3.0 doesn't ship `libgcc_eh.a` — it's a known TDM-GCC limitation
**Fix:** Copied `libgcc.a` to `libgcc_eh.a` in `C:\TDM-GCC-64\lib\gcc\x86_64-w64-mingw32\10.3.0\`

### Error 4: Incomplete Rust installation
**Date:** 2026-07-02
**Symptom:** `rustup: The term 'rustup' is not recognized`
**Root Cause:** Only toolchain binaries existed, not rustup itself
**Fix:** Downloaded and ran `rustup-init.exe` to install rustup properly + GNU toolchain

### Error 5: clap `env` feature missing
**Date:** 2026-07-02
**Symptom:** `no method named 'env' found for struct 'Arg'`
**Root Cause:** `clap` derive needs the `env` feature enabled to use `env = "..."` in arg attributes
**Fix:** Added `"env"` to clap features: `clap = { version = "4", features = ["derive", "env"] }`

### Error 6: Incompatible match arm types
**Date:** 2026-07-02
**Symptom:** `match arms have incompatible types: Vec<DailyRow> vs Vec<ModelRow>`
**Root Cause:** Different DB query return types used in same match expression
**Fix:** Created unified `BreakdownRow` struct with conversion methods from both types

### Error 7: Config commands required database
**Date:** 2026-07-02
**Symptom:** `xenxen config show` fails with "OpenCode database not found"
**Root Cause:** Database connection was opened before match statement, required for all commands
**Fix:** Restructured `main()` to only open DB connection for commands that need it (dashboard, stats)

### Error 8: Database path detection wrong on Windows
**Date:** 2026-07-02
**Symptom:** DB not found even though `opencode.db` exists
**Root Cause:** `dirs::data_local_dir()` returns `%LOCALAPPDATA%` on Windows, but OpenCode uses `~/.local/share/opencode/` (xdg-style path)
**Fix:** Added `dirs::home_dir().join(".local/share/opencode/opencode.db")` to the search path

## Environment Notes
- **OS:** Windows (win32)
- **Rust:** 1.96.1 (stable-x86_64-pc-windows-gnu)
- **Linker:** TDM-GCC 10.3.0 (x86_64-w64-mingw32-gcc)
- **OpenCode DB:** `~/.local/share/opencode/opencode.db`
- **Config dir:** `%APPDATA%\xenxen\config.toml` (via `dirs::config_dir()`)

## Phase 2 Errors

### Error 9: Variable `headers` not in scope
**Date:** 2026-07-02
**Symptom:** `cannot find value 'headers' in this scope`
**Root Cause:** Match arms returned `(headers, rows, widths)` but destructured to `(header_cells, rows, widths)`, then referenced `headers` on line 306
**Fix:** Changed `Row::new(headers)` to `Row::new(header_cells)`

### Error 10: Double-borrow on iterator
**Date:** 2026-07-02
**Symptom:** `&std::iter::Take<...> is not an iterator`
**Root Cause:** `&stats.top_tools.iter().take(10)` — the `&` borrows the iterator making it non-Iterator
**Fix:** Removed leading `&`: `stats.top_tools.iter().take(10)`

### Error 11: Missing `session_count` and `total_cost` after refactor
**Date:** 2026-07-02
**Symptom:** After rewriting db.rs, main.rs still called old standalone functions
**Root Cause:** Phase 2 rewrote db.rs with new `AggregateStats` struct, old functions removed
**Fix:** Updated main.rs stats command to use `db::aggregate_stats()` instead

## Phase 3 Errors

### Error 12: Unused method warning after refactor
**Date:** 2026-07-02
**Symptom:** `method 'remaining_balance' is never used`
**Root Cause:** Phase 3 BalanceTracker replaced direct Config.remaining_balance() calls
**Fix:** Warning is harmless — method kept for backward compatibility and tests. Could suppress with `#[allow(dead_code)]` or remove in future cleanup.

## Phase 4 Errors

### Error 13: Missing `by_project` field in test helpers
**Date:** 2026-07-02
**Symptom:** `missing field 'by_project' in initializer of db::AggregateStats`
**Root Cause:** Phase 4 added `by_project` to AggregateStats but test helper in balance.rs wasn't updated
**Fix:** Added `by_project: vec![]` to the test_stats() helper function

### Error 14: Project names showing as hash IDs
**Date:** 2026-07-02
**Symptom:** Project breakdown shows truncated hash strings like `e0f6bcf8cad88bd7d35b445a50eda…`
**Root Cause:** OpenCode stores project_id as a hash, and the `project.name` column is NULL for most entries
**Fix:** Known data limitation — the `COALESCE(p.name, project_id)` fallback works correctly. Could resolve to directory path in future by querying the `project` table's `worktree` column.

## Phase 5 Errors

### Error 15: Match arm type mismatch in sort column names
**Date:** 2026-07-02
**Symptom:** `match arms have incompatible types` — tab 4 tools array had 2 elements while others had 5
**Root Cause:** Different tabs have different column counts, but the match arms must all return the same array size
**Fix:** Padded the tools array to 5 elements with empty strings: `["tool", "count", "", "", ""]`

### Error 16: Lifetime error from sorting local clone
**Date:** 2026-07-02
**Symptom:** `cannot return value referencing local variable 'data'` — rows borrowed from cloned data couldn't be returned
**Root Cause:** Cloning data into a local variable and then borrowing from it in Row cells created rows with a shorter lifetime than the function return
**Fix:** Moved sorting to `render_breakdown` (which takes `&mut App`) to sort in-place on `app.stats` before calling the builders, which just read from the already-sorted data

## Phase 6 — Cross-Platform Notes

### Build Environment
- **Platform:** Windows 11 (win32)
- **Rust toolchain:** 1.96.1 stable-x86_64-pc-windows-gnu
- **C compiler:** TDM-GCC 10.3.0 (at `C:\TDM-GCC-64\bin`)
- **Target:** x86_64-pc-windows-gnu (not MSVC — Windows SDK was incomplete)

### Known Windows Issues
1. MSVC Build Tools may be missing `Windows SDK/UCRT` — use GNU toolchain instead
2. `libgcc.a` must be copied to `libgcc_eh.a` for the linker to find exception handling symbols
3. `dirs::data_local_dir()` returns `C:\Users\<user>\AppData\Local` but OpenCode uses `~/.local/share/` (xdg-style) — manual path probing required
4. `libsqlite3` bundled feature compiles from source via TDM-GCC — no system SQLite needed

### Tested On
- Windows 11 x86_64 — fully functional

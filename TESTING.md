# Testing Guide for MonARCH Store

**Release:** v0.3.5-alpha

## ⚠️ Important: What Tests Actually Prove

### ✅ Unit Tests (What We Have)
These tests verify **command serialization/parsing**:
- Commands serialize to valid JSON ✅
- Raw strings are rejected ✅  
- JSON format matches helper expectations ✅
- File/env var formats are correct ✅

**These catch the "raw string bug" but DO NOT test actual installs/updates.**

### ❌ What's Missing (Full Integration Tests)
To **actually prove** install/update works, we need:
- Real helper execution with root privileges
- Actual pacman/ALPM transactions
- Real package database sync
- End-to-end GUI → Helper → pacman flow
- Error handling in real scenarios

## Quick Test Commands

### Test Command Serialization (No Build Required)
```bash
# Test helper command parsing (catches JSON bugs immediately)
cd src-tauri/monarch-helper
cargo test command_tests

# Test GUI command serialization
cd src-tauri/monarch-gui
cargo test helper_client::tests
```

### Test Actual Helper Execution (Requires Root)
```bash
cd src-tauri/monarch-helper
./tests/test_install_flow.sh
```

### Run All Tests
```bash
cd src-tauri
cargo test
```

## What These Tests Catch

### ✅ Command Serialization Tests
- **Prevents**: Raw strings like "cachyos" being sent instead of JSON
- **Verifies**: All HelperCommand variants serialize/deserialize correctly
- **Checks**: File and env var formats match helper expectations

### ✅ Integration Tests (Parsing Only)
- **Location**: `src-tauri/monarch-helper/tests/integration_test.rs`
- **Tests**: Full command roundtrip (GUI → JSON → Helper parsing)
- **No Root Required**: Tests parsing logic without needing sudo
- **Limitation**: Does NOT test actual package installation

## Before Every Commit

Run these tests to catch **serialization bugs** before building:

```bash
# 1. Test helper can parse commands
cd src-tauri/monarch-helper && cargo test

# 2. Test GUI serializes correctly  
cd ../monarch-gui && cargo test helper_client::tests

# 3. Run integration tests (parsing only)
cd .. && cargo test --test integration_test
```

## To Actually Test Install/Update

You still need to:
1. **Build the app**: `npm run tauri build` or `npm run tauri dev`
2. **Run it manually**: Test install/update in the GUI
3. **Check logs**: Look for errors in helper output

The unit tests catch **serialization bugs** (like the "cachyos" bug), but don't replace manual testing of the full feature.

## Common Issues These Tests Prevent

1. **Raw String Bug**: Tests reject "cachyos" as a command ✅
2. **JSON Format**: Ensures all commands are valid JSON objects ✅
3. **Repo Names**: Verifies repo names serialize correctly in JSON structure ✅
4. **Env Var Format**: Tests MONARCH_CMD_JSON format matches helper expectations ✅

## Adding New Tests

When adding new HelperCommand variants:
1. Add serialization test in `command_tests` module
2. Add integration test in `tests/integration_test.rs`
3. Verify it works with both file and env var passing
4. **Still need to manually test** actual execution

## Debugging Failed Tests

If tests fail:
1. Check the JSON output - is it valid JSON?
2. Does it start with `{` and end with `}`?
3. Is it a raw string or a proper JSON object?
4. Compare with working command examples in tests

## Summary

**Unit tests = Serialization/parsing verification** ✅  
**Full testing = Still requires manual GUI testing** ⚠️

The tests are a **safety net** that catches bugs early, but they don't replace testing the actual feature in the app.

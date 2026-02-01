# ErrorService - Centralized Error Handling

**Last updated:** 2026-02-01 (v0.3.6-alpha)

## Overview

The `ErrorService` provides a unified API for handling and displaying errors throughout the MonARCH Store application. It replaces scattered error handling patterns with a single, consistent system.

**Wired app-wide (2025-01-31):** `getErrorService()` is available for use outside the React tree (Zustand store, hooks, `main.tsx`). All critical paths use `reportError`/`reportWarning`/`reportCritical` instead of `console.error`: App, SettingsPage, OnboardingModal, SystemHealthSection, InstallMonitor, CategoryView, PackageDetailsFresh, ErrorModal, internal_store, useSettings, RepoStatusContext, and `window.onerror` in main.

## Features

- **Unified API**: Single interface for all error reporting
- **Severity Levels**: `info`, `warning`, `error`, `critical`
- **Automatic Routing**: Critical errors show modals, others show toasts
- **Backend Integration**: Handles `ClassifiedError` from Rust backend
- **Recovery Actions**: Supports recovery actions with custom handlers
- **Error History**: Maintains last 50 errors for debugging
- **Backward Compatible**: Works alongside existing `ToastContext`

## Usage

### Basic Usage

```tsx
import { useErrorService } from '../context/ErrorContext';

function MyComponent() {
    const errorService = useErrorService();

    const handleAction = async () => {
        try {
            await someOperation();
        } catch (e) {
            // Simple error (shows toast)
            errorService.reportError(e);
        }
    };

    return <button onClick={handleAction}>Do Something</button>;
}
```

### Severity Levels

```tsx
// Info - shows info toast
errorService.reportInfo('Operation completed successfully');

// Warning - shows warning toast
errorService.reportWarning('This action may take a while');

// Error - shows error toast
errorService.reportError('Failed to load data');

// Critical - shows modal dialog
errorService.reportCritical('System configuration failed');
```

### With Recovery Actions

```tsx
errorService.reportCritical(
    'Database is locked',
    {
        type: 'unlock_database',
        label: 'Unlock & Retry',
        handler: async () => {
            await invoke('repair_unlock_pacman', { password: null });
            // Retry the operation
            await retryOperation();
        }
    }
);
```

### v0.3.6 Iron Core Synergy
The `ErrorService` is now tightly integrated with `SafeUpdateTransaction`. When a transaction is aborted due to a pre-existing lock, the backend emits a `ClassifiedError` which `ErrorService` captures to show the specialized "Package Manager Busy" recovery modal.

### Backend ClassifiedError

```tsx
// Listen for classified errors from backend
useEffect(() => {
    const unlisten = listen('install-error-classified', (event) => {
        const classifiedError = event.payload as ClassifiedError;
        
        // ErrorService automatically handles ClassifiedError
        errorService.reportCritical(classifiedError, {
            type: classifiedError.recovery_action?.type || 'retry',
            label: 'Retry',
            handler: async () => {
                // Recovery logic
            }
        });
    });
    
    return () => { unlisten.then(f => f()); };
}, []);
```

### String Errors

```tsx
// String errors are automatically converted via friendlyError()
errorService.reportError('unable to lock database');
// Shows: "Package Manager Busy - Another package manager is running..."
```

### Error Objects

```tsx
try {
    throw new Error('Something went wrong');
} catch (e) {
    errorService.reportError(e);
    // Automatically converts Error to FriendlyError
}
```

## Migration Guide

### From `useToast().error()`

**Before:**
```tsx
const { error } = useToast();
error('Failed to install package');
```

**After:**
```tsx
const errorService = useErrorService();
errorService.reportError('Failed to install package');
```

### From `friendlyError()` + Toast

**Before:**
```tsx
const { error } = useToast();
const friendly = friendlyError(rawError);
error(friendly.description);
```

**After:**
```tsx
const errorService = useErrorService();
errorService.reportError(rawError); // Automatically uses friendlyError()
```

### From Inline Error Display

**Before:**
```tsx
const [error, setError] = useState<string | null>(null);
// ... display error in UI
```

**After:**
```tsx
const errorService = useErrorService();
errorService.reportError(errorMessage); // Handles display automatically
```

## API Reference

### `useErrorService()`

Returns the error service context with the following methods:

#### `report(error, severity?, recoveryAction?)`
Main reporting function. Automatically routes based on severity.

#### `reportCritical(error, recoveryAction?)`
Shorthand for `report(error, 'critical', recoveryAction)`.

#### `reportError(error)`
Shorthand for `report(error, 'error')`. Shows error toast.

#### `reportWarning(error)`
Shorthand for `report(error, 'warning')`. Shows warning toast.

#### `reportInfo(message)`
Shorthand for `report(message, 'info')`. Shows info toast.

#### `currentCriticalError`
Current critical error (for ErrorModal). Usually not needed.

#### `dismissCritical()`
Dismiss current critical error modal. Usually not needed.

## Error Types

### ErrorInput
Can be one of:
- `string` - Raw error message (converted via `friendlyError()`)
- `Error` - JavaScript Error object
- `ClassifiedError` - Backend classified error from Rust
- `FriendlyError` - Frontend friendly error

### ErrorSeverity
- `'info'` - Informational message (toast)
- `'warning'` - Warning message (toast)
- `'error'` - Error message (toast)
- `'critical'` - Critical error (modal)

### ClassifiedError
Matches Rust `error_classifier.rs`:
```typescript
interface ClassifiedError {
    kind: string;
    title: string;
    description: string;
    recovery_action?: {
        type: string;
        payload?: string;
    };
    raw_message: string;
}
```

### friendlyError() (Frontend)
`src/utils/friendlyError.ts` mirrors backend classification for string errors. It maps raw messages (including install/AUR failures) to `FriendlyError` (title, description, recoveryAction). Supported patterns include:
- Database lock, keyring, package not found, mirror failure, disk full, dependency/file conflict
- **AUR build "unknown error"**: `unknown error has occurred`, `makepkg reported an unknown error`, or `permission sanitizer` → title "AUR Build Failed (Unknown Error)", recovery label "Run Permission Sanitizer"

When the backend returns a string (e.g. from `build_aur_package_single`), `reportError(raw)` runs it through `friendlyError()`, so the user sees the mapped title/description and recovery hint.

## Architecture

```
ErrorService (Context)
├── ErrorProvider (wraps app)
│   ├── Uses ToastContext for non-critical errors
│   ├── Manages critical error state
│   └── Maintains error history
├── ErrorModal (Component)
│   ├── Displays critical errors
│   ├── Shows recovery actions
│   └── Handles technical details
└── ErrorBoundary (Integration)
    └── Reports React errors to ErrorService
```

## Best Practices

1. **Use appropriate severity**: Don't use `critical` for simple errors
2. **Provide recovery actions**: When possible, offer one-click recovery
3. **Use backend classified errors**: Prefer `ClassifiedError` over raw strings
4. **Gradual migration**: Migrate components one at a time
5. **Keep ToastContext**: Still works for simple success messages

## Future Enhancements

- Error logging to file
- Error telemetry integration
- Error history UI
- Error reporting to backend
- Error analytics dashboard

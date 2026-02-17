import { Result } from '../services/bindings';

/**
 * Unwraps a Specta Result object.
 * If status is "ok", returns data.
 * If status is "error", throws the error (as an Error object).
 */
export function unwrap<T, E>(result: Result<T, E>): T {
    if (result.status === "ok") {
        return result.data;
    }
    // Convert error to string or Error object
    throw new Error(String(result.error));
}

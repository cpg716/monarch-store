use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MockCommand {
    pub binary: String,
    pub args: Vec<String>,
}

#[derive(Clone)]
pub struct MockPackageManager {
    // Log of executed commands for assertion
    pub command_history: Arc<Mutex<Vec<MockCommand>>>,
    // Configurable responses
    pub should_fail_next: Arc<Mutex<bool>>,
    pub next_error_message: Arc<Mutex<String>>,
}

impl MockPackageManager {
    pub fn new() -> Self {
        Self {
            command_history: Arc::new(Mutex::new(Vec::new())),
            should_fail_next: Arc::new(Mutex::new(false)),
            next_error_message: Arc::new(Mutex::new("Mock Failure".to_string())),
        }
    }

    pub fn reset(&self) {
        if let Ok(mut hist) = self.command_history.lock() {
            hist.clear();
        }
        if let Ok(mut fail) = self.should_fail_next.lock() {
            *fail = false;
        }
    }

    pub fn set_failure(&self, msg: &str) {
        if let Ok(mut fail) = self.should_fail_next.lock() {
            *fail = true;
        }
        if let Ok(mut err) = self.next_error_message.lock() {
            *err = msg.to_string();
        }
    }

    // Mock Execution Logic
    pub fn execute(&self, binary: &str, args: &[&str]) -> Result<String, String> {
        // 1. Record Command
        if let Ok(mut hist) = self.command_history.lock() {
            hist.push(MockCommand {
                binary: binary.to_string(),
                args: args.iter().map(|s| s.to_string()).collect(),
            });
        }

        // 2. Check for Forced Failure
        if let Ok(fail) = self.should_fail_next.lock() {
            if *fail {
                let msg = self.next_error_message.lock().unwrap().clone();
                return Err(msg);
            }
        }

        // 3. Return Success Stub
        Ok(format!("Mock success: {} {:?}", binary, args))
    }
}

// Tests for the Mock itself
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_recording() {
        let mock = MockPackageManager::new();
        let _ = mock.execute("pacman", &["-S", "firefox"]);

        let history = mock.command_history.lock().unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].binary, "pacman");
        assert_eq!(history[0].args[1], "firefox");
    }

    #[test]
    fn test_failure_injection() {
        let mock = MockPackageManager::new();
        mock.set_failure("Disk Full");

        let result = mock.execute("pacman", &["-S", "big-package"]);
        assert!(result.is_err());
        assert_eq!(result.err(), Some("Disk Full".to_string()));
    }
}

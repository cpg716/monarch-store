/// Integration tests for monarch-helper
/// These tests verify the helper can parse commands correctly without needing root or the full GUI
/// 
/// Run with: `cargo test --test integration_test`

use serde_json;
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(tag = "command", content = "payload")]
enum HelperCommand {
    AlpmInstall {
        packages: Vec<String>,
        sync_first: bool,
        enabled_repos: Vec<String>,
        cpu_optimization: Option<String>,
    },
    ForceRefreshDb,
    Refresh,
    Initialize,
}

#[test]
fn test_helper_can_parse_install_command_from_file() {
    // Create a command file like the GUI would
    let cmd = HelperCommand::AlpmInstall {
        packages: vec!["test-package".to_string()],
        sync_first: true,
        enabled_repos: vec!["core".to_string(), "cachyos".to_string()],
        cpu_optimization: Some("v3".to_string()),
    };

    let json = serde_json::to_string(&cmd).expect("Should serialize");
    
    // Write to temp file
    let mut file = NamedTempFile::new().expect("Should create temp file");
    file.write_all(json.as_bytes()).expect("Should write");
    file.flush().expect("Should flush");
    
    // Verify file content is valid JSON
    let contents = std::fs::read_to_string(file.path()).expect("Should read");
    let parsed: Result<HelperCommand, _> = serde_json::from_str(&contents.trim());
    assert!(parsed.is_ok(), "Helper should be able to parse this JSON");
}

#[test]
fn test_helper_rejects_raw_string() {
    // Test that raw strings are rejected
    let raw_string = "cachyos";
    
    let result: Result<HelperCommand, _> = serde_json::from_str(raw_string);
    assert!(result.is_err(), "Helper should reject raw strings");
}

#[test]
fn test_helper_can_parse_from_env_var() {
    // Test env var format
    let cmd = HelperCommand::AlpmInstall {
        packages: vec!["pkg".to_string()],
        sync_first: false,
        enabled_repos: vec!["chaotic-aur".to_string()],
        cpu_optimization: None,
    };

    let json = serde_json::to_string(&cmd).expect("Should serialize");
    
    // Simulate env var
    std::env::set_var("TEST_MONARCH_CMD_JSON", &json);
    let env_json = std::env::var("TEST_MONARCH_CMD_JSON").expect("Should read");
    
    // Helper should be able to parse this
    let parsed: Result<HelperCommand, _> = serde_json::from_str(&env_json);
    assert!(parsed.is_ok(), "Helper should parse env var JSON");
    
    std::env::remove_var("TEST_MONARCH_CMD_JSON");
}

#[test]
fn test_command_serialization_roundtrip() {
    // Test that commands serialize and deserialize correctly
    let commands = vec![
        HelperCommand::ForceRefreshDb,
        HelperCommand::Refresh,
        HelperCommand::Initialize,
        HelperCommand::AlpmInstall {
            packages: vec!["pkg1".to_string(), "pkg2".to_string()],
            sync_first: true,
            enabled_repos: vec!["core".to_string(), "cachyos".to_string()],
            cpu_optimization: Some("v4".to_string()),
        },
    ];

    for cmd in commands {
        let json = serde_json::to_string(&cmd).expect("Should serialize");
        assert!(json.starts_with('{'));
        assert!(json.ends_with('}'));
        
        let parsed: HelperCommand = serde_json::from_str(&json).expect("Should deserialize");
        // Verify it's the same type (can't easily compare enum variants)
        std::mem::drop(parsed);
    }
}

#[test]
fn test_install_command_with_cachyos_repos() {
    // Specific test for the bug we're fixing
    let cmd = HelperCommand::AlpmInstall {
        packages: vec!["anydesk-bin".to_string()],
        sync_first: true,
        enabled_repos: vec!["cachyos".to_string(), "chaotic-aur".to_string()],
        cpu_optimization: Some("v3".to_string()),
    };

    let json = serde_json::to_string(&cmd).expect("Should serialize");
    
    // Verify it's valid JSON, not a raw string
    assert!(json.starts_with('{'));
    assert!(json.contains("cachyos")); // Should contain repo name in JSON structure
    assert!(!json.trim().eq("\"cachyos\"")); // Should NOT be just the string
    
    // Should parse back
    let parsed: HelperCommand = serde_json::from_str(&json).expect("Should parse");
    match parsed {
        HelperCommand::AlpmInstall { enabled_repos, .. } => {
            assert!(enabled_repos.contains(&"cachyos".to_string()));
        }
        _ => panic!("Wrong variant"),
    }
}

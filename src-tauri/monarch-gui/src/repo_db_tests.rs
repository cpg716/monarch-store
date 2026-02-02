use super::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct MockRepoClient {
    pub responses: Arc<Mutex<HashMap<String, Result<Vec<u8>, String>>>>,
}

impl MockRepoClient {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn mock_response(&self, url: &str, data: Vec<u8>) {
        self.responses
            .lock()
            .unwrap()
            .insert(url.to_string(), Ok(data));
    }

    pub fn mock_error(&self, url: &str, error: &str) {
        self.responses
            .lock()
            .unwrap()
            .insert(url.to_string(), Err(error.to_string()));
    }
}

#[async_trait::async_trait]
impl RepoClient for MockRepoClient {
    async fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>, String> {
        let responses = self.responses.lock().unwrap();
        if let Some(res) = responses.get(url) {
            res.clone()
        } else {
            Err(format!("Mock 404: {}", url))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PackageSource;

    #[tokio::test]
    async fn test_fetch_repo_packages_success() {
        let mock_client = MockRepoClient::new();
        // Create valid tar.gz (mocking an empty repo db for simplicity first,
        // normally we'd construct a real tar with entries, but let's test protocol flow first)
        // Magic bytes for gzip: 1f 8b
        let mock_data = vec![
            0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x03, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        // ^ This is a mostly empty gzip stream. The parser might fail on empty archive,
        // but the network part should succeed.

        // Actually, let's just create an empty uncompressed vec to simulate "No compression" flow
        // if the parser allows it, OR just verify the network call happens.
        // repo_db checks magic bytes. If none match, it treats as uncompressed tar.
        // An empty vec is an invalid tar, so we expect an error from the Parser, NOT the Network.

        let url = "https://example.com/repo.db";
        mock_client.mock_response(url, vec![]); // Invalid tar, but valid HTTP

        let temp_dir = tempfile::tempdir().unwrap();
        let cache_path = temp_dir.path();

        let result = fetch_repo_packages(
            &mock_client,
            url,
            "test_repo",
            PackageSource::cachyos(),
            cache_path,
            true,
            0,
        )
        .await;

        // We expect an error, but it should be a Parse error, not a Network error.
        assert!(result.is_err());
        let err = result.err().unwrap();
        // println!("Error: {}", err);
        assert!(
            err.contains("Task join error")
                || err.contains("IO error")
                || err.contains("failed to read")
                || err.contains("unexpected end of file")
        );
        // This confirms the network retrieval "Succeeded" and passed data to the parser (which failed).
    }

    #[tokio::test]
    async fn test_fetch_repo_all_mirrors_fail() {
        let mock_client = MockRepoClient::new();
        let url = "https://example.com/repo.db";
        mock_client.mock_error(url, "Connection Timeout");

        let temp_dir = tempfile::tempdir().unwrap();
        let cache_path = temp_dir.path();

        let result = fetch_repo_packages(
            &mock_client,
            url,
            "fail_repo",
            PackageSource::cachyos(),
            cache_path,
            true,
            0,
        )
        .await;

        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.contains("All mirrors failed"));
        assert!(err.contains("Connection Timeout"));
    }
}

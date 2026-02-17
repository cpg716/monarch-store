use crate::flathub_api::FlathubApiClient;
use tokio::runtime::Runtime;

#[tokio::test]
async fn test_heroic_mapping() {
    let client = FlathubApiClient::new();

    let meta = client
        .get_metadata_for_package("heroic-games-launcher-bin")
        .await;

    let m = meta.expect("Mapping failed for heroic-games-launcher-bin");
    assert!(m.icon.is_some(), "Icon should be present");
    assert!(!m.screenshots.is_empty(), "Screenshots should be present");
}

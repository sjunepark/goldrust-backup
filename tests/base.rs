//! This example demonstrates how to use Goldrust to test a simple GET request.
//! The⭐️s indicate where manual implementation is required.

use goldrust::{Goldrust, ResponseSource};
use std::path::Path;
use tracing_subscriber::EnvFilter;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn base() {
    tracing_subscriber::fmt::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .pretty()
        .init();

    let goldrust = Goldrust::default();

    let mock_server = MockServer::start().await;

    // The domain will be used to create future requests
    let mut domain: Option<String> = None;
    let url_path = "/api/actual";

    // ⭐️ Implement the corresponding logic for each case
    match goldrust.response_source {
        ResponseSource::Local => {
            tracing::info!("Running branch ResponseSource::Local");

            // ⭐️ Set the domain to the mock server uri
            domain = Some(mock_server.uri());

            // ⭐️ Configure the mock server to return a local response
            Mock::given(method("GET"))
                .and(path(url_path))
                .respond_with(create_response_template(&goldrust.golden_file_path))
                .mount(&mock_server)
                .await;
        }
        ResponseSource::External => {
            tracing::info!("Running branch ResponseSource::External");

            // ⭐️ Set the domain to the mock server uri
            domain = Some("https://some-external-api.com".to_string());
        }
    }

    let response = reqwest::Client::new()
        .get(&format!("{}{}", domain.expect("domain not set"), url_path))
        .send()
        .await
        .expect("Failed to send request");
    tracing::debug!(response = ?response, "Got response");

    #[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq)]
    struct Data {
        name: String,
        age: u16,
    }
    // The response body
    let response_body: Data = response.json().await.expect("Failed to get bytes");
    let golden_file_text =
        std::fs::read_to_string(&goldrust.golden_file_path).expect("Failed to read golden file");

    // The response body should match the golden file
    assert_eq!(
        serde_json::to_string(&response_body).expect("Failed to serialize"),
        golden_file_text.replace(" ", "").replace("\n", "")
    );

    // ⭐️ Using the closure to save content to the golden file
    goldrust.save(response_body).expect("Failed to save");
}

#[tracing::instrument]
fn create_response_template<P: AsRef<Path> + std::fmt::Debug>(path: P) -> ResponseTemplate {
    let path = path.as_ref();
    let body = std::fs::read_to_string(path).expect("Failed to read file");
    ResponseTemplate::new(200).set_body_string(body)
}

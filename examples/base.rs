//! This example demonstrates how to use Goldrust to test a simple GET request.
//! In similar scenarios, you will have to consider the following:
//! - A mock server should intercept requests: You can make your request function accept a domain parameter.
//!   This parameter can be set to the mock server uri when running tests.
//! - The mock server should serve the golden file for mock responses.
//! - When a golden file does not exist, or an update is required (via the `GOLDRUST_UPDATE_GOLDEN_FILES` env var),
//!   an external api request has to be made, and the response body should be saved to the golden file.

#[tokio::main]
async fn main() {
    println!("Running main");
}

#[cfg(test)]
mod tests {
    use goldrust::{get_test_id, Goldrust};
    use std::path::Path;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tracing_subscriber::EnvFilter;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn base() {
        tracing_subscriber::fmt::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .init();

        let test_id = get_test_id();
        tracing::debug!(test_id = ?test_id, "Running test");

        let golden_file_path = format!("examples/{}.json", test_id);
        let goldrust = Goldrust::new(golden_file_path.clone());
        let mock_server = MockServer::start().await;

        // The domain will be used to create future requests
        let domain = Arc::new(Mutex::new("".to_string()));
        let url_path = format!("/api/{}", test_id);

        // region: Closures to run depending on the configuration
        let run_when_local = || async {
            tracing::info!("Running run_when_local");

            // ⭐️ Set the domain to the mock server uri
            let mut guard = domain.lock().await;
            *guard = mock_server.uri();

            // ⭐️ Configure the mock server to return a local response
            Mock::given(method("GET"))
                .and(path(url_path.clone()))
                .respond_with(create_response_template(golden_file_path.clone()))
                .mount(&mock_server)
                .await;
        };

        let run_when_external = || async {
            tracing::info!("Running run_when_local");

            // ⭐️ Set the domain to the mock server uri
            let mut guard = domain.lock().await;
            *guard = "https://some-external-api.com".to_string();
        };
        // endregion

        // ⭐ The returned closure should always be used to save content to the golden file
        let save = goldrust.run(run_when_local, run_when_external).await;

        let response = reqwest::Client::new()
            .get(&format!("{}{}", domain.lock().await, url_path))
            .send()
            .await
            .expect("Failed to send request");
        tracing::debug!(response = ?response, "Got response");

        let bytes = response.bytes().await.expect("Failed to get bytes");

        // The response body should match the golden file
        assert_eq!(
            String::from_utf8(bytes.to_vec()).expect("Failed to convert bytes to string"),
            "{\n  \"name\": \"June\",\n  \"age\": 1\n}"
        );

        // ⭐️ Using the closure to save content to the golden file
        save(&bytes).expect("Failed to save bytes");
    }

    #[tracing::instrument]
    fn create_response_template<P: AsRef<Path> + std::fmt::Debug>(path: P) -> ResponseTemplate {
        let path = path.as_ref();
        tracing::debug!(path = ?path);
        let abs_path = path.canonicalize().expect("Failed to canonicalize path");
        tracing::debug!(abs_path = ?abs_path);
        let body = std::fs::read_to_string(path).expect("Failed to read file");
        ResponseTemplate::new(200).set_body_string(body)
    }
}

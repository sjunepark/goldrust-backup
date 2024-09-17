//! # Introduction
//!
//! A minimal golden file testing library for Rust,
//! when golden files are required for external api requests.
//!
//! # Warning & Disclaimer
//! Currently intended for _personal use_, and has the following limitations:
//! - Be aware that the design is very messy and not scalable.
//! - This is just the initial published version, and the API is not stable.
//!
//! # When to use
//!
//! This crate is intended to be used in the situation below:
//!
//! - You're sending external api requests in your library/binary.
//! - You want to mock the responses for testing purposes.
//! - You want to create the mocks based on the actual responses.
//!     - As so, you want to make actual api requests,
//!     - then save these to golden files mocking.
//!
//! The logic above ensures that your mocks are based on actual external api responses
//! but also allows testing
//! 1. in constrained environments when you can't make actual external requests,
//! 2. when the external api server is unavailable.
//!
//! # Testing logic
//!
//! In the case which fits the [When to use](#when-to-use) section, you can use the following logic:
//!
//! 1. Create a mock server which intercepts requests:
//!    - Your function, which sends external api requests, should accept a domain parameter,
//!      so it can be injected as a dependency.
//!      You can set this domain as:
//!      - In production: The actual domain
//!      - In tests: The mock server uri
//! 2. Serve the mock response:
//!    - When a golden file does not exist
//!       (or an update is required via the `GOLDRUST_UPDATE_GOLDEN_FILES` env var):
//!       1. Create an external api request
//!       2. Save the response body to the golden file
//!    - When a golden file exists and no update is required,
//!       serve the golden file for mock responses
//!
//! # Async
//!
//! Only supports async, as was intended to be used in relevance to http request mocking.
//!
//! # Usage
//!
//! Check the examples folder for a full example.
//!
//! ## Requirements
//!
//! Related environment variables should be set:
//!
//! - `GOLDRUST_DIR`: `String`
//!   - The directory where the golden files will be saved.
//!   - Defaults to `tests/golden`.
//! - `GOLDRUST_ALLOW_EXTERNAL_API_CALL`: `bool`
//!   - Whether external api calls are allowed.
//!   - Defaults to `true`.
//! - `GOLDRUST_UPDATE_GOLDEN_FILES`: `bool`
//!   - Whether golden files should be updated.
//!   - Defaults to `true`.
//!
//! Some combinations are invariant and will panic:
//! (for example, you can't update golden files without allowing external api calls).
//!
//!
//! # Current Limitations
//!
//! - Content that is to be created as golden files should be JSON serializable, deserializable.
//!   (This is because the golden files are saved as JSON files)
//! - Assumes that only a single golden file is required per test.
//!   (The current implementation creates golden file names based on the thread name of the test)
//!   If multiple golden files are required, it is recommended to break down the test
//!   in the current implementation.
//!   (Having to pass down the golden file name
//!   and track each seemed like an unnecessary complexity for now)
//!
use std::fs::OpenOptions;
use std::io::Error;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Goldrust {
    allow_external_api_call: bool,
    update_golden_files: bool,
    /// The path to the golden file,
    /// which was automatically generated based on the thread name of the test
    pub golden_file_path: PathBuf,
    pub response_source: ResponseSource,
    save_check: bool,
}

impl Default for Goldrust {
    #[tracing::instrument]
    /// Create a new instance of GoldrustBuilder
    ///
    /// A new instance of Goldrust should be created for each test.
    ///
    /// Golden file names are based on the thread name of the test.
    /// (e.g. `test::test_name` → `test-test_name.json`)
    fn default() -> Self {
        let golden_file_dir = std::env::var("GOLDRUST_DIR").unwrap_or("tests/golden".to_string());
        tracing::trace!(?golden_file_dir);
        let test_id = std::thread::current()
            .name()
            .expect("Thread should have a name. Threads don't have names by default when they are created with `thread::Builder::spawn`")
            .split("::")
            .collect::<Vec<_>>()
            .join("-")
            .to_string();
        let golden_file_path = Path::new(&golden_file_dir).join(format!("{}.json", test_id));
        tracing::trace!(?golden_file_path);

        let allow_external_api_call: bool = std::env::var("GOLDRUST_ALLOW_EXTERNAL_API_CALL")
            .unwrap_or("true".to_string())
            .parse()
            .expect("GOLDRUST_ALLOW_EXTERNAL_API_CALL must be parseable as a boolean");
        tracing::trace!(?allow_external_api_call);

        let update_golden_files: bool = std::env::var("GOLDRUST_UPDATE_GOLDEN_FILES")
            .unwrap_or("true".to_string())
            .parse()
            .expect("GOLDRUST_UPDATE_GOLDEN_FILES must be a boolean");
        tracing::trace!(?update_golden_files);

        let save_check = !update_golden_files;
        tracing::trace!(?save_check);

        let response_source = response_source(
            allow_external_api_call,
            update_golden_files,
            golden_file_path.exists(),
        );
        tracing::trace!(?response_source);

        Self {
            allow_external_api_call,
            update_golden_files,
            golden_file_path,
            response_source,
            save_check,
        }
    }
}

impl Goldrust {
    /// Save content to the golden file
    ///
    /// This method should be called when required,
    /// or Goldrust will panic when dropped.
    #[tracing::instrument(skip(self))]
    pub fn save<T>(&self, content: T) -> Result<(), Error>
    where
        T: serde::Serialize,
        for<'de> T: serde::Deserialize<'de>,
        T: std::fmt::Debug,
    {
        if !self.update_golden_files {
            tracing::debug!("Golden files should not be updated, skipping save");
            return Ok(());
        }
        tracing::debug!("Saving content to file");
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.golden_file_path)?;

        serde_json::to_writer_pretty(file, &content)?;
        Ok(())
    }
}

/// Evaluates the response source based on the configuration
///
/// For detailed combinations of possible evaluations, check the source code.
#[tracing::instrument]
fn response_source(
    allow_external_api_call: bool,
    update_golden_files: bool,
    golden_file_exists: bool,
) -> ResponseSource {
    let response_source: ResponseSource = match (
        allow_external_api_call,
        update_golden_files,
        golden_file_exists,
    ) {
        (false, true, _) => {
            panic!("Cannot update golden files without allowing external API calls")
        }
        (false, false, false) => {
            panic!("Cannot test without allowing external API calls when golden files do not exist")
        }
        (false, false, true) => {
            tracing::trace!("Use local golden files without making external API calls");
            ResponseSource::Local
        }
        (true, false, false) => {
            tracing::trace!("Use external API without updating golden files");
            ResponseSource::External
        }
        (true, false, true) => {
            tracing::trace!("Use local golden files without making external API calls, even though external API calls are allowed");
            ResponseSource::Local
        }
        (true, true, _) => {
            tracing::trace!("Use external API calls and update golden files");
            ResponseSource::External
        }
    };
    response_source
}

impl Drop for Goldrust {
    fn drop(&mut self) {
        if !self.save_check {
            panic!("should save content to golden file. Call the `save` method");
        }
    }
}

#[derive(Debug)]
pub enum ResponseSource {
    Local,
    External,
}

#[cfg(test)]
mod tests {
    use tracing_subscriber::EnvFilter;

    #[test]
    fn examples_test() {
        tracing_subscriber::fmt::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .init();
    }
}

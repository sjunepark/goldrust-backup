//! # Introduction
//!
//! A minimal golden file testing library for Rust,
//! when golden files are required for external api requests.
//!
//! ## Warning & Disclaimer
//! Currently intended for _personal use_, and has the following limitations:
//! - Be aware that the design is very messy and not scalable.
//! - This is just the initial published version, and the API is not stable.
//!
//! ## When to use
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
//! ## Testing logic
//!
//! In the case which fits the [When to use](#when-to-use) section, you can use the following logic:
//!
//! 1. Create a mock server which intercepts requests
//! - Your function, which sends external api requests, should accept a domain parameter,
//!   so it can be injected as a dependency.
//!   You can set this domain as:
//!   - In production: The actual domain
//!   - In tests: The mock server uri
//! 2. Serve the mock response:
//!    1. When a golden file does not exist
//!       (or an update is required via the `GOLDRUST_UPDATE_GOLDEN_FILES` env var):
//!       1. Create an external api request
//!       2. Save the response body to the golden file
//!    2. When a golden file exists and no update is required,
//!       serve the golden file for mock responses
//!
//! ## Async
//!
//! Only supports async, as was intended to be used in relevance to http request mocking.
//!
//! ## Usage
//!
//! Check the examples folder for a full example.

use std::fs::OpenOptions;
use std::future::Future;
use std::io::{Error, Write};
use std::path::Path;

#[derive(Debug)]
pub struct GoldRust<P>
where
    P: AsRef<Path> + std::fmt::Debug + Clone,
{
    allow_external_api_call: bool,
    update_golden_files: bool,
    golden_file_path: P,
}

impl<P> GoldRust<P>
where
    P: AsRef<Path> + std::fmt::Debug + Clone,
{
    #[tracing::instrument]
    pub fn new(golden_file_path: P) -> Self {
        let allow_external_api_call: bool = std::env::var("GOLDRUST_ALLOW_EXTERNAL_API_CALL")
            .expect("GOLDRUST_ALLOW_EXTERNAL_API_CALL must be set")
            .parse()
            .expect("GOLDRUST_ALLOW_EXTERNAL_API_CALL must be a boolean");

        let update_golden_files: bool = std::env::var("GOLDRUST_UPDATE_GOLDEN_FILES")
            .expect("GOLDRUST_UPDATE_GOLDEN_FILES must be set")
            .parse()
            .expect("GOLDRUST_UPDATE_GOLDEN_FILES must be a boolean");

        tracing::debug!(
            allow_external_api_call,
            update_golden_files,
            golden_file_path = ?golden_file_path,
            "Initialising GoldRust"
        );

        Self {
            allow_external_api_call,
            update_golden_files,
            golden_file_path,
        }
    }

    /// Run either closure based on the configuration
    /// Only one closure will run, meaning that there are zero concurrency issues
    /// However, the type system is not aware of this.
    /// As so, you have to ensure thread safety for items passed to both closures,
    /// just to gain type safety.
    /// * `run_when_local` - Function with related setup when local golden files are to be used.
    /// * `run_when_external` - Function with related setup when external API calls are to be made.
    ///
    /// Returns a closure that can be used to save the content to the golden file.
    #[tracing::instrument(skip(self, run_when_local, run_when_external))]
    pub async fn run<LocalFut, ExternalFut>(
        &self,
        run_when_local: impl FnOnce() -> LocalFut,
        run_when_external: impl FnOnce() -> ExternalFut,
    ) -> impl FnOnce(&[u8]) -> Result<(), Error>
    where
        LocalFut: Future<Output = ()>,
        ExternalFut: Future<Output = ()>,
    {
        let golden_file_exists = self.golden_file_path.as_ref().exists();
        let response_source = self.response_source(golden_file_exists);

        match response_source {
            ResponseSource::Local => {
                tracing::debug!(response_source = ?response_source, golden_file_path = ?self.golden_file_path, "Running run_when_local");
                run_when_local().await;
            }
            ResponseSource::External => {
                tracing::debug!(response_source = ?response_source, "Running run_when_external");
                run_when_external().await;
            }
        };

        self.create_save_closure()
    }

    #[tracing::instrument(skip(self))]
    fn create_save_closure(&self) -> impl FnOnce(&[u8]) -> Result<(), Error> {
        let golden_file_path = self.golden_file_path.clone();
        let update_golden_files = self.update_golden_files;

        move |content| {
            if !update_golden_files {
                return Ok(());
            }
            tracing::debug!(golden_file_path = ?golden_file_path, "Saving content to file");
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(golden_file_path)?;

            file.write_all(content)?;
            file.flush()?;
            Ok(())
        }
    }

    #[tracing::instrument(skip(self))]
    fn response_source(&self, golden_file_exists: bool) -> ResponseSource {
        let response_source: ResponseSource = match (
            self.allow_external_api_call,
            self.update_golden_files,
            golden_file_exists,
        ) {
            (false, true, _) => {
                panic!("Cannot update golden files without allowing external API calls")
            }
            (false, false, false) => {
                panic!("Cannot test without allowing external API calls when golden files do not exist")
            }
            // Get from local without making external API calls
            (false, false, true) => ResponseSource::Local,
            // Get from external API without updating golden files
            (true, false, false) => ResponseSource::External,
            // Even if external API calls are allowed, respond from local if golden files exist
            (true, false, true) => ResponseSource::Local,
            // Allow external API calls and update golden files
            (true, true, _) => ResponseSource::External,
        };

        tracing::trace!(response_source = ?response_source, "Evaluated response source");
        response_source
    }
}

#[derive(Debug)]
enum ResponseSource {
    Local,
    External,
}

/// Get the test ID, based on the thread name
pub fn get_test_id() -> String {
    std::thread::current()
        .name()
        .expect("Thread should have a name. Threads don't have names by default when they are created with `thread::Builder::spawn`")
        .split("::")
        .collect::<Vec<_>>()
        .join("-")
        .to_string()
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

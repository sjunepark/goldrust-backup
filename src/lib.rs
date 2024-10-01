//! # Introduction
//!
//! A minimal golden file testing library for Rust,
//! when golden files are required for external api requests.
//!
//! # Warning & Disclaimer
//! Currently intended for _personal use_, and has the following limitations:
//! - The API is not stable.
//! - The library could be appropriate only for specific use cases.
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
//! Check `tests/base.rs` for a full example.
//! Instead of giving a detailed implementation on how tests should be set,
//! this library provides a `ResponseSource` enum for the library user to use.
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
//!   - Defaults to `false`.
//! - `GOLDRUST_UPDATE_GOLDEN_FILES`: `bool`
//!   - Whether golden files should be updated.
//!   - Defaults to `false`.
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

mod impl_check;

use derive_more::Display;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Error;
use std::path::{Path, PathBuf};

assert_impl_commons_without_default!(Goldrust);
assert_impl_commons_without_default!(ResponseSource);

/// Create a new instance of Goldrust.
///
/// A new instance of Goldrust should be created for each test.
///
/// The configurations are based on the environment variables:
/// - `GOLDRUST_DIR`: The directory where the golden files will be saved.
///    Defaults to `tests/golden`
/// - `GOLDRUST_ALLOW_EXTERNAL_API_CALL`: Whether external api calls are allowed.
/// - `GOLDRUST_UPDATE_GOLDEN_FILES`: Whether golden files should be updated.
///
/// Even if `GOLDRUST_ALLOW_EXTERNAL_API_CALL` is set to `true`,
/// the default behavior is to use local golden files without making external API calls,
/// which is the preferred behavior for testing.
#[macro_export]
macro_rules! goldrust {
    () => {
        Goldrust::new({
            fn f() {}
            fn type_name_of_val<T>(_: T) -> &'static str {
                std::any::type_name::<T>()
            }
            let mut name = type_name_of_val(f).strip_suffix("::f").unwrap_or("");
            while let Some(rest) = name.strip_suffix("::{{closure}}") {
                name = rest;
            }
            &name.replace("::", "-")
        })
    };
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Serialize, Deserialize, Display)]
#[display("{update_golden_files}, {golden_file_path:?}, {response_source}, {save_check}")]
pub struct Goldrust {
    update_golden_files: bool,
    /// The path to the golden file,
    /// which was automatically generated based on the thread name of the test
    pub golden_file_path: PathBuf,
    pub response_source: ResponseSource,
    pub save_check: bool,
}

impl Goldrust {
    /// Create a new instance of GoldrustBuilder
    ///
    /// A new instance of Goldrust should be created for each test.
    ///
    /// Golden file names are based on the thread name of the test.
    /// (e.g. `test::test_name` → `test-test_name.json`)
    #[tracing::instrument]
    pub fn new(function_name: &str) -> Self {
        let golden_file_dir =
            std::env::var("GOLDRUST_DIR").unwrap_or("tests/resources/golden".to_string());
        let golden_file_path = Path::new(&golden_file_dir).join(format!("{}.json", function_name));

        let allow_external_api_call: bool = std::env::var("GOLDRUST_ALLOW_EXTERNAL_API_CALL")
            .unwrap_or("false".to_string())
            .parse()
            .expect("GOLDRUST_ALLOW_EXTERNAL_API_CALL must be parseable as a boolean");

        let update_golden_files: bool = std::env::var("GOLDRUST_UPDATE_GOLDEN_FILES")
            .unwrap_or("false".to_string())
            .parse()
            .expect("GOLDRUST_UPDATE_GOLDEN_FILES must be a boolean");

        let save_check = !update_golden_files;

        let response_source = response_source(
            allow_external_api_call,
            update_golden_files,
            golden_file_path.as_ref(),
        );

        Self {
            update_golden_files,
            golden_file_path,
            response_source,
            save_check,
        }
    }

    /// Save content to the golden file
    ///
    /// This method should be called when required,
    /// or Goldrust will panic when dropped.
    #[tracing::instrument(skip(self, content))]
    pub fn save<T>(&mut self, content: T) -> Result<(), Error>
    where
        T: serde::Serialize,
        for<'de> T: serde::Deserialize<'de>,
        T: std::fmt::Debug,
    {
        self.save_check = true;
        if !self.update_golden_files {
            tracing::debug!("Golden files should not be updated, skipping save");
            return Ok(());
        }
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.golden_file_path)
            .inspect_err(|_e| tracing::error!(?self.golden_file_path, "Error opening file"))?;
        let file_fmt = format!("{:?}", self.golden_file_path);

        serde_json::to_writer_pretty(file, &content)
            .inspect_err(|_e| tracing::error!(file = file_fmt, "Error writing content to file"))?;
        tracing::debug!(?self.golden_file_path, "Saved content to golden file");

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
    golden_file_path: &Path,
) -> ResponseSource {
    let golden_file_exists = golden_file_path.exists();

    let response_source: ResponseSource = match (
        allow_external_api_call,
        update_golden_files,
        golden_file_exists,
    ) {
        (false, true, _) => {
            panic!("Cannot update golden files without allowing external API calls")
        }
        (false, false, false) => {
            panic!("Cannot test without allowing external API calls when golden files do not exist, create file: {}", golden_file_path.display())
        }
        (false, false, true) => {
            tracing::debug!("Use local golden files without making external API calls");
            ResponseSource::Local
        }
        (true, false, false) => {
            tracing::debug!("Use external API without updating golden files");
            ResponseSource::External
        }
        (true, false, true) => {
            tracing::debug!("Use local golden files without making external API calls, even though external API calls are allowed");
            ResponseSource::Local
        }
        (true, true, _) => {
            tracing::debug!("Use external API calls and update golden files");
            ResponseSource::External
        }
    };
    response_source
}

/// This ensures that the content is saved to the golden file
/// when an update is required.
impl Drop for Goldrust {
    fn drop(&mut self) {
        if !self.save_check {
            tracing::error!("Should save item to golden file.\nEven if you've called the `save` methods, it might not be executing due to prior early returns, etc.");
        }
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Serialize, Deserialize, Display)]
#[display("{_variant}")]
pub enum ResponseSource {
    Local,
    External,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_goldrust() {
        let goldrust = goldrust!();
        assert_eq!(
            format!("{}", goldrust),
            format!(
                "{}, {:?}, {}, {}",
                goldrust.update_golden_files,
                goldrust.golden_file_path,
                goldrust.response_source,
                goldrust.save_check
            )
        );
    }
}

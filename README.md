# goldrust

A minimal golden file testing library for Rust. Intended for _personal use_,
so be aware that the design is very messy and not scalable.

## When to use

This crate is intended to be used in the situation below:

- You're sending external api requests
- You want to mock the responses
- You want to create the mocks based on the actual responses
    - You have to save these to golden files

## Test logic

In the case which fits the [When to use](#when-to-use) section, you can use the following logic:

1. Create a mock server which intercepts requests
    - Your request function should accept a domain parameter to inject dependencies(domain).
      You can set this domain as:
        - In production: The actual domain
        - In tests: The mock server uri
2. When a golden file does not exist (or an update is required via the `GOLDRUST_UPDATE_GOLDEN_FILES` env var):
    1. Create an external api request
    2. Save the response body to the golden file
3. When a golden file exists and no update is required, serve the golden file for mock responses

## Async

Only supports async, as was intended to be used in relevance to http request mocking.

## Usage

Check the examples folder for a full example.
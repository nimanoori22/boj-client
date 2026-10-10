[日本語](./README.md) | English

# boj-client

[![CI](https://github.com/explorrrr/boj-client/actions/workflows/ci.yml/badge.svg)](https://github.com/explorrrr/boj-client/actions/workflows/ci.yml)
[![Nightly Contract Test](https://github.com/explorrrr/boj-client/actions/workflows/nightly-contract.yml/badge.svg)](https://github.com/explorrrr/boj-client/actions/workflows/nightly-contract.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

## Support

[![Buy Me a Coffee](https://cdn.buymeacoffee.com/buttons/v2/default-yellow.png)](https://buymeacoffee.com/explorrrr)

If this OSS helps you, consider supporting ongoing development.

An unofficial Rust API client for the Bank of Japan Time-Series Statistics Data Search API functions (launched on February 18, 2026).

## What this crate is

`boj-client` is designed to make it easier to integrate the BOJ time-series statistics API into Rust applications. It provides a cleaner way to build requests and handle responses based on the official API specification.

## Features

- Simplifies request building for the BOJ API
- Organizes API response handling for Rust usage
- Includes companion documentation converted from the official API manual

## Installation

For this async development branch, add a path dependency to `Cargo.toml`:
The pinned Rustopus Git dependency must become a registry dependency before
this branch can be published to crates.io. Published 0.3.x releases use the old API.

```toml
[dependencies]
boj-client = { path = "../boj-client" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Official references

- BOJ announcement (2026-02-18): [Launch of API functions in the Time-Series Statistics Data Search Site](https://www.boj.or.jp/statistics/outline/notice_2026/not260218a.htm)
- API manual (PDF): [api_manual.pdf](https://www.stat-search.boj.or.jp/info/api_manual.pdf)
- API usage notice (PDF): [api_notice.pdf](https://www.stat-search.boj.or.jp/info/api_notice.pdf)

## Positioning (Unofficial client)

- `boj-client` is an unofficial (non-endorsed by BOJ) client for the BOJ Time-Series Statistics Data Search API functions.
- For usage conditions, limitations, and disclaimer/guarantee scope, prioritize the official BOJ documents above.

## Important Notice (Usage & Disclaimer)

- Usage, interpretation of specifications, and operational decisions for this software must follow BOJ official documents and BOJ's latest published decisions.
- If this README or implementation differs from BOJ publications/decisions, BOJ publications/decisions take precedence.
- If BOJ requests actions regarding this software, the author will comply without prior notice. This may include changes to specifications, provided contents, or publication status.
- To the fullest extent permitted by applicable law, the author of this software shall not be liable for any damages arising from the use or inability to use this software.

## Quick links

- [Contributing](./CONTRIBUTING.en.md)
- [Contributing (Japanese)](./CONTRIBUTING.md)
- [API manual](./docs/api-manual/README.md)
- [API client design](./docs/design/api-client-versioning.md)
- [License](./LICENSE)
- [Buy Me a Coffee](https://buymeacoffee.com/explorrrr)

## Quick start

Response types are separated by API:

- `get_data_code` -> `CodeResponse`
- `get_data_layer` -> `LayerResponse`
- `get_metadata` -> `MetadataResponse`

```rust
use boj_client::client::BojClient;
use boj_client::query::{CodeQuery, Format, Language};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = BojClient::new()?;
    let query = CodeQuery::new("CO", vec!["TK99F1000601GCQ01000".to_string()])?
        .with_format(Format::Json)
        .with_lang(Language::En)
        .with_start_date("202401")?
        .with_end_date("202401")?;

    let _response = client.get_data_code(&query).await?;
    Ok(())
}
```

## Async client and resilience

`BojClient` endpoint methods are now async: add `.await` to existing calls.
The blocking client has been removed. Run requests inside a Tokio runtime.

`BojClient::with_config(BojResilienceConfig { ..Default::default() })` configures
rate, burst, concurrency, retry attempts, backoff, request timeout, total page
deadline, and maximum pages. Defaults are 1 request/second, burst 1, concurrency
2, 4 attempts, a 30-second attempt timeout, and a 180-second total page deadline.
The rate is a conservative client setting, not a published BOJ quota.

Use `BojClient::with_client(http, base_url, config)` to inject proxy/TLS settings
or a mock endpoint. Build `http` using the re-exported `boj_client::reqwest`.
Create one client and clone it: all clones share their Rustopus limiter and
server-directed pauses. HTTP transient errors and BOJ JSON `STATUS=500/503`
are retried, including gzip errors returned for CSV requests. Permanent API
errors, malformed payloads, and invalid input fail without retries.

`get_data_code` and `get_data_layer` return one page. The async
`get_data_code_all` and `get_data_layer_all` helpers follow `next_position` and
return merged series, without raw page bodies. They reject non-advancing cursors,
conflicting points, and unfinished results at the page limit. If the input
query has a start position or date filters, completeness covers that selection.
Each page has its own total deadline; dropping the future cancels waiting and
in-flight requests. Retry exhaustion preserves the final error and attempt count.

For Winter, map complete series to collected snapshots in a separate adapter;
BOJ survey dates and series-level last-update dates are not ALFRED vintage
intervals. See [Winter integration](./docs/design/winter-integration.md).

## Public API Notes (0.3.0)

- Breaking: `BojClient::new()` now returns `Result<BojClient, BojError>`.
- Breaking: `Default` implementation for `BojClient` was removed.
- The built-in default HTTP client now uses a 30-second timeout.

- Client: `boj_client::client::BojClient`
- Discovery catalog: `boj_client::catalog::*`
- Queries and options: `boj_client::query::*`
- Response types: `boj_client::model::*`
- `decode` / `transport` are internal implementation modules and are not public

The project is still under active development. For behavior details, prioritize the API specification docs.

- API manual: [`docs/api-manual/README.md`](./docs/api-manual/README.md)
- API client design: [`docs/design/api-client-versioning.md`](./docs/design/api-client-versioning.md)

`docs/api-manual` contains BOJ official API specification references (source-aligned). `docs/design` contains this crate's client-side design policies (public interfaces, versioning, and compatibility operations). `docs/design/api-client-versioning.md` is a policy proposal for handling potential future BOJ API version changes and does not define BOJ's official versioning specification. If interpretation differs, prioritize `docs/api-manual` first and update `docs/design` as needed.

## Coverage and limitations

- Target API: Bank of Japan Time-Series Statistics Data Search API functions (launched on February 18, 2026; see BOJ announcement above)
- This crate is an unofficial client (not endorsed by BOJ)
- This repository prioritizes conformance to the official specification
- When the specification changes, updates follow the official manual

## Development status

- Status: In active development
- Compatibility and API design may still change before stable releases

### README translation policy (loose synchronization)

- Single-language updates are allowed
- If only one language is updated, add the following note to the updated README

> Note: Japanese version will be synchronized later.

- Remove the note once synchronization is complete
- It is acceptable for `Last updated` dates to differ between languages

## License

MIT License. See [`LICENSE`](./LICENSE) for details.

Last updated: 2026-10-09

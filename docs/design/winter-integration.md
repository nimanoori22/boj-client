# Winter integration

The library is now async. It depends on a pinned Rustopus Git revision and is
intended for path/Git consumption. The existing crates.io publishing workflow
cannot publish this dependency graph until Rustopus is available as a registry
dependency. Published BOJ 0.3.x releases retain their previous API.

Create a single `BojClient` in Winter's composition root and wrap it in a
`MacroDataSource` adapter. Clone the client to share its retry, rate, concurrency,
and server-directed pause state. The client does not own a Tokio runtime.

The Winter adapter is `crates/adapters/boj_client`, with an aliased
`boj-client-api` workspace dependency on this adjacent repository. Its source
registry key is `boj`. No BOJ API key is needed. To move to a Git dependency,
replace the workspace path dependency with the URL and revision of your pushed
BOJ repository.

Winter stores the BOJ selector in the factor's `source_series` database field,
alongside its readable slug and `source: "boj"`:

```json
{
  "slug": "boj-tankan-large-manufacturing",
  "source": "boj",
  "source_series": {
    "db": "CO",
    "code": "TK99F1000601GCQ01000",
    "frequency": "Q"
  }
}
```

This code is the Tankan large-enterprise manufacturing business-conditions DI
used by the repository's request example. A configuration example and an
optional factor seed are supplied with Winter's adapter. The implementation
does not alter a running database or load example factors automatically.
Every adapter receives `source_series` from Winter's factor catalog; BOJ needs
`db`, `code`, and `frequency`. A missing or invalid selector fails visibly.

Both `fetch_full_history` and `fetch_since` fetch all remaining pages of the
complete current history. They return `CollectedSnapshot { collected_at }`.
Older periods must be included on refresh because BOJ can revise them, and
`since` is Winter's prior collection watermark, not a BOJ changes-since filter.

Survey dates become the first day of the period at UTC midnight. Calendar
quarters begin in January/April/July/October; calendar half-years in January/July;
fiscal years use April of the labeled year and fiscal halves April/October.
Weekly and daily series retain the actual survey date. Configuration must match
the returned series frequency. Missing values remain `None`; invalid dates,
unexpected series, non-finite or invalid numbers, and incomplete pagination fail
the entire batch. Country information comes from Winter's factor catalog.

Vintage starts use one collection timestamp for the batch, and ends use Winter's
open-ended sentinel. Survey periods and series-level `LAST_UPDATE` must not be
presented as authoritative historical publication vintages. Winter can establish
availability history only from the snapshots it actually collects.

## Migration from the synchronous client

- Add `.await` to endpoint calls and execute them in a Tokio runtime.
- Inject an async `boj_client::reqwest::Client` using
  `BojClient::with_client(http, base_url, config)`.
- Remove external retry loops; retries are owned by the shared Rustopus arm.
- Handle `RetryExhausted { attempts, last }`, `DeadlineExceeded`, structured
  transport failures, and pagination errors.
- Use `get_data_code_all` / `get_data_layer_all` when persistence requires a
  complete selection. Single-page methods preserve the raw response text.

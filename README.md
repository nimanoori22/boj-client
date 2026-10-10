日本語 | [English](./README.en.md)

# boj-client

[![CI](https://github.com/explorrrr/boj-client/actions/workflows/ci.yml/badge.svg)](https://github.com/explorrrr/boj-client/actions/workflows/ci.yml)
[![Nightly Contract Test](https://github.com/explorrrr/boj-client/actions/workflows/nightly-contract.yml/badge.svg)](https://github.com/explorrrr/boj-client/actions/workflows/nightly-contract.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

## サポート

[![Buy Me a Coffee](https://cdn.buymeacoffee.com/buttons/v2/default-yellow.png)](https://buymeacoffee.com/explorrrr)

このOSSが役に立ったら、開発継続のための支援をお願いします。

日本銀行「時系列統計データ検索サイト API機能」（2026年2月18日提供開始）を利用するための、Rust向け非公式APIクライアントです。

## このクレートについて

`boj-client` は、日本銀行の時系列統計データ取得をRustアプリケーションへ組み込みやすくすることを目的としたクライアントです。API仕様に沿ったリクエスト生成とレスポンス処理を提供し、利用者はデータ取得ロジックをシンプルに実装できます。

## 主な機能

- 日本銀行APIに対するリクエスト組み立ての簡素化
- APIレスポンスの取り扱いをRust向けに整理
- 公式マニュアルをMarkdown化した補助ドキュメントを同梱

## インストール

この非同期開発ブランチはパス依存として利用してください。Rustopus の Git 依存を使用しているため、crates.io 公開前にレジストリ依存へ変更する必要があります。公開済みの 0.3.x は従来のAPIです。

```toml
[dependencies]
boj-client = { path = "../boj-client" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## 公式情報（参照元）

- BOJ告知（2026-02-18）: [時系列統計データ検索サイトにおけるAPI機能の提供開始について](https://www.boj.or.jp/statistics/outline/notice_2026/not260218a.htm)
- API機能利用マニュアル（PDF）: [api_manual.pdf](https://www.stat-search.boj.or.jp/info/api_manual.pdf)
- API機能利用時の留意点（PDF）: [api_notice.pdf](https://www.stat-search.boj.or.jp/info/api_notice.pdf)

## 位置づけ（非公式クライアント）

- `boj-client` は、日本銀行が提供する「時系列統計データ検索サイト API機能」を利用するための非公式（BOJ非公認）クライアントです。
- 利用条件・制限事項・免責/保証範囲は、上記の日本銀行公式文書を優先してください。

## 重要事項（利用条件・免責）

- 本ソフトウェアの利用方法、仕様解釈、運用判断は、日本銀行（BOJ）の公式文書および最新の公表内容・決定に従ってください。
- 本READMEや実装内容と、日本銀行の公表内容・決定に差異がある場合は、日本銀行の内容を優先します。
- 日本銀行（BOJ）から本ソフトウェアに関する要請があった場合、作成者は予告なく当該要請に従います。これに伴い、仕様・提供内容・公開状態などを変更することがあります。
- 法令上許される範囲で、本ソフトウェアの作成者は、本ソフトウェアの利用または利用不能に起因して生じたいかなる損害についても責任を負いません。

## クイックリンク

- [Contributing](./CONTRIBUTING.md)
- [Contributing (English)](./CONTRIBUTING.en.md)
- [APIマニュアル](./docs/api-manual/README.md)
- [APIクライアント設計](./docs/design/api-client-versioning.md)
- [ライセンス](./LICENSE)
- [Buy Me a Coffee](https://buymeacoffee.com/explorrrr)

## クイックスタート

返却型は API ごとに分かれています。

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

> 非同期クライアントと再試行の説明は以下の英語セクションを参照してください。

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

## 公開APIノート（0.3.0）

- Breaking: `BojClient::new()` は `Result<BojClient, BojError>` を返すようになりました。
- Breaking: `BojClient` の `Default` 実装を削除しました。
- 既定の内部HTTPクライアントは 30 秒タイムアウトを設定します。

- クライアント: `boj_client::client::BojClient`
- ディスカバリ辞書: `boj_client::catalog::*`
- クエリとオプション: `boj_client::query::*`
- レスポンス型: `boj_client::model::*`
- `decode` / `transport` は内部実装となり、公開されません

開発中のため、仕様詳細はAPI仕様ドキュメントを優先して参照してください。

- APIマニュアル: [`docs/api-manual/README.md`](./docs/api-manual/README.md)
- APIクライアント設計: [`docs/design/api-client-versioning.md`](./docs/design/api-client-versioning.md)

`docs/api-manual` は日本銀行の公式API仕様（原本準拠）を参照するための文書群です。`docs/design` は本クレートのクライアント設計方針（公開IF、バージョン管理、互換性運用）を管理する文書群です。`docs/design/api-client-versioning.md` は、将来の日銀API仕様変更に備えるためのクライアント側方針案であり、日銀公式のバージョニング仕様そのものを定義する文書ではありません。仕様解釈で差分が出る場合は、まず `docs/api-manual` を優先し、必要に応じて `docs/design` を追従更新します。

## Contributing

寄稿方法は [`CONTRIBUTING.md`](./CONTRIBUTING.md) を参照してください。
英語版は [`CONTRIBUTING.en.md`](./CONTRIBUTING.en.md) を参照してください。

## 対応範囲と制約

- 対象API: 日本銀行「時系列統計データ検索サイト API機能」（2026年2月18日提供開始、上記BOJ告知参照）
- 本クレートは非公式（BOJ非公認）クライアントです
- 本リポジトリは、公式仕様に準拠することを優先します
- 仕様改定時は、公式マニュアルに追従して更新します

## 開発ステータス

- ステータス: 開発中
- 互換性やAPI設計は、リリースまでに調整される可能性があります

### README翻訳運用（緩め同期）

- 片言語のみ先行更新を許可します
- 未同期で更新する場合、更新したREADMEに次の注記を記載します

> 注: 英語版は追って同期します。

- 同期完了時に、上記注記を削除します
- 言語ごとの更新日時が一致しなくても問題ありません

## ライセンス

MIT License。詳細は [`LICENSE`](./LICENSE) を参照してください。

Last updated: 2026-10-09

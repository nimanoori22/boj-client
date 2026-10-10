mod common;

use std::io::Write;

use boj_client::client::BojClient;
use boj_client::error::BojError;
use boj_client::query::{CodeQuery, Format, Language, MetadataQuery};
use common::{StubResponse, StubServer, fixture_bytes};
use flate2::Compression;
use flate2::write::GzEncoder;

#[tokio::test]
async fn csv_request_still_parses_json_error_payload() {
    let server = StubServer::serve_once(StubResponse::with_content_type(
        200,
        fixture_bytes("tests/fixtures/csv_error_json_payload.json"),
        "text/csv",
    ));
    let client = BojClient::new()
        .expect("default client should build")
        .with_base_url(server.base_url().to_string());

    let query = CodeQuery::new("CO", vec!["TK99F1000601GCQ01000".to_string()])
        .unwrap()
        .with_format(Format::Csv)
        .with_lang(Language::Jp);

    let error = client.get_data_code(&query).await.unwrap_err();
    let _ = server.finish();

    assert_eq!(
        error,
        BojError::ApiError {
            status: 400,
            message_id: "M181004E".to_string(),
            message: "DBが指定されていません。".to_string(),
        }
    );
}

#[tokio::test]
async fn gzip_json_response_is_decoded() {
    let original = fixture_bytes("tests/fixtures/json_success_code_api.json");

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&original).unwrap();
    let compressed = encoder.finish().unwrap();

    let server = StubServer::serve_once(
        StubResponse::with_content_type(200, compressed, "application/json")
            .with_header("content-encoding", "gzip"),
    );
    let client = BojClient::new()
        .expect("default client should build")
        .with_base_url(server.base_url().to_string());
    let query = CodeQuery::new("CO", vec!["TK99F1000601GCQ01000".to_string()])
        .unwrap()
        .with_format(Format::Json)
        .with_lang(Language::Jp)
        .with_start_date("202401")
        .unwrap()
        .with_end_date("202401")
        .unwrap();

    let response = client.get_data_code(&query).await.unwrap();
    let _ = server.finish();

    assert_eq!(response.meta.status, 200);
    assert_eq!(response.series.len(), 1);
    assert_eq!(response.series[0].points.len(), 2);
}

#[tokio::test]
async fn metadata_response_is_typed() {
    let server = StubServer::serve_once(StubResponse::with_content_type(
        200,
        fixture_bytes("tests/fixtures/json_success_metadata_api.json"),
        "application/json",
    ));
    let client = BojClient::new()
        .expect("default client should build")
        .with_base_url(server.base_url().to_string());

    let query = MetadataQuery::new("FM08")
        .unwrap()
        .with_format(Format::Json)
        .with_lang(Language::En);

    let response = client.get_metadata(&query).await.unwrap();
    let _ = server.finish();

    assert_eq!(response.meta.status, 200);
    assert_eq!(response.db, "FM08");
    assert!(!response.entries.is_empty());
}

#[tokio::test]
async fn transport_failure_preserves_final_kind() {
    let client = BojClient::new()
        .expect("default client should build")
        .with_base_url("http://127.0.0.1:1");
    let query = MetadataQuery::new("PR01").unwrap();

    let error = client.get_metadata(&query).await.unwrap_err();
    assert!(
        matches!(error, BojError::RetryExhausted { attempts: 4, last }
        if matches!(*last, BojError::TransportError { kind: boj_client::error::TransportErrorKind::Connect, .. }))
    );
}

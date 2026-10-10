use boj_client::{
    BojClient, BojResilienceConfig,
    error::BojError,
    query::{CodeQuery, Frequency, LayerQuery},
};
use serde_json::{Value, json};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate, matchers::path};

struct Pages(Vec<Value>);

impl Respond for Pages {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let position = request
            .url
            .query_pairs()
            .find(|(key, _)| key == "startPosition")
            .map(|(_, value)| value.parse::<usize>().unwrap())
            .unwrap_or(1);
        ResponseTemplate::new(200).set_body_json(self.0.get(position - 1).unwrap())
    }
}

fn page(next: Option<u32>, date: &str, value: Option<&str>) -> Value {
    json!({"STATUS":200,"MESSAGEID":"M181000I","MESSAGE":"OK","NEXTPOSITION":next,
    "RESULTSET":[{"SERIES_CODE":"A","FREQUENCY":"MONTHLY","VALUES":{
        "SURVEY_DATES":[date],"VALUES":[value]
    }}]})
}

fn client(server: &MockServer, max_pages: usize) -> BojClient {
    BojClient::with_client(
        boj_client::reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap(),
        server.uri(),
        BojResilienceConfig {
            requests_per_second: 0,
            max_attempts: 1,
            max_pages,
            ..Default::default()
        },
    )
    .unwrap()
}

fn query() -> CodeQuery {
    CodeQuery::new("CO", vec!["A".into()]).unwrap()
}

#[tokio::test]
async fn code_history_merges_points_and_preserves_filters() {
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/getDataCode"))
        .respond_with(Pages(vec![
            page(Some(2), "202402", None),
            page(None, "202401", Some("12")),
        ]))
        .expect(2)
        .mount(&server)
        .await;
    let query = query()
        .with_start_date("202401")
        .unwrap()
        .with_end_date("202402")
        .unwrap();
    let rows = client(&server, 2).get_data_code_all(&query).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].points.len(), 2);
    assert_eq!(rows[0].points[0].survey_date, "202401");
    assert_eq!(rows[0].points[0].value.as_deref(), Some("12"));
    assert_eq!(rows[0].points[1].value, None);
    let requests = server.received_requests().await.unwrap();
    for request in &requests {
        let pairs: Vec<_> = request.url.query_pairs().collect();
        for (key, value) in [
            ("db", "CO"),
            ("code", "A"),
            ("startDate", "202401"),
            ("endDate", "202402"),
        ] {
            assert!(pairs.iter().any(|(k, v)| k == key && v == value));
        }
    }
    assert!(
        requests[1]
            .url
            .query_pairs()
            .any(|(key, value)| key == "startPosition" && value == "2")
    );
}

#[tokio::test]
async fn layer_history_merges_points_and_deduplicates_identical_rows() {
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/getDataLayer"))
        .respond_with(Pages(vec![
            page(Some(2), "202401", Some("12")),
            page(None, "202401", Some("12")),
        ]))
        .expect(2)
        .mount(&server)
        .await;
    let query = LayerQuery::new("CO", Frequency::M, vec!["*".into()]).unwrap();
    let rows = client(&server, 2).get_data_layer_all(&query).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].points.len(), 1);
}

#[tokio::test]
async fn supplied_start_position_is_respected() {
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/getDataCode"))
        .respond_with(Pages(vec![
            page(Some(2), "202401", Some("1")),
            page(None, "202402", Some("2")),
        ]))
        .expect(1)
        .mount(&server)
        .await;
    let rows = client(&server, 1)
        .get_data_code_all(&query().with_start_position(2).unwrap())
        .await
        .unwrap();
    assert_eq!(rows[0].points[0].survey_date, "202402");
}

#[tokio::test]
async fn unfinished_result_at_page_limit_is_rejected() {
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/getDataCode"))
        .respond_with(Pages(vec![page(Some(2), "202401", Some("1"))]))
        .expect(1)
        .mount(&server)
        .await;
    assert!(matches!(
        client(&server, 1).get_data_code_all(&query()).await,
        Err(BojError::PaginationError(_))
    ));
}

#[tokio::test]
async fn repeated_backwards_and_zero_cursors_are_rejected() {
    for next in [0, 1] {
        let server = MockServer::start().await;
        Mock::given(path("/api/v1/getDataCode"))
            .respond_with(Pages(vec![page(Some(next), "202401", Some("1"))]))
            .expect(1)
            .mount(&server)
            .await;
        assert!(matches!(
            client(&server, 10).get_data_code_all(&query()).await,
            Err(BojError::PaginationError(_))
        ));
    }
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/getDataCode"))
        .respond_with(Pages(vec![
            page(Some(2), "202401", Some("1")),
            page(Some(1), "202402", Some("2")),
        ]))
        .expect(2)
        .mount(&server)
        .await;
    assert!(matches!(
        client(&server, 10).get_data_code_all(&query()).await,
        Err(BojError::PaginationError(_))
    ));
}

#[tokio::test]
async fn conflicting_values_are_rejected() {
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/getDataCode"))
        .respond_with(Pages(vec![
            page(Some(2), "202401", Some("1")),
            page(None, "202401", Some("2")),
        ]))
        .expect(2)
        .mount(&server)
        .await;
    assert!(matches!(
        client(&server, 10).get_data_code_all(&query()).await,
        Err(BojError::PaginationError(_))
    ));
}

#[tokio::test]
async fn later_api_error_discards_the_partial_result() {
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/getDataCode"))
        .respond_with(Pages(vec![
            page(Some(2), "202401", Some("1")),
            json!({"STATUS":400,"MESSAGEID":"bad","MESSAGE":"bad"}),
        ]))
        .expect(2)
        .mount(&server)
        .await;
    assert!(matches!(
        client(&server, 10).get_data_code_all(&query()).await,
        Err(BojError::ApiError { status: 400, .. })
    ));
}

#[tokio::test]
async fn complete_empty_history_is_valid() {
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/getDataCode"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"STATUS":200,"MESSAGEID":"M181030I","MESSAGE":"no data","RESULTSET":[]}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    assert!(
        client(&server, 1)
            .get_data_code_all(&query())
            .await
            .unwrap()
            .is_empty()
    );
}

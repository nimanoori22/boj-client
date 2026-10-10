use boj_client::client::BojClient;
use boj_client::error::BojError;
use boj_client::query::{CodeQuery, Format, Frequency, Language, LayerQuery, MetadataQuery};

#[derive(Debug, Clone, Copy)]
enum ApiCase {
    Code,
    Layer,
    Metadata,
}

#[derive(Debug, Clone, Copy)]
struct ContractCase {
    api: ApiCase,
    format: Format,
    lang: Language,
}

#[tokio::test]
#[ignore = "Nightly only: requires live BOJ API and network access"]
async fn live_contract_matrix_all_api_all_formats() {
    if std::env::var("BOJ_CONTRACT_TEST").as_deref() != Ok("1") {
        return;
    }

    let client = BojClient::new().expect("default client should build");

    let mut cases = Vec::new();
    for format in [Format::Json, Format::Csv] {
        for lang in [Language::Jp, Language::En] {
            cases.push(ContractCase {
                api: ApiCase::Code,
                format,
                lang,
            });
            cases.push(ContractCase {
                api: ApiCase::Layer,
                format,
                lang,
            });
            cases.push(ContractCase {
                api: ApiCase::Metadata,
                format,
                lang,
            });
        }
    }

    assert_eq!(cases.len(), 12);

    for case in cases {
        run_case(&client, case).await.unwrap_or_else(|error| {
            panic!("contract case {:?} failed with error: {error}", case);
        });
    }
}

async fn run_case(client: &BojClient, case: ContractCase) -> Result<(), BojError> {
    match case.api {
        ApiCase::Code => run_code_case(client, case).await,
        ApiCase::Layer => run_layer_case(client, case).await,
        ApiCase::Metadata => run_metadata_case(client, case).await,
    }
}

async fn run_code_case(client: &BojClient, case: ContractCase) -> Result<(), BojError> {
    let query = CodeQuery::new("CO", vec!["TK99F1000601GCQ01000".to_string()])?
        .with_format(case.format)
        .with_lang(case.lang)
        .with_start_date("202401")?
        .with_end_date("202401")?;

    let response = client.get_data_code(&query).await?;

    assert_eq!(response.meta.status, 200);
    assert_info_message_id(&response.meta.message_id);
    assert_eq!(response.parameter.db.as_deref(), Some("CO"));
    assert_eq!(
        response.parameter.format.as_deref(),
        Some(expected_format(case.format))
    );
    assert_eq!(
        response.parameter.lang.as_deref(),
        Some(expected_lang(case.lang))
    );

    if !response.series.is_empty() {
        for series in &response.series {
            assert!(!series.series_code.is_empty());
            assert!(!series.points.is_empty());
            for point in &series.points {
                assert!(!point.survey_date.is_empty());
            }
        }
    }

    Ok(())
}

async fn run_layer_case(client: &BojClient, case: ContractCase) -> Result<(), BojError> {
    let query = LayerQuery::new(
        "BP01",
        Frequency::M,
        vec!["1".to_string(), "1".to_string(), "1".to_string()],
    )?
    .with_format(case.format)
    .with_lang(case.lang)
    .with_start_date("202504")?
    .with_end_date("202504")?;

    let response = client.get_data_layer(&query).await?;

    assert_eq!(response.meta.status, 200);
    assert_info_message_id(&response.meta.message_id);
    assert_eq!(response.parameter.db.as_deref(), Some("BP01"));
    assert_eq!(
        response.parameter.format.as_deref(),
        Some(expected_format(case.format))
    );
    assert_eq!(
        response.parameter.lang.as_deref(),
        Some(expected_lang(case.lang))
    );

    if !response.series.is_empty() {
        for series in &response.series {
            assert!(!series.series_code.is_empty());
            assert!(!series.points.is_empty());
            for point in &series.points {
                assert!(!point.survey_date.is_empty());
            }
        }
    }

    Ok(())
}

async fn run_metadata_case(client: &BojClient, case: ContractCase) -> Result<(), BojError> {
    let query = MetadataQuery::new("FM08")?
        .with_format(case.format)
        .with_lang(case.lang);

    let response = client.get_metadata(&query).await?;

    assert_eq!(response.meta.status, 200);
    assert_info_message_id(&response.meta.message_id);
    assert_eq!(response.db, "FM08");

    if !response.entries.is_empty() {
        for entry in &response.entries {
            assert!(entry.layer1.is_some());
            assert!(entry.layer2.is_some());
            assert!(entry.layer3.is_some());
            assert!(entry.layer4.is_some());
            assert!(entry.layer5.is_some());

            let has_name = entry
                .name_of_time_series
                .as_deref()
                .is_some_and(|value| !value.is_empty())
                || entry
                    .name_of_time_series_j
                    .as_deref()
                    .is_some_and(|value| !value.is_empty());
            assert!(has_name);
        }
    }

    Ok(())
}

fn expected_format(format: Format) -> &'static str {
    match format {
        Format::Json => "JSON",
        Format::Csv => "CSV",
    }
}

fn expected_lang(lang: Language) -> &'static str {
    match lang {
        Language::Jp => "JP",
        Language::En => "EN",
    }
}

fn assert_info_message_id(message_id: &str) {
    assert!(message_id.starts_with("M181") && message_id.ends_with('I'));
}

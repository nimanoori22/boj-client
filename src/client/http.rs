pub(super) fn build_url(
    base_url: &str,
    endpoint: &str,
    query_pairs: &[(String, String)],
) -> String {
    let base = base_url.trim_end_matches('/');
    let mut url = format!("{base}{endpoint}");

    if !query_pairs.is_empty() {
        let query = query_pairs
            .iter()
            .map(|(key, value)| {
                let key = urlencoding::encode(key);
                let value = urlencoding::encode(value);
                format!("{key}={value}")
            })
            .collect::<Vec<_>>()
            .join("&");
        url.push('?');
        url.push_str(&query);
    }

    url
}

pub(super) fn header_value(response: &rustopus::Outcome, header_name: &str) -> Option<String> {
    response
        .headers
        .get(header_name)?
        .to_str()
        .ok()
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::build_url;

    #[test]
    fn build_url_encodes_query_pairs() {
        let url = build_url(
            "https://example.com/",
            "/api/v1/getDataCode",
            &[("code".to_string(), "A,B".to_string())],
        );
        assert_eq!(
            url,
            "https://example.com/api/v1/getDataCode?code=A%2CB".to_string()
        );
    }
}

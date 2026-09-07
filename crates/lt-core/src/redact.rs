//! Helpers that keep secrets out of errors, events, and retained logs.

/// Custom endpoints may embed tokens in the path, query, or userinfo, and
/// diagnostics retain logs; show only the origin.
pub fn display_origin(base_url: &str) -> String {
    let Ok(url) = url::Url::parse(base_url) else {
        return "<invalid url>".into();
    };
    let Some(host) = url.host_str() else {
        return "<invalid url>".into();
    };
    match url.port() {
        Some(port) => format!("{}://{host}:{port}", url.scheme()),
        None => format!("{}://{host}", url.scheme()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_origin_drops_path_query_and_credentials() {
        assert_eq!(
            display_origin("http://127.0.0.1:8080/top-secret-token/v1"),
            "http://127.0.0.1:8080"
        );
        assert_eq!(
            display_origin("https://user:pw@stt.example.com/v1?key=abc"),
            "https://stt.example.com"
        );
        assert_eq!(display_origin("not a url"), "<invalid url>");
    }
}

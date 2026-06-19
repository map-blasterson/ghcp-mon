//! Async HTTP client mirroring `web/src/api/client.ts`. Every method maps to
//! a single REST endpoint on the running `ghcp-mon serve`. Errors are
//! flattened to `anyhow::Error` with the same message shape the web client
//! throws (`"<status> <reason> for <path>"`).

use anyhow::{Result, anyhow};
use reqwest::Client;
use serde::de::DeserializeOwned;

use crate::tui::model::*;

/// Build a query string from `(key, optional_value)` pairs. Pairs whose value
/// is `None` or the empty string are skipped; keys and values are
/// percent-encoded. Returns the empty string when no entries remain.
pub fn build_qs(params: &[(&str, Option<String>)]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (k, v) in params {
        let Some(v) = v else { continue };
        if v.is_empty() {
            continue;
        }
        parts.push(format!(
            "{}={}",
            percent_encode(k),
            percent_encode(v.as_str())
        ));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("?{}", parts.join("&"))
    }
}

/// Minimal `encodeURIComponent` analog: percent-encode anything outside the
/// unreserved set. Avoids pulling another crate just for the qs helper.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        let c = *b;
        let unreserved = c.is_ascii_alphanumeric()
            || c == b'-'
            || c == b'_'
            || c == b'.'
            || c == b'~';
        if unreserved {
            out.push(c as char);
        } else {
            out.push_str(&format!("%{c:02X}"));
        }
    }
    out
}

/// REST client. Cheap to clone; reuse an instance across the TUI's lifetime
/// so reqwest can pool connections.
#[derive(Clone, Debug)]
pub struct ApiClient {
    base: String,
    http: Client,
}

impl ApiClient {
    /// `base` must already be normalised by [`crate::tui::ws::normalize_server`].
    pub fn new(base: String) -> Self {
        Self {
            base,
            http: Client::new(),
        }
    }

    async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base, path);
        let resp = self.http.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!(
                "{} {} for {}",
                resp.status().as_u16(),
                resp.status().canonical_reason().unwrap_or(""),
                path
            ));
        }
        Ok(resp.json::<T>().await?)
    }

    async fn delete_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base, path);
        let resp = self.http.delete(&url).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!(
                "{} {} for {}",
                resp.status().as_u16(),
                resp.status().canonical_reason().unwrap_or(""),
                path
            ));
        }
        Ok(resp.json::<T>().await?)
    }

    pub async fn list_sessions(
        &self,
        limit: Option<u32>,
        since: Option<i64>,
    ) -> Result<ListSessionsResponse> {
        let qs = build_qs(&[
            ("limit", Some(limit.unwrap_or(50).to_string())),
            ("since", since.map(|v| v.to_string())),
        ]);
        self.get_json(&format!("/api/sessions{qs}")).await
    }

    pub async fn get_session(&self, cid: &str) -> Result<SessionDetail> {
        self.get_json(&format!("/api/sessions/{cid}")).await
    }

    pub async fn delete_session(&self, cid: &str) -> Result<DeleteSessionResponse> {
        self.delete_json(&format!("/api/sessions/{cid}")).await
    }

    pub async fn get_session_span_tree(&self, cid: &str) -> Result<SessionSpanTreeResponse> {
        self.get_json(&format!("/api/sessions/{cid}/span-tree"))
            .await
    }

    pub async fn list_session_contexts(
        &self,
        cid: &str,
    ) -> Result<ListSessionContextsResponse> {
        self.get_json(&format!("/api/sessions/{cid}/contexts"))
            .await
    }

    pub async fn list_spans(
        &self,
        session: Option<&str>,
        kind: Option<&str>,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<ListSpansResponse> {
        let qs = build_qs(&[
            ("session", session.map(|s| s.to_string())),
            ("kind", kind.map(|s| s.to_string())),
            ("since", since.map(|v| v.to_string())),
            ("limit", Some(limit.unwrap_or(100).to_string())),
        ]);
        self.get_json(&format!("/api/spans{qs}")).await
    }

    pub async fn get_span(&self, trace_id: &str, span_id: &str) -> Result<SpanDetail> {
        self.get_json(&format!("/api/spans/{trace_id}/{span_id}"))
            .await
    }

    pub async fn list_traces(
        &self,
        limit: Option<u32>,
        since: Option<i64>,
    ) -> Result<ListTracesResponse> {
        let qs = build_qs(&[
            ("limit", Some(limit.unwrap_or(50).to_string())),
            ("since", since.map(|v| v.to_string())),
        ]);
        self.get_json(&format!("/api/traces{qs}")).await
    }

    pub async fn get_trace(&self, trace_id: &str) -> Result<TraceDetailResponse> {
        self.get_json(&format!("/api/traces/{trace_id}")).await
    }

    pub async fn list_raw(
        &self,
        record_type: Option<RawRecordType>,
        limit: Option<u32>,
    ) -> Result<ListRawResponse> {
        let qs = build_qs(&[
            ("type", record_type.map(|r| r.as_str().to_string())),
            ("limit", Some(limit.unwrap_or(100).to_string())),
        ]);
        self.get_json(&format!("/api/raw{qs}")).await
    }

    pub async fn search_spans(
        &self,
        q: &str,
        session: &str,
        limit: Option<u32>,
    ) -> Result<SearchResponse> {
        let qs = build_qs(&[
            ("q", Some(q.to_string())),
            ("session", Some(session.to_string())),
            ("limit", limit.map(|v| v.to_string())),
        ]);
        self.get_json(&format!("/api/search{qs}")).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qs_skips_none_and_empty() {
        let s = build_qs(&[
            ("a", Some("1".to_string())),
            ("b", None),
            ("c", Some(String::new())),
            ("d", Some("two words".to_string())),
        ]);
        assert_eq!(s, "?a=1&d=two%20words");
    }

    #[test]
    fn qs_empty_when_no_entries() {
        assert_eq!(build_qs(&[]), "");
        assert_eq!(build_qs(&[("x", None)]), "");
    }

    #[test]
    fn qs_percent_encodes_special_chars() {
        let s = build_qs(&[("q", Some("a&b=c".to_string()))]);
        assert_eq!(s, "?q=a%26b%3Dc");
    }
}

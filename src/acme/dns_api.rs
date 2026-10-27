//! DNS-01 records through DNS provider APIs: Cloudflare (an API token with
//! DNS edit permission; Zone read too unless `dns_zone_id` is set) and
//! Amazon Route 53 (requests signed with AWS Signature Version 4, credentials
//! from the standard `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` /
//! `AWS_SESSION_TOKEN`). The zone is configured (`dns_zone_id`) or found by
//! asking the API for each parent of the record name in turn. The API base
//! URL can be replaced (`dns_api_url`) — CI points both at a mock that
//! writes into BIND.

use std::time::Duration;

const REQUEST_TIMEOUT_MS: u64 = 15_000;
/// TTL of the challenge record.
const RECORD_TTL: u32 = 60;

/// Cloudflare's API base URL.
pub const CLOUDFLARE_API: &str = "https://api.cloudflare.com/client/v4";
/// Route 53's API endpoint (a global service, signed for us-east-1).
pub const ROUTE53_API: &str = "https://route53.amazonaws.com";

/// A secret string: `Debug` doesn't show it.
#[derive(Clone)]
pub struct Secret(pub String);

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("(hidden)")
    }
}

/// AWS credentials for Route 53.
#[derive(Clone)]
pub struct AwsCredentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    /// For temporary credentials (STS, SSO, an instance role exported to
    /// the environment).
    pub session_token: Option<String>,
}

impl std::fmt::Debug for AwsCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AwsCredentials").field("access_key_id", &self.access_key_id).field("secret_access_key", &"(hidden)").finish()
    }
}

impl AwsCredentials {
    /// From `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` and (optional)
    /// `AWS_SESSION_TOKEN`.
    pub fn from_env() -> Option<AwsCredentials> {
        let get = |name: &str| std::env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        Some(AwsCredentials {
            access_key_id: get("AWS_ACCESS_KEY_ID")?,
            secret_access_key: get("AWS_SECRET_ACCESS_KEY")?,
            session_token: get("AWS_SESSION_TOKEN"),
        })
    }
}

async fn send(method: &str, url: &str, headers: &[(String, String)], body: Option<Vec<u8>>) -> Result<(u16, String), String> {
    let client = crate::http_client::AsyncClient::new().max_redirects(0);
    let mut request = client.request(method, url).timeout_ms(REQUEST_TIMEOUT_MS);
    for (name, value) in headers {
        request = request.header(name, value);
    }
    if let Some(body) = body {
        request = request.body(body);
    }
    let response = request.send().await.map_err(|e| format!("{method} {url}: {e}"))?;
    Ok((response.status(), String::from_utf8_lossy(response.bytes()).into_owned()))
}

/// The record name's parents that could be its zone, longest first
/// (`_acme-challenge.a.example.com` → `a.example.com`, `example.com`).
pub(crate) fn zone_candidates(name: &str) -> Vec<String> {
    let labels: Vec<&str> = name.trim_end_matches('.').split('.').collect();
    (1..labels.len().saturating_sub(1)).map(|i| labels[i..].join(".")).collect()
}

// ── Cloudflare ────────────────────────────────────────────────────────────────

fn cloudflare_headers(token: &str) -> Vec<(String, String)> {
    vec![("Authorization".to_string(), format!("Bearer {token}")), ("Content-Type".to_string(), "application/json".to_string())]
}

/// A Cloudflare reply: its body when it says `"success": true`.
fn cloudflare_ok(context: &str, (status, body): (u16, String)) -> Result<String, String> {
    let success = body.find("\"success\"").is_some_and(|at| body[at + 9..].trim_start_matches([' ', ':']).starts_with("true"));
    if (200..300).contains(&status) && success {
        return Ok(body);
    }
    let message = super::json_str(&body, "message").unwrap_or_else(|| body.chars().take(200).collect());
    Err(format!("Cloudflare {context}: HTTP {status}: {message}"))
}

async fn cloudflare_zone(token: &str, api: &str, zone_id: Option<&str>, name: &str) -> Result<String, String> {
    if let Some(zone_id) = zone_id {
        return Ok(zone_id.to_string());
    }
    for candidate in zone_candidates(name) {
        let body = cloudflare_ok("zone lookup", send("GET", &format!("{api}/zones?name={candidate}"), &cloudflare_headers(token), None).await?)?;
        if let Some(id) = super::json_objects(&body, "result").first().and_then(|zone| super::json_str(zone, "id")) {
            return Ok(id);
        }
    }
    Err(format!("Cloudflare has no zone for {name} this token can see (set dns_zone_id, or give the token Zone:Read)"))
}

/// The ids of the TXT records at `name` (with content `value`, if given).
async fn cloudflare_records(token: &str, api: &str, zone: &str, name: &str, value: Option<&str>) -> Result<Vec<String>, String> {
    let url = format!("{api}/zones/{zone}/dns_records?type=TXT&name={name}");
    let body = cloudflare_ok("record lookup", send("GET", &url, &cloudflare_headers(token), None).await?)?;
    Ok(super::json_objects(&body, "result")
        .iter()
        .filter(|record| value.is_none_or(|value| super::json_str(record, "content").is_some_and(|c| c.trim_matches('"') == value)))
        .filter_map(|record| super::json_str(record, "id"))
        .collect())
}

async fn cloudflare_delete(token: &str, api: &str, zone: &str, id: &str) -> Result<(), String> {
    cloudflare_ok("record delete", send("DELETE", &format!("{api}/zones/{zone}/dns_records/{id}"), &cloudflare_headers(token), None).await?).map(|_| ())
}

/// Replace the TXT records at `name` with `value`.
pub(crate) async fn cloudflare_present(token: &str, api: &str, zone_id: Option<&str>, name: &str, value: &str) -> Result<(), String> {
    let zone = cloudflare_zone(token, api, zone_id, name).await?;
    for id in cloudflare_records(token, api, &zone, name, None).await? {
        cloudflare_delete(token, api, &zone, &id).await?;
    }
    let record = format!(
        r#"{{"type":"TXT","name":"{}","content":"\"{}\"","ttl":{RECORD_TTL}}}"#,
        super::json_escape(name),
        super::json_escape(value)
    );
    let url = format!("{api}/zones/{zone}/dns_records");
    cloudflare_ok("record create", send("POST", &url, &cloudflare_headers(token), Some(record.into_bytes())).await?).map(|_| ())
}

/// Remove the TXT record `value` at `name`.
pub(crate) async fn cloudflare_cleanup(token: &str, api: &str, zone_id: Option<&str>, name: &str, value: &str) -> Result<(), String> {
    let zone = cloudflare_zone(token, api, zone_id, name).await?;
    for id in cloudflare_records(token, api, &zone, name, Some(value)).await? {
        cloudflare_delete(token, api, &zone, &id).await?;
    }
    Ok(())
}

// ── Route 53 ──────────────────────────────────────────────────────────────────

/// The text of the first `<tag>` element.
pub(crate) fn xml_text<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&format!("</{tag}>"))?;
    Some(&xml[start..start + end])
}

/// The host part of a URL (no scheme, port or path) — what the HTTP client
/// sends as `Host`, so what is signed.
fn url_host(url: &str) -> &str {
    let authority = url.split("://").nth(1).unwrap_or(url).split('/').next().unwrap_or("");
    if authority.starts_with('[') {
        return authority.split(']').next().map_or(authority, |h| &authority[..h.len() + 1]);
    }
    authority.split(':').next().unwrap_or(authority)
}

async fn route53_send(credentials: &AwsCredentials, api: &str, method: &str, path: &str, query: &str, body: Option<String>) -> Result<String, String> {
    let payload = body.as_deref().unwrap_or("").as_bytes();
    let amz_date = amz_date(super::unix_now().max(0) as u64);
    let headers = sigv4_headers(credentials, method, url_host(api), path, query, payload, &amz_date, "us-east-1", "route53");
    let url = if query.is_empty() { format!("{api}{path}") } else { format!("{api}{path}?{query}") };
    let (status, reply) = send(method, &url, &headers, body.map(String::into_bytes)).await?;
    if (200..300).contains(&status) {
        return Ok(reply);
    }
    let message = xml_text(&reply, "Message").map(str::to_string).unwrap_or_else(|| reply.chars().take(200).collect());
    Err(format!("Route 53 {method} {path}: HTTP {status}: {message}"))
}

async fn route53_zone(credentials: &AwsCredentials, api: &str, zone_id: Option<&str>, name: &str) -> Result<String, String> {
    if let Some(zone_id) = zone_id {
        return Ok(zone_id.trim_start_matches("/hostedzone/").to_string());
    }
    for candidate in zone_candidates(name) {
        let query = format!("dnsname={candidate}&maxitems=1");
        let reply = route53_send(credentials, api, "GET", "/2013-04-01/hostedzonesbyname", &query, None).await?;
        let found = xml_text(&reply, "Name").is_some_and(|n| n.trim_end_matches('.').eq_ignore_ascii_case(&candidate));
        if let (true, Some(id)) = (found, xml_text(&reply, "Id")) {
            return Ok(id.trim_start_matches("/hostedzone/").to_string());
        }
    }
    Err(format!("Route 53 has no hosted zone for {name} (set dns_zone_id)"))
}

/// A `ChangeResourceRecordSets` body for one TXT change.
pub(crate) fn route53_change_body(action: &str, name: &str, value: &str) -> String {
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8"?>"#,
            r#"<ChangeResourceRecordSetsRequest xmlns="https://route53.amazonaws.com/doc/2013-04-01/"><ChangeBatch><Changes><Change>"#,
            "<Action>{}</Action><ResourceRecordSet><Name>{}.</Name><Type>TXT</Type><TTL>{}</TTL>",
            r#"<ResourceRecords><ResourceRecord><Value>"{}"</Value></ResourceRecord></ResourceRecords>"#,
            "</ResourceRecordSet></Change></Changes></ChangeBatch></ChangeResourceRecordSetsRequest>"
        ),
        action,
        name.trim_end_matches('.'),
        RECORD_TTL,
        value
    )
}

/// Apply `action` (`UPSERT`: replace the TXT records at `name` with
/// `value`; `DELETE`: remove it) and, for an UPSERT, wait up to `wait` for
/// Route 53 to report the change in sync.
pub(crate) async fn route53_change(credentials: &AwsCredentials, api: &str, zone_id: Option<&str>, action: &str, name: &str, value: &str, wait: Duration) -> Result<(), String> {
    let zone = route53_zone(credentials, api, zone_id, name).await?;
    let path = format!("/2013-04-01/hostedzone/{zone}/rrset");
    let reply = route53_send(credentials, api, "POST", &path, "", Some(route53_change_body(action, name, value))).await?;
    if action != "UPSERT" {
        return Ok(());
    }
    let Some(change) = xml_text(&reply, "Id").map(|id| id.trim_start_matches("/change/").to_string()) else { return Ok(()) };
    let deadline = tokio::time::Instant::now() + wait;
    let mut status = xml_text(&reply, "Status").unwrap_or("").to_string();
    while status != "INSYNC" && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_secs(3)).await;
        let reply = route53_send(credentials, api, "GET", &format!("/2013-04-01/change/{change}"), "", None).await?;
        status = xml_text(&reply, "Status").unwrap_or("").to_string();
    }
    if status != "INSYNC" {
        eprintln!("[ACME] Route 53 change {change} is still {status}; going on.");
    }
    Ok(())
}

/// `YYYYMMDDTHHMMSSZ` for a Unix time.
pub(crate) fn amz_date(unix: u64) -> String {
    let (days, secs) = (unix / 86_400, unix % 86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z", secs / 3600, secs % 3600 / 60, secs % 60)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hmac(key: &[u8], data: &str) -> Vec<u8> {
    let key = aws_lc_rs::hmac::Key::new(aws_lc_rs::hmac::HMAC_SHA256, key);
    aws_lc_rs::hmac::sign(&key, data.as_bytes()).as_ref().to_vec()
}

fn sha256_hex(data: &[u8]) -> String {
    hex(aws_lc_rs::digest::digest(&aws_lc_rs::digest::SHA256, data).as_ref())
}

/// The headers that sign a request with AWS Signature Version 4: `X-Amz-Date`,
/// `X-Amz-Security-Token` (temporary credentials) and `Authorization`.
/// `query` is the canonical query string (parameters sorted, values
/// URI-encoded) — what is sent.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sigv4_headers(
    credentials: &AwsCredentials,
    method: &str,
    host: &str,
    path: &str,
    query: &str,
    payload: &[u8],
    amz_date: &str,
    region: &str,
    service: &str,
) -> Vec<(String, String)> {
    let mut signed = vec![("host", host.to_string()), ("x-amz-date", amz_date.to_string())];
    if let Some(token) = &credentials.session_token {
        signed.push(("x-amz-security-token", token.clone()));
    }
    let canonical_headers: String = signed.iter().map(|(name, value)| format!("{name}:{}\n", value.trim())).collect();
    let signed_headers = signed.iter().map(|(name, _)| *name).collect::<Vec<_>>().join(";");
    let canonical_request = format!("{method}\n{path}\n{query}\n{canonical_headers}\n{signed_headers}\n{}", sha256_hex(payload));
    let date = &amz_date[..8];
    let scope = format!("{date}/{region}/{service}/aws4_request");
    let string_to_sign = format!("AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}", sha256_hex(canonical_request.as_bytes()));
    let mut key = hmac(format!("AWS4{}", credentials.secret_access_key).as_bytes(), date);
    for part in [region, service, "aws4_request"] {
        key = hmac(&key, part);
    }
    let signature = hex(&hmac(&key, &string_to_sign));
    let mut headers = vec![("X-Amz-Date".to_string(), amz_date.to_string())];
    if let Some(token) = &credentials.session_token {
        headers.push(("X-Amz-Security-Token".to_string(), token.clone()));
    }
    headers.push((
        "Authorization".to_string(),
        format!("AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}", credentials.access_key_id),
    ));
    headers
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example_credentials() -> AwsCredentials {
        AwsCredentials {
            access_key_id: "AKIDEXAMPLE".to_string(),
            secret_access_key: "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY".to_string(),
            session_token: None,
        }
    }

    /// AWS's Signature Version 4 test suite, `get-vanilla`.
    #[test]
    fn sigv4_matches_the_aws_test_suite() {
        let headers = sigv4_headers(&example_credentials(), "GET", "example.amazonaws.com", "/", "", b"", "20150830T123600Z", "us-east-1", "service");
        assert_eq!(
            Some(&(
                "Authorization".to_string(),
                "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, SignedHeaders=host;x-amz-date, Signature=5fa00fa31553b73ebf1942676e86291e8372ff2a2260956d9b8aae1d763fbf31".to_string()
            )),
            headers.last()
        );
        assert_eq!(("X-Amz-Date".to_string(), "20150830T123600Z".to_string()), headers[0]);
    }

    #[test]
    fn a_session_token_is_sent_and_signed() {
        let credentials = AwsCredentials { session_token: Some("TOKEN".to_string()), ..example_credentials() };
        let headers = sigv4_headers(&credentials, "GET", "example.amazonaws.com", "/", "", b"", "20150830T123600Z", "us-east-1", "service");
        assert!(headers.contains(&("X-Amz-Security-Token".to_string(), "TOKEN".to_string())));
        assert!(headers.last().unwrap().1.contains("SignedHeaders=host;x-amz-date;x-amz-security-token"));
    }

    #[test]
    fn amz_dates() {
        assert_eq!("19700101T000000Z", amz_date(0));
        assert_eq!("20150830T123600Z", amz_date(1_440_938_160));
        assert_eq!("20240229T235959Z", amz_date(1_709_251_199));
    }

    #[test]
    fn zone_candidates_go_from_longest_to_two_labels() {
        assert_eq!(vec!["a.example.com", "example.com"], zone_candidates("_acme-challenge.a.example.com"));
        assert_eq!(vec!["example.com"], zone_candidates("_acme-challenge.example.com."));
        assert!(zone_candidates("example.com").is_empty());
    }

    #[test]
    fn route53_change_xml() {
        let body = route53_change_body("UPSERT", "_acme-challenge.example.com", "abc");
        assert_eq!(Some("UPSERT"), xml_text(&body, "Action"));
        assert_eq!(Some("_acme-challenge.example.com."), xml_text(&body, "Name"));
        assert_eq!(Some("\"abc\""), xml_text(&body, "Value"));
        assert_eq!(Some("60"), xml_text(&body, "TTL"));
    }

    #[test]
    fn hosts_are_signed_as_sent() {
        assert_eq!("route53.amazonaws.com", url_host(ROUTE53_API));
        assert_eq!("127.0.0.1", url_host("http://127.0.0.1:9181/x"));
        assert_eq!("[::1]", url_host("http://[::1]:9181"));
    }

    #[test]
    fn cloudflare_replies() {
        assert!(cloudflare_ok("x", (200, r#"{"result":[],"success": true}"#.to_string())).is_ok());
        let error = cloudflare_ok("x", (403, r#"{"success":false,"errors":[{"code":10000,"message":"Authentication error"}]}"#.to_string())).unwrap_err();
        assert!(error.contains("HTTP 403") && error.contains("Authentication error"), "{error}");
        assert!(cloudflare_ok("x", (200, r#"{"success":false}"#.to_string())).is_err());
    }

    #[test]
    fn credentials_are_not_printed() {
        assert!(!format!("{:?}", example_credentials()).contains("EXAMPLEKEY"));
    }
}

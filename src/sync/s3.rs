//! S3-compatible backend: AWS Signature Version 4 over `reqwest::blocking`.
//!
//! Path-style URLs (`{endpoint}/{bucket}/{key}`) so MinIO, Cloudflare R2 and
//! Backblaze B2 work without virtual-host DNS.

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use reqwest::blocking::{Client, Response};
use sha2::Sha256;

use super::{http_client, sha256_hex, RemoteEntry};
use crate::error::{Error, Result};

/// SHA-256 of an empty body, used for GET/HEAD/LIST.
pub(crate) const EMPTY_SHA256: &str =
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
const META_SHA256: &str = "x-amz-meta-sha256";

/// Everything except RFC 3986 unreserved characters is encoded.
const AWS_ENCODE: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');
const AWS_PATH_ENCODE: &AsciiSet = &AWS_ENCODE.remove(b'/');

#[derive(Clone, PartialEq, Eq)]
pub struct Creds {
    pub access_key: String,
    pub secret_key: String,
    pub session_token: Option<String>,
}

/// Hand-written so a stray `{:?}` — a `dbg!`, a panic message, an error
/// wrapping the struct — can never print the signing key.
impl std::fmt::Debug for Creds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Creds")
            .field("access_key", &self.access_key)
            .field("secret_key", &"[redacted]")
            .field(
                "session_token",
                &self.session_token.as_ref().map(|_| "[redacted]"),
            )
            .finish()
    }
}

/// `AWS_*` env vars first, then `~/.aws/credentials` (`[$AWS_PROFILE]` or
/// `[default]`).
// ponytail: no instance metadata, SSO or assume-role. Add a chained provider
// if someone needs it.
pub fn load_credentials() -> Result<Creds> {
    if let (Ok(access_key), Ok(secret_key)) = (
        std::env::var("AWS_ACCESS_KEY_ID"),
        std::env::var("AWS_SECRET_ACCESS_KEY"),
    ) {
        return Ok(Creds {
            access_key,
            secret_key,
            session_token: std::env::var("AWS_SESSION_TOKEN").ok(),
        });
    }
    let no_creds = || {
        Error::Sync(
            "no AWS credentials: set AWS_ACCESS_KEY_ID/AWS_SECRET_ACCESS_KEY or ~/.aws/credentials"
                .into(),
        )
    };
    let path = dirs::home_dir()
        .ok_or_else(no_creds)?
        .join(".aws")
        .join("credentials");
    let text = std::fs::read_to_string(&path).map_err(|_| no_creds())?;
    let profile = std::env::var("AWS_PROFILE").unwrap_or_else(|_| "default".into());
    parse_credentials_file(&text, &profile).ok_or_else(|| {
        Error::Sync(format!(
            "profile `[{}]` with access keys not found in {}",
            profile,
            path.display()
        ))
    })
}

fn parse_credentials_file(text: &str, profile: &str) -> Option<Creds> {
    let header = format!("[{}]", profile);
    let mut in_section = false;
    let (mut access, mut secret, mut token) = (None, None, None);
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_section = line == header;
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let v = v.trim().to_string();
            match k.trim() {
                "aws_access_key_id" => access = Some(v),
                "aws_secret_access_key" => secret = Some(v),
                "aws_session_token" => token = Some(v),
                _ => {}
            }
        }
    }
    Some(Creds {
        access_key: access?,
        secret_key: secret?,
        session_token: token,
    })
}

fn encode_path(path: &str) -> String {
    utf8_percent_encode(path, AWS_PATH_ENCODE).to_string()
}

fn encode_value(value: &str) -> String {
    utf8_percent_encode(value, AWS_ENCODE).to_string()
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("hmac accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

/// Build the SigV4 headers for one request.
///
/// `path` and `query` must already be canonical: path percent-encoded once,
/// query parameters sorted and encoded, without a leading `?`. `extra`
/// headers are included in the signature. Returns lower-case header names.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sign(
    creds: &Creds,
    region: &str,
    method: &str,
    host: &str,
    path: &str,
    query: &str,
    extra: &[(&str, &str)],
    payload_hash: &str,
    now: DateTime<Utc>,
) -> Vec<(String, String)> {
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let date_stamp = now.format("%Y%m%d").to_string();

    let mut headers: Vec<(String, String)> = vec![
        ("host".into(), host.into()),
        ("x-amz-content-sha256".into(), payload_hash.into()),
        ("x-amz-date".into(), amz_date.clone()),
    ];
    if let Some(token) = &creds.session_token {
        headers.push(("x-amz-security-token".into(), token.clone()));
    }
    for (k, v) in extra {
        headers.push((k.to_lowercase(), v.trim().to_string()));
    }
    headers.sort();

    let canonical_headers: String = headers
        .iter()
        .map(|(k, v)| format!("{}:{}\n", k, v))
        .collect();
    let signed_headers: Vec<&str> = headers.iter().map(|(k, _)| k.as_str()).collect();
    let signed_headers = signed_headers.join(";");

    let canonical_request = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        method, path, query, canonical_headers, signed_headers, payload_hash
    );
    let scope = format!("{}/{}/s3/aws4_request", date_stamp, region);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{}\n{}",
        amz_date,
        scope,
        sha256_hex(canonical_request.as_bytes())
    );

    let k_date = hmac_sha256(
        format!("AWS4{}", creds.secret_key).as_bytes(),
        date_stamp.as_bytes(),
    );
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, b"s3");
    let k_signing = hmac_sha256(&k_service, b"aws4_request");
    let signature: String = hmac_sha256(&k_signing, string_to_sign.as_bytes())
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();

    headers.push((
        "authorization".into(),
        format!(
            "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
            creds.access_key, scope, signed_headers, signature
        ),
    ));
    headers
}

fn keys_from_list_xml(xml: &str, prefix: &str) -> Vec<String> {
    let re = regex::Regex::new(r"<Key>([^<]*)</Key>").expect("static regex");
    let mut out: Vec<String> = re
        .captures_iter(xml)
        .filter_map(|c| {
            c[1].strip_prefix(prefix)
                .and_then(|k| k.strip_suffix(".env"))
                .map(str::to_string)
        })
        .collect();
    out.sort();
    out
}

fn first_line_or_message(body: &str) -> String {
    let re = regex::Regex::new(r"<Message>([^<]*)</Message>").expect("static regex");
    if let Some(c) = re.captures(body) {
        return c[1].to_string();
    }
    body.lines().next().unwrap_or("").to_string()
}

pub struct S3 {
    bucket: String,
    prefix: String,
    region: String,
    endpoint: String,
    host: String,
    creds: Creds,
    client: Client,
}

impl S3 {
    pub fn new(bucket: &str, prefix: &str, region: &str, endpoint: Option<&str>) -> Result<S3> {
        let endpoint = endpoint
            .map(|e| e.trim_end_matches('/').to_string())
            .unwrap_or_else(|| format!("https://s3.{}.amazonaws.com", region));
        let url = reqwest::Url::parse(&endpoint)
            .map_err(|e| Error::Sync(format!("bad S3 endpoint `{}`: {}", endpoint, e)))?;
        let mut host = url
            .host_str()
            .ok_or_else(|| Error::Sync(format!("S3 endpoint `{}` has no host", endpoint)))?
            .to_string();
        if let Some(port) = url.port() {
            host = format!("{}:{}", host, port);
        }
        let prefix = prefix.trim_matches('/');
        let prefix = if prefix.is_empty() {
            String::new()
        } else {
            format!("{}/", prefix)
        };
        Ok(S3 {
            bucket: bucket.to_string(),
            prefix,
            region: region.to_string(),
            endpoint,
            host,
            creds: load_credentials()?,
            client: http_client()?,
        })
    }

    fn key(&self, name: &str) -> String {
        format!("{}{}.env", self.prefix, name)
    }

    /// Send one signed request. `path` is `/{bucket}` or `/{bucket}/{key}`,
    /// unencoded. `query` is already canonical.
    fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        query: &str,
        extra: &[(&str, &str)],
        body: Vec<u8>,
    ) -> Result<Response> {
        let payload_hash = if body.is_empty() {
            EMPTY_SHA256.to_string()
        } else {
            sha256_hex(&body)
        };
        let encoded_path = encode_path(path);
        let headers = sign(
            &self.creds,
            &self.region,
            method.as_str(),
            &self.host,
            &encoded_path,
            query,
            extra,
            &payload_hash,
            Utc::now(),
        );
        let mut url = format!("{}{}", self.endpoint, encoded_path);
        if !query.is_empty() {
            url.push('?');
            url.push_str(query);
        }
        let mut req = self.client.request(method, url.as_str());
        for (k, v) in headers.iter().filter(|(k, _)| k != "host") {
            req = req.header(k.as_str(), v.as_str());
        }
        Ok(req.body(body).send()?)
    }

    fn check(resp: Response, what: &str) -> Result<Response> {
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        let body = resp.text().unwrap_or_default();
        Err(Error::Sync(format!(
            "S3 {}: {} {}",
            what,
            status,
            first_line_or_message(&body)
        )))
    }

    pub fn list(&self) -> Result<Vec<RemoteEntry>> {
        // ponytail: single page, max 1000 keys. Follow NextContinuationToken
        // if anyone stores that many profiles.
        let query = format!("list-type=2&prefix={}", encode_value(&self.prefix));
        let resp = self.request(
            reqwest::Method::GET,
            &format!("/{}", self.bucket),
            &query,
            &[],
            Vec::new(),
        )?;
        let xml = Self::check(resp, "LIST")?.text()?;
        let mut out = Vec::new();
        for name in keys_from_list_xml(&xml, &self.prefix) {
            let sha256 = match self.head_sha256(&name)? {
                Some(h) => h,
                // uploaded by something else: fetch and hash
                None => sha256_hex(&self.get(&name)?),
            };
            out.push(RemoteEntry { name, sha256 });
        }
        Ok(out)
    }

    fn head_sha256(&self, name: &str) -> Result<Option<String>> {
        let resp = self.request(
            reqwest::Method::HEAD,
            &format!("/{}/{}", self.bucket, self.key(name)),
            "",
            &[],
            Vec::new(),
        )?;
        let resp = Self::check(resp, &format!("HEAD {}", self.key(name)))?;
        Ok(resp
            .headers()
            .get(META_SHA256)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string))
    }

    pub fn get(&self, name: &str) -> Result<Vec<u8>> {
        let resp = self.request(
            reqwest::Method::GET,
            &format!("/{}/{}", self.bucket, self.key(name)),
            "",
            &[],
            Vec::new(),
        )?;
        Ok(Self::check(resp, &format!("GET {}", self.key(name)))?
            .bytes()?
            .to_vec())
    }

    pub fn put(&self, name: &str, bytes: &[u8]) -> Result<()> {
        let sha = sha256_hex(bytes);
        let resp = self.request(
            reqwest::Method::PUT,
            &format!("/{}/{}", self.bucket, self.key(name)),
            "",
            &[(META_SHA256, sha.as_str())],
            bytes.to_vec(),
        )?;
        Self::check(resp, &format!("PUT {}", self.key(name)))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn example_creds() -> Creds {
        Creds {
            access_key: "AKIAIOSFODNN7EXAMPLE".into(),
            secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".into(),
            session_token: None,
        }
    }

    /// The GET Object example from the AWS "Signature Calculations for the
    /// Authorization Header: Transferring Payload in a Single Chunk" page.
    #[test]
    fn sigv4_matches_aws_documented_example() {
        let now = Utc.with_ymd_and_hms(2013, 5, 24, 0, 0, 0).unwrap();
        let headers = sign(
            &example_creds(),
            "us-east-1",
            "GET",
            "examplebucket.s3.amazonaws.com",
            "/test.txt",
            "",
            &[("range", "bytes=0-9")],
            EMPTY_SHA256,
            now,
        );
        let auth = headers
            .iter()
            .find(|(k, _)| k == "authorization")
            .map(|(_, v)| v.as_str())
            .unwrap();
        assert_eq!(
            auth,
            "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request, \
             SignedHeaders=host;range;x-amz-content-sha256;x-amz-date, \
             Signature=f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41"
        );
    }

    #[test]
    fn debug_redacts_the_signing_key() {
        let mut creds = example_creds();
        creds.session_token = Some("FwoGZXIvYXdzEXAMPLE".into());
        let shown = format!("{:?}", creds);
        assert!(!shown.contains(&creds.secret_key), "{}", shown);
        assert!(!shown.contains("FwoGZXIvYXdzEXAMPLE"), "{}", shown);
        assert_eq!(shown.matches("[redacted]").count(), 2, "{}", shown);
        // The key *id* is not a secret and stays, so a debug print is still
        // useful for telling two credential sets apart.
        assert!(shown.contains("AKIAIOSFODNN7EXAMPLE"), "{}", shown);
    }

    #[test]
    fn credentials_file_parsing() {
        let text = "[default]\naws_access_key_id = AKID\naws_secret_access_key = SECRET\n\n\
                    [work]\naws_access_key_id=W\naws_secret_access_key=WS\naws_session_token=TOK\n";
        let d = parse_credentials_file(text, "default").unwrap();
        assert_eq!(d.access_key, "AKID");
        assert_eq!(d.secret_key, "SECRET");
        assert_eq!(d.session_token, None);
        let w = parse_credentials_file(text, "work").unwrap();
        assert_eq!(w.access_key, "W");
        assert_eq!(w.session_token.as_deref(), Some("TOK"));
        assert!(parse_credentials_file(text, "nope").is_none());
    }

    #[test]
    fn uri_encoding_keeps_slashes_and_unreserved() {
        assert_eq!(encode_path("/envio/my app.env"), "/envio/my%20app.env");
        assert_eq!(encode_path("/b/a-b_c.d~e"), "/b/a-b_c.d~e");
        assert_eq!(encode_value("envio/"), "envio%2F");
    }

    #[test]
    fn list_xml_key_extraction() {
        let xml = "<ListBucketResult><Contents><Key>envio/work.env</Key></Contents>\
                   <Contents><Key>envio/notes.txt</Key></Contents>\
                   <Contents><Key>envio/staging.env</Key></Contents></ListBucketResult>";
        assert_eq!(
            keys_from_list_xml(xml, "envio/"),
            vec!["staging".to_string(), "work".to_string()]
        );
    }

    #[test]
    fn error_message_prefers_xml_message() {
        assert_eq!(
            first_line_or_message("<?xml version=\"1.0\"?>\n<Error><Code>AccessDenied</Code><Message>Access Denied</Message></Error>"),
            "Access Denied"
        );
        assert_eq!(first_line_or_message("plain\nsecond"), "plain");
    }
}

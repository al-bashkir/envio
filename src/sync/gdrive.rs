//! Google Drive backend over the Drive v3 REST API, scope `drive.file`.
//!
//! Auth is a user-owned OAuth "Desktop" client plus the device-code flow, so
//! no secrets ship in the binary and login works over SSH.

use std::time::{Duration, Instant};

use reqwest::blocking::{Client, Response};
use serde::Deserialize;

use super::{sha256_hex, RemoteEntry};
use crate::error::{Error, Result};

const DEVICE_URL: &str = "https://oauth2.googleapis.com/device/code";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const SCOPE: &str = "https://www.googleapis.com/auth/drive.file";
const API: &str = "https://www.googleapis.com/drive/v3/files";
const UPLOAD: &str = "https://www.googleapis.com/upload/drive/v3/files";
const FOLDER_MIME: &str = "application/vnd.google-apps.folder";
pub(crate) const BOUNDARY: &str = "envio-sync-boundary-7f3a9c";

fn check(resp: Response, what: &str) -> Result<Response> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    let body = resp.text().unwrap_or_default();
    let msg = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
        .unwrap_or_else(|| body.lines().next().unwrap_or("").to_string());
    Err(Error::Sync(format!(
        "Google Drive {}: {} {}",
        what, status, msg
    )))
}

/// Run the OAuth device-code flow. Prints the verification URL and user code,
/// then polls until the user approves. Returns the refresh token.
pub fn device_login(client_id: &str, client_secret: &str) -> Result<String> {
    #[derive(Deserialize)]
    struct DeviceResp {
        device_code: String,
        user_code: String,
        verification_url: String,
        interval: u64,
        expires_in: u64,
    }
    let client = Client::new();
    let resp = client
        .post(DEVICE_URL)
        .form(&[("client_id", client_id), ("scope", SCOPE)])
        .send()?;
    let d: DeviceResp = check(resp, "device code")?.json()?;

    println!(
        "Open {} and enter the code: {}",
        d.verification_url, d.user_code
    );
    println!("Waiting for approval...");

    let deadline = Instant::now() + Duration::from_secs(d.expires_in);
    let mut interval = Duration::from_secs(d.interval.max(1));
    loop {
        std::thread::sleep(interval);
        if Instant::now() > deadline {
            return Err(Error::Sync(
                "Google login: code expired, run `remote add` again".into(),
            ));
        }
        let v: serde_json::Value = client
            .post(TOKEN_URL)
            .form(&[
                ("client_id", client_id),
                ("client_secret", client_secret),
                ("device_code", d.device_code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()?
            .json()?;
        if let Some(rt) = v["refresh_token"].as_str() {
            return Ok(rt.to_string());
        }
        match v["error"].as_str() {
            Some("authorization_pending") => {}
            Some("slow_down") => interval += Duration::from_secs(5),
            Some(other) => {
                return Err(Error::Sync(format!("Google login failed: {}", other)));
            }
            None => {
                return Err(Error::Sync(
                    "Google login: token response had neither refresh_token nor a recognized error"
                        .into(),
                ))
            }
        }
    }
}

/// Exchange a refresh token for a short-lived access token.
pub fn access_token(client_id: &str, client_secret: &str, refresh_token: &str) -> Result<String> {
    let resp = Client::new()
        .post(TOKEN_URL)
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ])
        .send()?;
    let v: serde_json::Value = check(resp, "token refresh")?.json()?;
    v["access_token"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| Error::Sync("Google token refresh: no access_token in response".into()))
}

/// Create a folder in My Drive and return its id.
pub fn create_folder(access_token: &str, name: &str) -> Result<String> {
    let resp = Client::new()
        .post(API)
        .bearer_auth(access_token)
        .json(&serde_json::json!({ "name": name, "mimeType": FOLDER_MIME }))
        .send()?;
    let v: serde_json::Value = check(resp, "create folder")?.json()?;
    v["id"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| Error::Sync("Google Drive create folder: no id in response".into()))
}

fn q_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

fn file_query(folder_id: &str, name: Option<&str>) -> String {
    let mut q = format!("'{}' in parents and trashed = false", q_escape(folder_id));
    if let Some(name) = name {
        q.push_str(&format!(" and name = '{}.env'", q_escape(name)));
    }
    q
}

fn multipart_body(metadata_json: &str, bytes: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(bytes.len() + 512);
    body.extend_from_slice(
        format!(
            "--{b}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{m}\r\n--{b}\r\nContent-Type: application/octet-stream\r\n\r\n",
            b = BOUNDARY,
            m = metadata_json
        )
        .as_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{}--\r\n", BOUNDARY).as_bytes());
    body
}

#[derive(Deserialize)]
struct DriveFile {
    id: String,
    name: String,
    #[serde(rename = "sha256Checksum")]
    sha256: Option<String>,
}

#[derive(Deserialize)]
struct FileList {
    #[serde(default)]
    files: Vec<DriveFile>,
}

pub struct Drive {
    token: String,
    folder_id: String,
    client: Client,
}

impl Drive {
    pub fn new(
        client_id: &str,
        client_secret: &str,
        refresh_token: &str,
        folder_id: &str,
    ) -> Result<Drive> {
        Ok(Drive {
            token: access_token(client_id, client_secret, refresh_token)?,
            folder_id: folder_id.to_string(),
            client: Client::new(),
        })
    }

    fn files(&self, name: Option<&str>) -> Result<Vec<DriveFile>> {
        // ponytail: single page of 1000. Follow nextPageToken if needed.
        let resp = self
            .client
            .get(API)
            .bearer_auth(&self.token)
            .query(&[
                ("q", file_query(&self.folder_id, name).as_str()),
                ("fields", "files(id,name,sha256Checksum)"),
                ("pageSize", "1000"),
            ])
            .send()?;
        let list: FileList = check(resp, "list")?.json()?;
        Ok(list.files)
    }

    fn find(&self, name: &str) -> Result<Option<String>> {
        Ok(self.files(Some(name))?.into_iter().next().map(|f| f.id))
    }

    fn download(&self, id: &str, what: &str) -> Result<Vec<u8>> {
        let resp = self
            .client
            .get(format!("{}/{}", API, id))
            .bearer_auth(&self.token)
            .query(&[("alt", "media")])
            .send()?;
        Ok(check(resp, what)?.bytes()?.to_vec())
    }

    pub fn list(&self) -> Result<Vec<RemoteEntry>> {
        let mut out = Vec::new();
        for f in self.files(None)? {
            let Some(name) = f.name.strip_suffix(".env") else {
                continue;
            };
            let sha256 = match f.sha256 {
                Some(h) => h,
                None => sha256_hex(&self.download(&f.id, &format!("GET {}", f.name))?),
            };
            out.push(RemoteEntry {
                name: name.to_string(),
                sha256,
            });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    pub fn get(&self, name: &str) -> Result<Vec<u8>> {
        let id = self
            .find(name)?
            .ok_or_else(|| Error::Sync(format!("Google Drive: {}.env not found", name)))?;
        self.download(&id, &format!("GET {}.env", name))
    }

    pub fn put(&self, name: &str, bytes: &[u8]) -> Result<()> {
        let resp = match self.find(name)? {
            Some(id) => self
                .client
                .patch(format!("{}/{}", UPLOAD, id))
                .bearer_auth(&self.token)
                .query(&[("uploadType", "media")])
                .header("Content-Type", "application/octet-stream")
                .body(bytes.to_vec())
                .send()?,
            None => {
                let meta = serde_json::json!({
                    "name": format!("{}.env", name),
                    "parents": [self.folder_id],
                })
                .to_string();
                self.client
                    .post(UPLOAD)
                    .bearer_auth(&self.token)
                    .query(&[("uploadType", "multipart")])
                    .header(
                        "Content-Type",
                        format!("multipart/related; boundary={}", BOUNDARY),
                    )
                    .body(multipart_body(&meta, bytes))
                    .send()?
            }
        };
        check(resp, &format!("PUT {}.env", name))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multipart_body_layout() {
        let body = multipart_body("{\"name\":\"a.env\"}", b"BYTES");
        let text = String::from_utf8_lossy(&body);
        let expected = format!(
            "--{b}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{{\"name\":\"a.env\"}}\r\n\
             --{b}\r\nContent-Type: application/octet-stream\r\n\r\nBYTES\r\n--{b}--\r\n",
            b = BOUNDARY
        );
        assert_eq!(text, expected);
    }

    #[test]
    fn query_escapes_quotes_and_backslashes() {
        assert_eq!(q_escape("it's"), "it\\'s");
        assert_eq!(q_escape("a\\b"), "a\\\\b");
        assert_eq!(q_escape("plain"), "plain");
    }

    #[test]
    fn file_query_shape() {
        assert_eq!(
            file_query("FOLDER", Some("work")),
            "'FOLDER' in parents and trashed = false and name = 'work.env'"
        );
        assert_eq!(
            file_query("FOLDER", None),
            "'FOLDER' in parents and trashed = false"
        );
    }
}

use crate::app_state::APP_DATA_DIR;
use crate::error::{AppError, AppResult};
use crate::file_store;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri_plugin_opener::OpenerExt;

const TOKEN_URI: &str = "https://oauth2.googleapis.com/token";
const AUTH_URI: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const SCOPES: &str = "https://www.googleapis.com/auth/drive.readonly https://www.googleapis.com/auth/documents.readonly";
const GOOGLE_SOURCE_ID: &str = "google-docs-default";
const KEYRING_SERVICE: &str = "com.searchdoc.google";
const CLIENT_SECRET_KEY: &str = "client-secret";
const TOKENS_KEY: &str = "tokens";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleClientConfig {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleTokenSet {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    pub token_type: Option<String>,
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleAuthStatus {
    pub configured: bool,
    pub connected: bool,
    pub has_refresh_token: bool,
    pub source_id: String,
    pub config_path: String,
    pub token_path: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: Option<i64>,
    refresh_token: Option<String>,
    scope: Option<String>,
    token_type: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct StoredClientConfig {
    client_id: String,
}

pub fn google_source_id() -> &'static str {
    GOOGLE_SOURCE_ID
}

pub fn config_path() -> PathBuf {
    APP_DATA_DIR.join("google_oauth.json")
}

pub fn token_path() -> PathBuf {
    APP_DATA_DIR.join("google_tokens.json")
}

fn keyring_entry(key: &str) -> AppResult<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, key)
        .map_err(|err| AppError::msg(format!("无法初始化系统凭据存储：{err}")))
}

fn load_secret(key: &str) -> AppResult<Option<String>> {
    let entry = keyring_entry(key)?;
    match entry.get_password() {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
        Ok(_) | Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(AppError::msg(format!("无法读取系统凭据：{err}"))),
    }
}

fn save_secret(key: &str, value: &str) -> AppResult<()> {
    keyring_entry(key)?
        .set_password(value)
        .map_err(|err| AppError::msg(format!("无法保存系统凭据：{err}")))
}

fn clear_secret(key: &str) -> AppResult<()> {
    let entry = keyring_entry(key)?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(AppError::msg(format!("无法删除系统凭据：{err}"))),
    }
}

pub fn load_client_config() -> AppResult<Option<GoogleClientConfig>> {
    // Prefer env for quick local setup; fall back to config file.
    if let (Ok(client_id), Ok(client_secret)) = (
        std::env::var("SEARCHDOC_GOOGLE_CLIENT_ID"),
        std::env::var("SEARCHDOC_GOOGLE_CLIENT_SECRET"),
    ) {
        if !client_id.trim().is_empty() && !client_secret.trim().is_empty() {
            return Ok(Some(GoogleClientConfig {
                client_id: client_id.trim().to_string(),
                client_secret: client_secret.trim().to_string(),
            }));
        }
    }

    let path = config_path();
    if !path.exists() {
        return Ok(None);
    }
    let value: serde_json::Value = file_store::read_json(&path)?.unwrap_or_default();
    let client_id = value
        .get("client_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if client_id.is_empty() {
        return Ok(None);
    }
    let legacy_secret = value
        .get("client_secret")
        .and_then(serde_json::Value::as_str);
    let client_secret = if let Some(secret) = load_secret(CLIENT_SECRET_KEY)? {
        secret
    } else if let Some(secret) = legacy_secret.filter(|secret| !secret.trim().is_empty()) {
        save_secret(CLIENT_SECRET_KEY, secret.trim())?;
        secret.trim().to_string()
    } else {
        return Ok(None);
    };
    if legacy_secret.is_some() {
        // Remove the legacy plaintext field even when the keyring migration
        // was already completed by an earlier partial run.
        file_store::write_json(
            &path,
            &StoredClientConfig {
                client_id: client_id.clone(),
            },
        )?;
        // The atomic writer backed up the legacy file, which still contained
        // the plaintext secret. Credentials must not survive in recovery files.
        file_store::remove_backup(&path)?;
    }
    Ok(Some(GoogleClientConfig {
        client_id,
        client_secret,
    }))
}

pub fn save_client_config(
    client_id: String,
    client_secret: String,
) -> AppResult<GoogleClientConfig> {
    std::fs::create_dir_all(APP_DATA_DIR.as_path())?;
    let cfg = GoogleClientConfig {
        client_id: client_id.trim().to_string(),
        client_secret: client_secret.trim().to_string(),
    };
    if cfg.client_id.is_empty() || cfg.client_secret.is_empty() {
        return Err(AppError::msg("client_id 与 client_secret 不能为空"));
    }
    save_secret(CLIENT_SECRET_KEY, &cfg.client_secret)?;
    let path = config_path();
    file_store::write_json(
        &path,
        &StoredClientConfig {
            client_id: cfg.client_id.clone(),
        },
    )?;
    file_store::remove_backup(&path)?;
    Ok(cfg)
}

pub fn load_tokens() -> AppResult<Option<GoogleTokenSet>> {
    if let Some(raw) = load_secret(TOKENS_KEY)? {
        return Ok(Some(serde_json::from_str(&raw)?));
    }
    let path = token_path();
    if path.exists() {
        let raw = fs::read_to_string(&path)?;
        let tokens: GoogleTokenSet = serde_json::from_str(&raw)?;
        save_tokens(&tokens)?;
        return Ok(Some(tokens));
    }
    Ok(None)
}

pub fn save_tokens(tokens: &GoogleTokenSet) -> AppResult<()> {
    save_secret(TOKENS_KEY, &serde_json::to_string(tokens)?)?;
    let path = token_path();
    if path.exists() {
        fs::remove_file(&path)?;
    }
    file_store::remove_backup(&path)?;
    Ok(())
}

pub fn clear_tokens() -> AppResult<()> {
    clear_secret(TOKENS_KEY)?;
    let path = token_path();
    if path.exists() {
        fs::remove_file(&path)?;
    }
    file_store::remove_backup(&path)?;
    Ok(())
}

pub fn auth_status() -> AppResult<GoogleAuthStatus> {
    let configured = load_client_config()?.is_some();
    let tokens = load_tokens()?;
    Ok(GoogleAuthStatus {
        configured,
        connected: tokens
            .as_ref()
            .map(|t| !t.access_token.is_empty() || t.refresh_token.is_some())
            .unwrap_or(false),
        has_refresh_token: tokens
            .as_ref()
            .and_then(|t| t.refresh_token.as_ref())
            .map(|s| !s.is_empty())
            .unwrap_or(false),
        source_id: GOOGLE_SOURCE_ID.to_string(),
        config_path: config_path().to_string_lossy().to_string(),
        token_path: "系统凭据库（com.searchdoc.google）".into(),
    })
}

pub fn connect_google_interactive(app: &tauri::AppHandle) -> AppResult<GoogleAuthStatus> {
    let cfg = load_client_config()?.ok_or_else(|| {
        AppError::msg(
            "尚未配置 Google OAuth。请先在「来源」页填写 client_id / client_secret，或设置环境变量 SEARCHDOC_GOOGLE_CLIENT_ID / SEARCHDOC_GOOGLE_CLIENT_SECRET。",
        )
    })?;

    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| AppError::msg(format!("无法启动本地回调服务: {e}")))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| AppError::msg(e.to_string()))?;
    let port = listener
        .local_addr()
        .map_err(|e| AppError::msg(e.to_string()))?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}");

    let state = random_state();
    let code_verifier = random_code_verifier();
    let code_challenge = pkce_challenge(&code_verifier);
    let auth_url = format!(
        "{AUTH_URI}?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent&state={}&code_challenge={}&code_challenge_method=S256",
        urlencoding::encode(&cfg.client_id),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(SCOPES),
        urlencoding::encode(&state),
        urlencoding::encode(&code_challenge),
    );

    open_browser(app, &auth_url)?;

    let (tx, rx) = mpsc::channel::<AppResult<(String, String)>>();
    thread::spawn(move || {
        let _ = tx.send(wait_for_oauth_code(listener, &state));
    });

    let (code, _) = rx
        .recv_timeout(Duration::from_secs(180))
        .map_err(|_| AppError::msg("等待 Google 授权超时（3 分钟）"))??;

    let mut tokens = exchange_code(&cfg, &code, &redirect_uri, &code_verifier)?;
    // Preserve refresh token if Google omits it on re-consent edge cases.
    if tokens.refresh_token.is_none() {
        if let Ok(Some(existing)) = load_tokens() {
            tokens.refresh_token = existing.refresh_token;
        }
    }
    save_tokens(&tokens)?;
    auth_status()
}

pub fn disconnect_google() -> AppResult<GoogleAuthStatus> {
    clear_tokens()?;
    auth_status()
}

pub fn get_valid_access_token() -> AppResult<String> {
    let cfg = load_client_config()?.ok_or_else(|| AppError::msg("Google OAuth 未配置"))?;
    let mut tokens = load_tokens()?.ok_or_else(|| AppError::msg("尚未连接 Google 账号"))?;

    if !token_expired(&tokens) {
        return Ok(tokens.access_token);
    }

    let refresh = tokens.refresh_token.clone().ok_or_else(|| {
        AppError::msg("access token 已过期且没有 refresh token，请重新连接 Google")
    })?;

    let refreshed = refresh_access_token(&cfg, &refresh)?;
    tokens.access_token = refreshed.access_token;
    tokens.expires_at = refreshed.expires_at;
    if refreshed.refresh_token.is_some() {
        tokens.refresh_token = refreshed.refresh_token;
    }
    if refreshed.scope.is_some() {
        tokens.scope = refreshed.scope;
    }
    if refreshed.token_type.is_some() {
        tokens.token_type = refreshed.token_type;
    }
    save_tokens(&tokens)?;
    Ok(tokens.access_token)
}

fn token_expired(tokens: &GoogleTokenSet) -> bool {
    match tokens.expires_at {
        Some(exp) => {
            let now = now_epoch();
            // refresh 60s early
            now >= exp - 60
        }
        None => false,
    }
}

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn random_state() -> String {
    let raw = format!("{}-{}", uuid::Uuid::new_v4(), now_epoch());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw.as_bytes())
}

fn random_code_verifier() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

fn pkce_challenge(verifier: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn open_browser(app: &tauri::AppHandle, url: &str) -> AppResult<()> {
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::msg(format!("无法打开浏览器: {e}")))
}

fn wait_for_oauth_code(listener: TcpListener, expected_state: &str) -> AppResult<(String, String)> {
    let deadline = Instant::now() + Duration::from_secs(175);
    loop {
        if Instant::now() >= deadline {
            return Err(AppError::msg("等待 Google 授权超时（3 分钟）"));
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                if let Some(result) = read_oauth_callback(&mut stream, expected_state)? {
                    return Ok(result);
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(err) => {
                return Err(AppError::msg(format!("OAuth 回调接收失败: {err}")));
            }
        }
    }
}

fn read_oauth_callback(
    stream: &mut std::net::TcpStream,
    expected_state: &str,
) -> AppResult<Option<(String, String)>> {
    let mut buf = [0u8; 8192];
    let n = match stream.read(&mut buf) {
        Ok(0) => return Ok(None),
        Ok(n) => n,
        Err(err)
            if matches!(
                err.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ) =>
        {
            return Ok(None);
        }
        Err(err) => return Err(AppError::msg(format!("读取 OAuth 回调失败: {err}"))),
    };
    let req = String::from_utf8_lossy(&buf[..n]);
    let first_line = req.lines().next().unwrap_or("");
    // GET /?code=...&state=... HTTP/1.1
    let path = first_line.split_whitespace().nth(1).unwrap_or("/");
    let url = format!("http://127.0.0.1{path}");
    let parsed = url::Url::parse(&url).map_err(|e| AppError::msg(e.to_string()))?;

    let mut code = None;
    let mut state = None;
    let mut error = None;
    for (k, v) in parsed.query_pairs() {
        match k.as_ref() {
            "code" => code = Some(v.to_string()),
            "state" => state = Some(v.to_string()),
            "error" => error = Some(v.to_string()),
            _ => {}
        }
    }

    let state = state.unwrap_or_default();
    if state != expected_state {
        let body = html_page("授权失败", "state 校验失败，请重试连接。");
        let _ = write_http_response(stream, 400, &body);
        return Ok(None);
    }
    if let Some(err) = error {
        let body = html_page("授权失败", &format!("Google 返回错误：{err}"));
        let _ = write_http_response(stream, 400, &body);
        return Err(AppError::msg(format!("Google 授权失败: {err}")));
    }
    let Some(code) = code else {
        let body = html_page("授权失败", "回调中缺少授权码，请重试连接。");
        let _ = write_http_response(stream, 400, &body);
        return Ok(None);
    };

    let body = html_page(
        "SearchDoc 已连接 Google",
        "授权成功，可以关闭此页面回到 SearchDoc。",
    );
    let _ = write_http_response(stream, 200, &body);
    Ok(Some((code, state)))
}

fn write_http_response(stream: &mut std::net::TcpStream, status: u16, body: &str) -> AppResult<()> {
    let reason = if status == 200 { "OK" } else { "Bad Request" };
    let resp = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(resp.as_bytes())
        .map_err(|e| AppError::msg(e.to_string()))?;
    let _ = stream.flush();
    Ok(())
}

fn html_page(title: &str, message: &str) -> String {
    let title = html_escape(title);
    let message = html_escape(message);
    format!(
        r#"<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8"><title>{title}</title>
<style>
body{{font-family:Segoe UI,Microsoft YaHei,sans-serif;background:#0f0f0f;color:#f2f2f2;display:grid;place-items:center;height:100vh;margin:0}}
.card{{background:#1a1a1a;border:1px solid rgba(255,255,255,.08);border-radius:12px;padding:28px 32px;max-width:420px}}
h1{{font-size:18px;margin:0 0 10px}} p{{color:#b4b4b4;line-height:1.5;margin:0}}
</style></head>
<body><div class="card"><h1>{title}</h1><p>{message}</p></div></body></html>"#
    )
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Turn raw Google OAuth error payloads into one actionable hint line.
fn oauth_rejection_hint(body: &str) -> String {
    let lowered = body.to_lowercase();
    let hint = if lowered.contains("invalid_client") {
        "Client ID 或 Client Secret 不正确，请在 Google Cloud Console 核对 OAuth 客户端凭据"
    } else if lowered.contains("invalid_grant") {
        "授权码已过期或已被使用：每次授权码几分钟内有效且只能用一次，请重新点击连接"
    } else if lowered.contains("redirect_uri_mismatch") {
        "重定向 URI 不匹配：请在 Google Cloud Console 该 OAuth 客户端中，原样添加应用界面显示的重定向 URI"
    } else if lowered.contains("access_denied") {
        "授权被拒绝：需要同意对应权限才能读取 Google Docs"
    } else if lowered.contains("unauthorized_client") {
        "客户端类型不受支持：请在 Google Cloud Console 创建「桌面应用」类型的 OAuth 客户端"
    } else {
        return String::new();
    };
    format!("\n提示：{hint}")
}

fn exchange_code(
    cfg: &GoogleClientConfig,
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> AppResult<GoogleTokenSet> {
    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(TOKEN_URI)
        .form(&[
            ("code", code),
            ("client_id", cfg.client_id.as_str()),
            ("client_secret", cfg.client_secret.as_str()),
            ("redirect_uri", redirect_uri),
            ("code_verifier", code_verifier),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .map_err(|e| {
            AppError::msg(format!(
                "换取 token 失败: {e}\n提示：请检查网络或系统代理设置后重试"
            ))
        })?;

    if !resp.status().is_success() {
        let text = resp.text().unwrap_or_default();
        return Err(AppError::msg(format!(
            "换取 token 被拒绝: {text}{}",
            oauth_rejection_hint(&text)
        )));
    }

    let parsed: TokenResponse = resp
        .json()
        .map_err(|e| AppError::msg(format!("解析 token 响应失败: {e}")))?;

    Ok(token_response_to_set(parsed))
}

fn refresh_access_token(
    cfg: &GoogleClientConfig,
    refresh_token: &str,
) -> AppResult<GoogleTokenSet> {
    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(TOKEN_URI)
        .form(&[
            ("client_id", cfg.client_id.as_str()),
            ("client_secret", cfg.client_secret.as_str()),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .map_err(|e| {
            AppError::msg(format!(
                "刷新 token 失败: {e}\n提示：请检查网络或系统代理设置；若长期未使用，重新连接一次即可"
            ))
        })?;

    if !resp.status().is_success() {
        let text = resp.text().unwrap_or_default();
        return Err(AppError::msg(format!(
            "刷新 token 被拒绝: {text}{}",
            oauth_rejection_hint(&text)
        )));
    }

    let mut parsed: TokenResponse = resp
        .json()
        .map_err(|e| AppError::msg(format!("解析刷新响应失败: {e}")))?;
    if parsed.refresh_token.is_none() {
        parsed.refresh_token = Some(refresh_token.to_string());
    }
    Ok(token_response_to_set(parsed))
}

fn token_response_to_set(parsed: TokenResponse) -> GoogleTokenSet {
    let expires_at = parsed.expires_in.map(|secs| now_epoch() + secs);
    GoogleTokenSet {
        access_token: parsed.access_token,
        refresh_token: parsed.refresh_token,
        expires_at,
        token_type: parsed.token_type,
        scope: parsed.scope,
    }
}

#[cfg(test)]
mod tests {
    use super::{html_page, pkce_challenge, random_code_verifier};

    #[test]
    fn pkce_values_use_safe_lengths_and_characters() {
        let verifier = random_code_verifier();
        assert!((43..=128).contains(&verifier.len()));
        assert!(verifier.chars().all(|ch| ch.is_ascii_alphanumeric()));
        let challenge = pkce_challenge(&verifier);
        assert_eq!(challenge.len(), 43);
        assert!(!challenge.contains('='));
    }

    #[test]
    fn oauth_callback_page_escapes_reflected_text() {
        let page = html_page("失败", "<script>alert('x')</script>");
        assert!(!page.contains("<script>"));
        assert!(page.contains("&lt;script&gt;"));
    }
}

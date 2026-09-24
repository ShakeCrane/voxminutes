//! User-configured local OpenAI-compatible model servers.
//! Models run in the user's own server (OpenVINO, llama.cpp, Ollama, etc.).
//! This module never launches untrusted executables or claims hardware support.
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};
use crate::database::repositories::setting::SettingsRepository;
use crate::state::AppState;

const PROFILES_KEY: &str = "custom_local.profiles";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelProfile {
    pub id: String,
    pub name: String,
    /// asr | translation | summary
    pub task: String,
    /// OpenAI-compatible base URL, e.g. http://127.0.0.1:8000/v1
    pub endpoint: String,
    pub model: String,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}
fn default_timeout() -> u64 { 120 }

/// Only loopback HTTP is supported; no network audio or text is sent to LAN/cloud.
/// URL parsing (not string-prefix checks) prevents userinfo / deceptive hosts.
pub fn validate_endpoint(endpoint: &str) -> Result<String, String> {
    let u = url::Url::parse(endpoint.trim()).map_err(|e| format!("Invalid URL: {e}"))?;
    if u.scheme() != "http" || u.username() != "" || u.password().is_some()
        || u.query().is_some() || u.fragment().is_some() {
        return Err("Use a plain http://localhost (or 127.0.0.1 / [::1]) endpoint without credentials, query or fragment".into());
    }
    let loopback = match u.host() {
        Some(url::Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        _ => false,
    };
    if !loopback { return Err("Custom model servers must be bound to loopback; LAN/cloud endpoints are not supported here".into()); }
    let path = u.path().trim_end_matches('/');
    if path != "" && path != "/" && path != "/v1" {
        return Err("Endpoint must be the server root or /v1 (not a full API method URL)".into());
    }
    Ok(endpoint.trim().trim_end_matches('/').to_string())
}

pub fn validate_profile(p: &LocalModelProfile) -> Result<(), String> {
    if !matches!(p.task.as_str(), "asr" | "translation" | "summary") {
        return Err("Task must be asr, translation or summary".into());
    }
    if p.name.trim().is_empty() || p.name.len() > 128 || p.model.trim().is_empty() || p.model.len() > 256 {
        return Err("Model name and model ID must be nonempty and reasonably short".into());
    }
    if p.id.len() > 80 || (!p.id.is_empty() && !p.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')) {
        return Err("Invalid profile ID".into());
    }
    if !(1..=3600).contains(&p.timeout_secs) { return Err("Timeout must be 1–3600 seconds".into()); }
    validate_endpoint(&p.endpoint)?;
    Ok(())
}

fn client(timeout: u64) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(timeout))
        .redirect(reqwest::redirect::Policy::none())
        .build().map_err(|e| e.to_string())
}
async fn read_profiles(pool: &sqlx::SqlitePool) -> Result<Vec<LocalModelProfile>, String> {
    let raw = SettingsRepository::get(pool, PROFILES_KEY).await.map_err(|e| e.to_string())?;
    match raw {
        Some(s) => serde_json::from_str(&s).map_err(|e| format!("Invalid profile store: {e}")),
        None => Ok(Vec::new()),
    }
}
pub async fn profile<R: Runtime>(app: &AppHandle<R>, id: &str, task: &str) -> Result<LocalModelProfile, String> {
    let state = app.state::<AppState>();
    let profiles = read_profiles(state.db_manager.pool()).await?;
    profiles.into_iter().find(|p| p.id == id && p.task == task)
        .ok_or_else(|| format!("No {task} profile with ID {id}"))
}

#[tauri::command]
pub async fn custom_local_list(state: tauri::State<'_, AppState>) -> Result<Vec<LocalModelProfile>, String> {
    read_profiles(state.db_manager.pool()).await
}
#[tauri::command]
pub async fn custom_local_upsert(
    state: tauri::State<'_, AppState>, mut profile: LocalModelProfile
) -> Result<LocalModelProfile, String> {
    validate_profile(&profile)?;
    profile.endpoint = validate_endpoint(&profile.endpoint)?;
    if profile.id.is_empty() { profile.id = uuid::Uuid::new_v4().to_string(); }
    let pool = state.db_manager.pool();
    let mut items = read_profiles(pool).await?;
    if let Some(existing) = items.iter_mut().find(|p| p.id == profile.id) {
        if existing.task != profile.task { return Err("Cannot change task of an existing profile".into()); }
        *existing = profile.clone();
    } else {
        items.push(profile.clone());
    }
    let serialized = serde_json::to_string(&items).map_err(|e| e.to_string())?;
    SettingsRepository::set(pool, PROFILES_KEY, &serialized).await.map_err(|e| e.to_string())?;
    Ok(profile)
}
#[tauri::command]
pub async fn custom_local_delete(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    let pool = state.db_manager.pool();
    let mut items = read_profiles(pool).await?;
    items.retain(|p| p.id != id);
    let serialized = serde_json::to_string(&items).map_err(|e| e.to_string())?;
    SettingsRepository::set(pool, PROFILES_KEY, &serialized).await.map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn custom_local_select(
    state: tauri::State<'_, AppState>, id: String
) -> Result<(), String> {
    let pool = state.db_manager.pool();
    let p = read_profiles(pool).await?.into_iter().find(|p| p.id == id)
        .ok_or_else(|| "Profile not found".to_string())?;
    match p.task.as_str() {
        "asr" => {
            SettingsRepository::set(pool, "transcript.provider", "custom-local").await.map_err(|e| e.to_string())?;
            SettingsRepository::set(pool, "transcript.model", &id).await.map_err(|e| e.to_string())?;
        }
        "translation" => {
            let engine = format!("custom:{id}");
            SettingsRepository::set(pool, "translation.engine", &engine).await.map_err(|e| e.to_string())?;
            *crate::translation::TRANSLATION_ENGINE.lock().map_err(|e| e.to_string())? = engine;
        }
        "summary" => return Err("Select a summary profile in the summary dialog".into()),
        _ => return Err("Unsupported task".into()),
    }
    Ok(())
}
#[tauri::command]
pub async fn custom_local_test(profile: LocalModelProfile) -> Result<String, String> {
    validate_profile(&profile)?;
    let c = client(profile.timeout_secs.min(15))?;
    let url = format!("{}/models", validate_endpoint(&profile.endpoint)?);
    let r = c.get(&url).send().await.map_err(|e| e.to_string())?;
    if !r.status().is_success() { return Err(format!("GET /models returned HTTP {}", r.status())); }
    Ok("Local model endpoint responded successfully".into())
}

/// Little-endian, 16 kHz mono PCM WAV; the TranscriptionProvider contract supplies f32.
fn wav16(audio: &[f32]) -> Result<Vec<u8>, String> {
    let data_len = audio.len().checked_mul(2).ok_or("Audio is too long")?;
    let size = u32::try_from(data_len).map_err(|_| "Audio is too long")?;
    let mut out = Vec::with_capacity(44 + data_len);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(size + 36).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&16000u32.to_le_bytes());
    out.extend_from_slice(&32000u32.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&size.to_le_bytes());
    for sample in audio {
        let val = (sample.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        out.extend_from_slice(&val.to_le_bytes());
    }
    Ok(out)
}
pub async fn transcribe(p: &LocalModelProfile, audio: &[f32], language: Option<String>) -> Result<String, String> {
    let url = format!("{}/audio/transcriptions", validate_endpoint(&p.endpoint)?);
    let wav = wav16(audio)?;
    let file = reqwest::multipart::Part::bytes(wav).file_name("audio.wav")
        .mime_str("audio/wav").map_err(|e| e.to_string())?;
    let mut form = reqwest::multipart::Form::new()
        .part("file", file).text("model", p.model.clone());
    if let Some(lang) = language.filter(|l| !l.is_empty()) { form = form.text("language", lang); }
    let r = client(p.timeout_secs)?.post(url).multipart(form).send().await.map_err(|e| e.to_string())?;
    let status = r.status();
    let bytes = r.bytes().await.map_err(|e| e.to_string())?;
    if !status.is_success() { return Err(format!("ASR HTTP {status}: {}", String::from_utf8_lossy(&bytes).chars().take(300).collect::<String>())); }
    let v: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    v.get("text").and_then(|t| t.as_str()).map(str::to_string)
        .ok_or_else(|| "ASR response has no text field".into())
}
pub async fn translate(p: &LocalModelProfile, text: &str, src: &str, tgt: &str) -> Result<String, String> {
    let url = format!("{}/chat/completions", validate_endpoint(&p.endpoint)?);
    let body = serde_json::json!({
        "model":p.model,"stream":false,"temperature":0,
        "messages":[
            {"role":"system","content":format!("Translate from {src} to {tgt}. Return only the translation, with no commentary.")},
            {"role":"user","content":text}
        ]
    });
    let r = client(p.timeout_secs)?.post(url).json(&body).send().await.map_err(|e| e.to_string())?;
    let status = r.status();
    let bytes = r.bytes().await.map_err(|e| e.to_string())?;
    if !status.is_success() { return Err(format!("Translation HTTP {status}: {}",String::from_utf8_lossy(&bytes).chars().take(300).collect::<String>())); }
    let v: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    v.pointer("/choices/0/message/content").and_then(|t| t.as_str())
        .map(str::to_string).ok_or_else(|| "Chat response has no message content".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoints_never_escape_loopback() {
        for bad in ["https://127.0.0.1:8000/v1", "http://127.0.0.1.evil.com/v1",
            "http://127.0.0.1@evil.com/v1", "http://192.168.1.2:8000/v1",
            "http://localhost:8000/v1?next=evil", "http://localhost:8000/redirect"] {
            assert!(validate_endpoint(bad).is_err(), "{bad}");
        }
        assert!(validate_endpoint("http://127.0.0.1:8000/v1").is_ok());
        assert!(validate_endpoint("http://[::1]:8000/v1").is_ok());
    }
    #[test]
    fn wav_header_contains_actual_length() {
        let wav = wav16(&[0.0, 1.0, -1.0]).unwrap();
        assert_eq!(&wav[..4],b"RIFF");
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()),6);
        assert_eq!(wav.len(),50);
    }
}

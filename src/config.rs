//! Runtime configuration for Amir Tools Rust Bot.
//! Reads from environment variables or `.env`, with built-in defaults for
//! @Game_iran_amir_bot so it runs out of the box with zero manual config.

use std::collections::HashMap;
use std::path::PathBuf;

pub const DEFAULT_BOT_TOKEN: &str = "8496115477:AAEKo8aKq5NVW2xC6aA7V5Q7HE6jE2fHPis";
pub const DEFAULT_OWNER_ID: i64 = 7243980602;
pub const DEFAULT_BOT_USERNAME: &str = "Game_iran_amir_bot";
pub const DEFAULT_CF_WORKER_URL: &str = "https://amir-tools-bot.amirjam684.workers.dev";
pub const DEFAULT_AI_BASE_URL: &str = "https://ninerouter-latest-brbz.onrender.com/v1";
pub const DEFAULT_AI_API_KEY: &str = "sk-d30040332bd8c421-92hziv-a7eecbb3";
pub const DEFAULT_AI_MODEL: &str = "hermes";

#[derive(Clone, Debug)]
pub struct Config {
    pub bot_token: String,
    pub owner_id: i64,
    pub tg_api_base: String,
    pub max_upload_mb: u64,
    pub cf_worker_url: String,
    pub ai_base_url: String,
    pub ai_api_key: String,
    pub ai_model: String,
    pub gemini_api_key: String,
    pub gemini_model: String,
    pub data_dir: PathBuf,
    pub ytdlp_bin: String,
    pub ffmpeg_bin: String,
    pub bot_username: String,
    pub group_char_limit: usize,
    pub workers: usize,
}

fn load_dotenv() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut candidates = vec![PathBuf::from(".env")];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(".env"));
        }
    }
    for p in candidates {
        if let Ok(text) = std::fs::read_to_string(&p) {
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((k, v)) = line.split_once('=') {
                    let v = v.trim().trim_matches('"').trim_matches('\'');
                    map.entry(k.trim().to_string()).or_insert_with(|| v.to_string());
                }
            }
        }
    }
    map
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let dot = load_dotenv();
        let get = |k: &str, def: &str| -> String {
            std::env::var(k)
                .ok()
                .filter(|v| !v.trim().is_empty())
                .or_else(|| dot.get(k).cloned().filter(|v| !v.trim().is_empty()))
                .unwrap_or_else(|| def.to_string())
        };

        let bot_token = get("BOT_TOKEN", DEFAULT_BOT_TOKEN);
        let owner_id: i64 = get("OWNER_ID", &DEFAULT_OWNER_ID.to_string())
            .parse()
            .unwrap_or(DEFAULT_OWNER_ID);
        let data_dir = PathBuf::from(get("DATA_DIR", "./data"));
        let _ = std::fs::create_dir_all(&data_dir);
        let _ = std::fs::create_dir_all(data_dir.join("tmp"));

        let tg_api_base = get("TG_API_BASE", "https://api.telegram.org")
            .trim_end_matches('/')
            .to_string();
        let default_limit = if tg_api_base.contains("api.telegram.org") {
            "49"
        } else {
            "1990"
        };

        Ok(Self {
            bot_token,
            owner_id,
            tg_api_base,
            max_upload_mb: get("MAX_UPLOAD_MB", default_limit).parse().unwrap_or(49),
            cf_worker_url: get("CF_WORKER_URL", DEFAULT_CF_WORKER_URL)
                .trim_end_matches('/')
                .to_string(),
            ai_base_url: get("AI_BASE_URL", DEFAULT_AI_BASE_URL)
                .trim_end_matches('/')
                .to_string(),
            ai_api_key: get("AI_API_KEY", DEFAULT_AI_API_KEY),
            ai_model: get("AI_MODEL", DEFAULT_AI_MODEL),
            gemini_api_key: get("GEMINI_API_KEY", ""),
            gemini_model: get("GEMINI_MODEL", "gemini-2.5-flash"),
            data_dir,
            ytdlp_bin: get("YTDLP_BIN", "yt-dlp"),
            ffmpeg_bin: get("FFMPEG_BIN", "ffmpeg"),
            bot_username: get("BOT_USERNAME", DEFAULT_BOT_USERNAME),
            group_char_limit: get("GROUP_CHAR_LIMIT", "100").parse().unwrap_or(100),
            workers: get("WORKERS", "32").parse().unwrap_or(32),
        })
    }

    pub fn tmp_dir(&self) -> PathBuf {
        self.data_dir.join("tmp")
    }

    pub fn max_upload_bytes(&self) -> u64 {
        self.max_upload_mb * 1024 * 1024
    }
}

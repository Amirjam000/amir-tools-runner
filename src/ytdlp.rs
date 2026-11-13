//! Real YouTube / Instagram / TikTok / SoundCloud / Music downloader powered
//! by `yt-dlp` (with automatic binary bootstrap if missing on the server) and
//! `ffmpeg`, streaming finished MP4/MP3 files directly to Telegram.

use crate::config::Config;
use crate::db::Store;
use crate::tg::{html_escape, InlineBtn, Keyboard, TgClient};
use anyhow::{Context, Result};
use regex::Regex;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaMode {
    Video720,
    Video360,
    VideoBest,
    AudioMp3,
}

#[derive(Debug, Deserialize, Default)]
pub struct YtInfo {
    #[serde(default)]
    #[allow(dead_code)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    pub uploader: Option<String>,
    pub channel: Option<String>,
    pub duration: Option<f64>,
    pub view_count: Option<u64>,
    pub webpage_url: Option<String>,
}

/// Detects YouTube, Instagram, TikTok, SoundCloud, Twitter/X, or Reddit media URLs in a message.
pub fn detect_media_url(text: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(
            r"(?i)https?://(?:www\.|m\.)?(?:youtube\.com/(?:watch\?[^\s]+|shorts/[^\s/?#]+|live/[^\s/?#]+)|youtu\.be/[^\s/?#]+|music\.youtube\.com/watch\?[^\s]+|instagram\.com/(?:reel|p|tv)/[^\s/?#]+|tiktok\.com/[^\s]+|vm\.tiktok\.com/[^\s]+|soundcloud\.com/[^\s]+|(?:twitter|x)\.com/[^\s]+/status/\d+)",
        )
        .unwrap()
    });
    re.find(text)
        .map(|m| m.as_str().trim_end_matches(&['.', ',', ')', ']'][..]).to_string())
}

/// Ensures a working `yt-dlp` binary is available. If the system `yt-dlp` is not
/// in PATH, automatically downloads the official standalone binary into `./data/`.
pub async fn resolve_ytdlp_bin(cfg: &Config, http: &reqwest::Client) -> String {
    // 1. Check configured binary in PATH
    if Command::new(&cfg.ytdlp_bin)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return cfg.ytdlp_bin.clone();
    }

    // 2. Check cached binary in data_dir
    let local_name = if cfg!(windows) { "yt-dlp.exe" } else { "yt-dlp" };
    let local_path = cfg.data_dir.join(local_name);
    if local_path.exists() {
        return local_path.to_string_lossy().to_string();
    }

    // 3. Auto-download official yt-dlp standalone binary
    let dl_url = if cfg!(windows) {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
    } else if cfg!(target_os = "macos") {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos"
    } else {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux"
    };

    if let Ok(resp) = http
        .get(dl_url)
        .timeout(Duration::from_secs(90))
        .send()
        .await
    {
        if resp.status().is_success() {
            if let Ok(bytes) = resp.bytes().await {
                if tokio::fs::write(&local_path, &bytes).await.is_ok() {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        if let Ok(meta) = std::fs::metadata(&local_path) {
                            let mut perms = meta.permissions();
                            perms.set_mode(0o755);
                            let _ = std::fs::set_permissions(&local_path, perms);
                        }
                    }
                    return local_path.to_string_lossy().to_string();
                }
            }
        }
    }

    cfg.ytdlp_bin.clone()
}

pub fn normalize_yt_target(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("ytsearch")
    {
        trimmed.to_string()
    } else {
        format!("ytsearch1:{}", trimmed)
    }
}

pub async fn fetch_info(cfg: &Config, http: &reqwest::Client, url_or_query: &str) -> Result<YtInfo> {
    let bin = resolve_ytdlp_bin(cfg, http).await;
    let target = normalize_yt_target(url_or_query);

    let out = tokio::time::timeout(
        Duration::from_secs(45),
        Command::new(&bin)
            .args([
                "--dump-single-json",
                "--no-playlist",
                "--no-warnings",
                "--geo-bypass",
                "--",
                &target,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .context("yt-dlp metadata timeout")?
    .with_context(|| format!("failed to spawn `{}`", bin))?;

    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!(
            "yt-dlp نتوانست اطلاعات لینک را دریافت کند:\n{}",
            err.lines().last().unwrap_or("خطای ناشناخته")
        );
    }

    let val: serde_json::Value =
        serde_json::from_slice(&out.stdout).context("parse yt-dlp JSON")?;
    // If target was `ytsearch1:...`, the entry is inside `entries[0]`
    if let Some(entries) = val.get("entries").and_then(|e| e.as_array()) {
        if let Some(first) = entries.first() {
            let info: YtInfo = serde_json::from_value(first.clone())?;
            return Ok(info);
        }
    }

    let info: YtInfo = serde_json::from_value(val)?;
    Ok(info)
}

/// Presents an interactive YouTube / Media card with quality and MP3 selection buttons.
pub async fn present_card(
    tg: &TgClient,
    cfg: &Config,
    store: &Store,
    chat_id: i64,
    reply_to: i64,
    url_or_query: &str,
) -> Result<()> {
    let wait = tg
        .send_html(
            chat_id,
            Some(reply_to),
            "🔎 <b>در حال استخراج اطلاعات ویدیو/موزیک با <code>yt-dlp</code>...</b>",
            None,
        )
        .await?;

    let target = normalize_yt_target(url_or_query);
    let key = store.stash_url(&target).await;

    let text = match fetch_info(cfg, &tg.http, &target).await {
        Ok(info) => {
            // If it was a search query and we resolved a direct webpage_url, stash that too
            if let Some(ref web) = info.webpage_url {
                if !web.is_empty() {
                    let _ = store.stash_url(web).await;
                }
            }
            let ch = info
                .uploader
                .or(info.channel)
                .unwrap_or_else(|| "نامشخص".to_string());
            let dur = info
                .duration
                .map(|d| format!("{}:{:02}", (d as u64) / 60, (d as u64) % 60))
                .unwrap_or_else(|| "—".to_string());
            let views = info
                .view_count
                .map(fmt_num)
                .unwrap_or_else(|| "—".to_string());
            let link_display = info.webpage_url.as_deref().unwrap_or(url_or_query);
            format!(
                "🎬 <b>{}</b>\n\n\
                 👤 کانال: <b>{}</b>\n\
                 ⏱ مدت زمان: <code>{}</code>  |  👁 بازدید: <code>{}</code>\n\
                 🔗 <code>{}</code>\n\n\
                 👇 <b>کیفیت مورد نظر برای دانلود مستقیم با <code>yt-dlp</code> را انتخاب کنید:</b>",
                html_escape(if info.title.is_empty() {
                    url_or_query
                } else {
                    &info.title
                }),
                html_escape(&ch),
                dur,
                views,
                html_escape(link_display)
            )
        }
        Err(_) => format!(
            "🎬 <b>لینک/جستجوی رسانه شناسایی شد</b>\n\
             🔗 <code>{}</code>\n\n\
             👇 <b>کیفیت مورد نظر برای دانلود مستقیم با <code>yt-dlp</code> را انتخاب کنید:</b>",
            html_escape(url_or_query)
        ),
    };

    let kb: Keyboard = vec![
        vec![
            InlineBtn::cb("🎬 ویدیو 720p (MP4)", format!("ytdl:v720:{}", key)),
            InlineBtn::cb("📱 ویدیو 360p (کم‌حجم)", format!("ytdl:v360:{}", key)),
        ],
        vec![
            InlineBtn::cb("🔥 بهترین کیفیت (Best)", format!("ytdl:vbest:{}", key)),
            InlineBtn::cb("🎵 فقط صدا / موزیک (MP3)", format!("ytdl:mp3:{}", key)),
        ],
        vec![InlineBtn::cb("🏠 بازگشت به منوی اصلی", "menu:home")],
    ];

    tg.edit_html(chat_id, wait.message_id, &text, Some(kb))
        .await?;
    Ok(())
}

/// Runs `yt-dlp` to download the requested video or audio and uploads it directly to Telegram.
pub async fn run_download(
    tg: &TgClient,
    cfg: &Config,
    store: &Store,
    chat_id: i64,
    user_id: i64,
    status_msg_id: i64,
    url_or_query: &str,
    mode: MediaMode,
) -> Result<()> {
    let mode_label = match mode {
        MediaMode::Video720 => "ویدیو 720p (MP4)",
        MediaMode::Video360 => "ویدیو 360p (MP4)",
        MediaMode::VideoBest => "بهترین کیفیت ویدیو (MP4)",
        MediaMode::AudioMp3 => "فایل صوتی (MP3 192kbps)",
    };

    let _ = tg
        .edit_html(
            chat_id,
            status_msg_id,
            &format!(
                "⏳ <b>در حال دانلود {} با <code>yt-dlp</code>...</b>\n\
                 🔗 <code>{}</code>\n\n\
                 <i>لطفاً چند لحظه صبر کنید؛ فایل مستقیماً در همین چت آپلود می‌شود.</i>",
                mode_label,
                html_escape(url_or_query)
            ),
            None,
        )
        .await;

    let bin = resolve_ytdlp_bin(cfg, &tg.http).await;
    let target = normalize_yt_target(url_or_query);

    let job_id = format!(
        "yt_{}_{}",
        user_id,
        chrono::Utc::now().timestamp_millis().unsigned_abs()
    );
    let work_dir = cfg.tmp_dir().join(&job_id);
    tokio::fs::create_dir_all(&work_dir).await?;
    let _cleanup_guard = TempCleanGuard(work_dir.clone());

    let out_tmpl = work_dir.join("%(title).75B.%(ext)s");
    let out_tmpl_str = out_tmpl.to_string_lossy().to_string();
    let max_size_str = format!("{}M", cfg.max_upload_mb);

    let mut args: Vec<String> = vec![
        "--no-playlist".into(),
        "--no-warnings".into(),
        "--geo-bypass".into(),
        "--no-mtime".into(),
        "--restrict-filenames".into(),
        "--max-filesize".into(),
        max_size_str,
        "-o".into(),
        out_tmpl_str,
    ];

    // Check if ffmpeg is available for stream merging / mp3 conversion
    let has_ffmpeg = Command::new(&cfg.ffmpeg_bin)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false);

    match mode {
        MediaMode::AudioMp3 => {
            if has_ffmpeg {
                args.extend([
                    "-x".into(),
                    "--audio-format".into(),
                    "mp3".into(),
                    "--audio-quality".into(),
                    "192K".into(),
                ]);
            } else {
                args.extend(["-f".into(), "bestaudio[ext=m4a]/bestaudio/best".into()]);
            }
        }
        MediaMode::Video360 => {
            let fmt = if has_ffmpeg {
                "bestvideo[height<=360][ext=mp4]+bestaudio[ext=m4a]/best[height<=360][ext=mp4]/best[height<=480]/best"
            } else {
                "best[height<=360][ext=mp4]/best[height<=480][ext=mp4]/best"
            };
            args.extend([
                "-f".into(),
                fmt.into(),
                "--merge-output-format".into(),
                "mp4".into(),
            ]);
        }
        MediaMode::Video720 => {
            let fmt = if has_ffmpeg {
                "bestvideo[height<=720][ext=mp4]+bestaudio[ext=m4a]/best[height<=720][ext=mp4]/best[height<=720]/best"
            } else {
                "best[height<=720][ext=mp4]/best[ext=mp4]/best"
            };
            args.extend([
                "-f".into(),
                fmt.into(),
                "--merge-output-format".into(),
                "mp4".into(),
            ]);
        }
        MediaMode::VideoBest => {
            let fmt = if has_ffmpeg {
                "bestvideo[height<=1080][ext=mp4]+bestaudio[ext=m4a]/best[ext=mp4]/best"
            } else {
                "best[ext=mp4]/best"
            };
            args.extend([
                "-f".into(),
                fmt.into(),
                "--merge-output-format".into(),
                "mp4".into(),
            ]);
        }
    }
    args.push("--".into());
    args.push(target.clone());

    let out = tokio::time::timeout(
        Duration::from_secs(360),
        Command::new(&bin)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .context("مهلت زمانی دانلود yt-dlp به پایان رسید")?
    .with_context(|| format!("اجرای `{}` ناموفق بود", bin))?;

    let downloaded_file = find_downloaded_file(&work_dir).await;
    let file_path = match downloaded_file {
        Some(p) => p,
        None => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let stdout = String::from_utf8_lossy(&out.stdout);
            let combined = format!("{}\n{}", stderr, stdout);
            let tail: Vec<&str> = combined
                .lines()
                .filter(|l| !l.trim().is_empty())
                .rev()
                .take(4)
                .collect();
            let err_msg = tail.into_iter().rev().collect::<Vec<_>>().join("\n");
            let _ = tg
                .edit_html(
                    chat_id,
                    status_msg_id,
                    &format!(
                        "❌ <b>خطا در دانلود با <code>yt-dlp</code>:</b>\n\
                         <pre>{}</pre>\n\
                         💡 <i>اگر حجم ویدیو بیش از {} مگابایت است، گزینه ۳۶۰p یا MP3 را امتحان کنید.</i>",
                        html_escape(if err_msg.is_empty() {
                            "فایلی تولید نشد (ممکن است ویدیو محدودیت سنی/منطقه‌ای داشته باشد یا حجم آن بیش از سقف مجاز باشد)."
                        } else {
                            &err_msg
                        }),
                        cfg.max_upload_mb
                    ),
                    None,
                )
                .await;
            return Ok(());
        }
    };

    let size = tokio::fs::metadata(&file_path).await?.len();
    if size > cfg.max_upload_bytes() {
        let _ = tg
            .edit_html(
                chat_id,
                status_msg_id,
                &format!(
                    "⚠️ <b>حجم فایل دانلود شده ({:.1} MB) بیشتر از سقف آپلود تلگرام ({} MB) است.</b>\n\
                     لطفاً کیفیت <b>360p</b> یا <b>MP3</b> را انتخاب کنید.",
                    size as f64 / 1_048_576.0,
                    cfg.max_upload_mb
                ),
                None,
            )
            .await;
        return Ok(());
    }

    let fname = file_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("media")
        .to_string();

    let _ = tg
        .edit_html(
            chat_id,
            status_msg_id,
            &format!(
                "📤 <b>در حال آپلود در تلگرام...</b>\n\
                 📁 <code>{}</code> ({:.1} MB)",
                html_escape(&fname),
                size as f64 / 1_048_576.0
            ),
            None,
        )
        .await;

    let ext = file_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let (action, method, field) =
        if mode == MediaMode::AudioMp3 || matches!(ext.as_str(), "mp3" | "m4a" | "ogg" | "opus" | "flac") {
            ("upload_voice", "sendAudio", "audio")
        } else if matches!(ext.as_str(), "mp4" | "mkv" | "webm" | "mov") {
            ("upload_video", "sendVideo", "video")
        } else {
            ("upload_document", "sendDocument", "document")
        };

    tg.send_action(chat_id, action).await;

    let caption = format!(
        "✅ <b>{}</b>\n📦 حجم: <code>{:.1} MB</code>  |  ⚡️ دانلودر <code>yt-dlp</code> امیر تولز 🦀",
        html_escape(&fname),
        size as f64 / 1_048_576.0
    );

    tg.send_file(chat_id, None, &file_path, &caption, method, field, None)
        .await?;
    tg.delete(chat_id, status_msg_id).await;
    store.inc_download(user_id).await;
    Ok(())
}

async fn find_downloaded_file(dir: &Path) -> Option<PathBuf> {
    let mut rd = tokio::fs::read_dir(dir).await.ok()?;
    let mut best: Option<(PathBuf, u64)> = None;
    while let Ok(Some(entry)) = rd.next_entry().await {
        let p = entry.path();
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name.ends_with(".part") || name.ends_with(".ytdl") || name.ends_with(".temp") {
            continue;
        }
        if let Ok(meta) = entry.metadata().await {
            if meta.is_file() && meta.len() > 0 {
                if best.as_ref().map(|(_, s)| meta.len() > *s).unwrap_or(true) {
                    best = Some((p, meta.len()));
                }
            }
        }
    }
    best.map(|(p, _)| p)
}

fn fmt_num(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}

struct TempCleanGuard(PathBuf);
impl Drop for TempCleanGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

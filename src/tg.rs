//! Full asynchronous Telegram Bot API 8.x client with streaming multipart
//! uploads (`sendVideo`, `sendAudio`, `sendDocument`, `sendPhoto`, `sendVoice`),
//! animated message effects, expandable blockquotes, and group administration.

use anyhow::{Context, Result};
use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;
use tokio::fs::File;
use tokio_util::io::ReaderStream;

pub const EFFECT_FIRE: &str = "5107584321108051014";
pub const EFFECT_PARTY: &str = "5046509860389126442";
pub const EFFECT_HEART: &str = "5159385139981059251";
#[allow(dead_code)]
pub const EFFECT_THUMBS_UP: &str = "5104841245755180586";

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct User {
    pub id: i64,
    #[serde(default)]
    pub is_bot: bool,
    #[serde(default)]
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Chat {
    pub id: i64,
    #[serde(rename = "type", default)]
    pub kind: String,
    pub title: Option<String>,
    pub username: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FileMeta {
    pub file_id: String,
    pub file_size: Option<u64>,
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PhotoSize {
    pub file_id: String,
    pub width: u32,
    pub height: u32,
    pub file_size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Message {
    pub message_id: i64,
    #[serde(default)]
    pub date: i64,
    #[serde(default)]
    pub chat: Chat,
    pub from: Option<User>,
    pub text: Option<String>,
    pub caption: Option<String>,
    pub reply_to_message: Option<Box<Message>>,
    pub new_chat_members: Option<Vec<User>>,
    pub left_chat_member: Option<User>,
    pub document: Option<FileMeta>,
    pub video: Option<FileMeta>,
    pub audio: Option<FileMeta>,
    pub voice: Option<FileMeta>,
    pub photo: Option<Vec<PhotoSize>>,
    pub forward_origin: Option<Value>,
    pub forward_from: Option<User>,
    pub forward_from_chat: Option<Chat>,
}

impl Message {
    pub fn body(&self) -> &str {
        self.text
            .as_deref()
            .or(self.caption.as_deref())
            .unwrap_or("")
    }

    pub fn is_forwarded(&self) -> bool {
        self.forward_origin.is_some()
            || self.forward_from.is_some()
            || self.forward_from_chat.is_some()
    }

    pub fn has_media(&self) -> bool {
        self.photo.is_some()
            || self.video.is_some()
            || self.document.is_some()
            || self.audio.is_some()
            || self.voice.is_some()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CallbackQuery {
    pub id: String,
    pub from: User,
    pub message: Option<Message>,
    pub data: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Update {
    pub update_id: i64,
    pub message: Option<Message>,
    pub edited_message: Option<Message>,
    pub callback_query: Option<CallbackQuery>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InlineBtn {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl InlineBtn {
    pub fn cb(text: impl Into<String>, data: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            callback_data: Some(data.into()),
            url: None,
        }
    }

    pub fn url(text: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            callback_data: None,
            url: Some(url.into()),
        }
    }
}

pub type Keyboard = Vec<Vec<InlineBtn>>;

#[derive(Clone)]
pub struct TgClient {
    pub http: reqwest::Client,
    base: String,
    file_base: String,
}

impl TgClient {
    pub fn new(api_base: &str, token: &str) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(600))
            .connect_timeout(Duration::from_secs(15))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AmirToolsRust/6.0")
            .build()?;
        let base_trim = api_base.trim_end_matches('/');
        Ok(Self {
            http,
            base: format!("{}/bot{}", base_trim, token),
            file_base: format!("{}/file/bot{}", base_trim, token),
        })
    }

    pub async fn call(&self, method: &str, body: &Value) -> Result<Value> {
        let url = format!("{}/{}", self.base, method);
        let resp: Value = self
            .http
            .post(&url)
            .json(body)
            .send()
            .await
            .with_context(|| format!("Telegram API {}", method))?
            .json()
            .await?;
        if resp.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            let desc = resp
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            anyhow::bail!("Telegram {} error: {}", method, desc);
        }
        Ok(resp["result"].clone())
    }

    pub async fn get_me(&self) -> Result<User> {
        let v = self.call("getMe", &json!({})).await?;
        Ok(serde_json::from_value(v)?)
    }

    pub async fn get_updates(&self, offset: i64, timeout: u32) -> Result<Vec<Update>> {
        let v = self
            .call(
                "getUpdates",
                &json!({
                    "offset": offset,
                    "timeout": timeout,
                    "allowed_updates": ["message", "callback_query"]
                }),
            )
            .await?;
        Ok(serde_json::from_value(v).unwrap_or_default())
    }

    pub async fn send_action(&self, chat_id: i64, action: &str) {
        let _ = self
            .call(
                "sendChatAction",
                &json!({ "chat_id": chat_id, "action": action }),
            )
            .await;
    }

    pub async fn set_reaction(&self, chat_id: i64, message_id: i64, emoji: &str) {
        let _ = self
            .call(
                "setMessageReaction",
                &json!({
                    "chat_id": chat_id,
                    "message_id": message_id,
                    "reaction": [{ "type": "emoji", "emoji": emoji }]
                }),
            )
            .await;
    }

    pub async fn send_html(
        &self,
        chat_id: i64,
        reply_to: Option<i64>,
        text: &str,
        kb: Option<Keyboard>,
    ) -> Result<Message> {
        self.send_html_full(chat_id, reply_to, None, text, kb, None)
            .await
    }

    pub async fn send_html_full(
        &self,
        chat_id: i64,
        reply_to: Option<i64>,
        quote: Option<&str>,
        text: &str,
        kb: Option<Keyboard>,
        effect_id: Option<&str>,
    ) -> Result<Message> {
        let chunks = split_message(text, 3950);
        let mut last_msg: Option<Message> = None;

        for (i, chunk) in chunks.iter().enumerate() {
            let mut body = json!({
                "chat_id": chat_id,
                "text": chunk,
                "parse_mode": "HTML",
                "link_preview_options": { "is_disabled": true },
            });
            if i == 0 {
                if let Some(mid) = reply_to {
                    let mut rp = json!({
                        "message_id": mid,
                        "allow_sending_without_reply": true
                    });
                    if let Some(q) = quote {
                        let q_clean: String = q.chars().take(120).collect();
                        if !q_clean.trim().is_empty() {
                            rp["quote"] = json!(q_clean);
                        }
                    }
                    body["reply_parameters"] = rp;
                }
                if let Some(eff) = effect_id {
                    body["message_effect_id"] = json!(eff);
                }
            }
            if i + 1 == chunks.len() {
                if let Some(ref k) = kb {
                    body["reply_markup"] = json!({ "inline_keyboard": k });
                }
            }

            let res = match self.call("sendMessage", &body).await {
                Ok(v) => v,
                Err(_) => {
                    // Fallback without parse_mode or effect if HTML tags were malformed or group chat rejects effect
                    body.as_object_mut().map(|m| {
                        m.remove("parse_mode");
                        m.remove("message_effect_id");
                        m.remove("reply_parameters");
                    });
                    body["text"] = json!(strip_html_tags(chunk));
                    self.call("sendMessage", &body).await?
                }
            };
            last_msg = Some(serde_json::from_value(res)?);
        }
        last_msg.context("empty message chunks")
    }

    pub async fn edit_html(
        &self,
        chat_id: i64,
        message_id: i64,
        text: &str,
        kb: Option<Keyboard>,
    ) -> Result<()> {
        let mut body = json!({
            "chat_id": chat_id,
            "message_id": message_id,
            "text": truncate_str(text, 4000),
            "parse_mode": "HTML",
            "link_preview_options": { "is_disabled": true },
        });
        if let Some(k) = kb {
            body["reply_markup"] = json!({ "inline_keyboard": k });
        }
        if self.call("editMessageText", &body).await.is_err() {
            body.as_object_mut().map(|m| m.remove("parse_mode"));
            body["text"] = json!(strip_html_tags(&truncate_str(text, 4000)));
            let _ = self.call("editMessageText", &body).await;
        }
        Ok(())
    }

    pub async fn delete(&self, chat_id: i64, message_id: i64) {
        let _ = self
            .call(
                "deleteMessage",
                &json!({ "chat_id": chat_id, "message_id": message_id }),
            )
            .await;
    }

    pub async fn answer_cb(&self, id: &str, text: Option<&str>, alert: bool) {
        let mut body = json!({ "callback_query_id": id, "show_alert": alert });
        if let Some(t) = text {
            body["text"] = json!(t);
        }
        let _ = self.call("answerCallbackQuery", &body).await;
    }

    /// Streams a local file (`sendDocument`, `sendVideo`, `sendAudio`, `sendPhoto`, `sendVoice`)
    /// directly to Telegram via multipart/form-data without buffering the whole file in memory.
    pub async fn send_file(
        &self,
        chat_id: i64,
        reply_to: Option<i64>,
        path: &Path,
        caption: &str,
        method: &str,
        field: &str,
        kb: Option<Keyboard>,
    ) -> Result<Message> {
        let file = File::open(path)
            .await
            .with_context(|| format!("open {}", path.display()))?;
        let len = file.metadata().await?.len();
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("file.bin")
            .to_string();

        let stream = ReaderStream::new(file);
        let part = Part::stream_with_length(reqwest::Body::wrap_stream(stream), len)
            .file_name(filename);

        let mut form = Form::new()
            .text("chat_id", chat_id.to_string())
            .text("caption", truncate_str(caption, 1000))
            .text("parse_mode", "HTML")
            .part(field.to_string(), part);

        if method == "sendVideo" {
            form = form.text("supports_streaming", "true");
        }
        if let Some(mid) = reply_to {
            form = form.text("reply_to_message_id", mid.to_string());
        }
        if let Some(k) = kb {
            form = form.text(
                "reply_markup",
                serde_json::to_string(&json!({ "inline_keyboard": k }))?,
            );
        }

        let url = format!("{}/{}", self.base, method);
        let resp: Value = self.http.post(&url).multipart(form).send().await?.json().await?;
        if resp.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            let desc = resp
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("upload error");
            anyhow::bail!("Telegram {} failed: {}", method, desc);
        }
        Ok(serde_json::from_value(resp["result"].clone())?)
    }

    pub async fn send_photo_bytes(
        &self,
        chat_id: i64,
        reply_to: Option<i64>,
        bytes: Vec<u8>,
        filename: &str,
        caption: &str,
        kb: Option<Keyboard>,
    ) -> Result<Message> {
        let part = Part::bytes(bytes).file_name(filename.to_string());
        let mut form = Form::new()
            .text("chat_id", chat_id.to_string())
            .text("caption", truncate_str(caption, 1000))
            .text("parse_mode", "HTML")
            .part("photo", part);
        if let Some(mid) = reply_to {
            form = form.text("reply_to_message_id", mid.to_string());
        }
        if let Some(k) = kb {
            form = form.text(
                "reply_markup",
                serde_json::to_string(&json!({ "inline_keyboard": k }))?,
            );
        }
        let url = format!("{}/sendPhoto", self.base);
        let resp: Value = self.http.post(&url).multipart(form).send().await?.json().await?;
        if resp.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            anyhow::bail!("sendPhoto failed: {}", resp);
        }
        Ok(serde_json::from_value(resp["result"].clone())?)
    }

    pub async fn send_audio_bytes(
        &self,
        chat_id: i64,
        reply_to: Option<i64>,
        bytes: Vec<u8>,
        filename: &str,
        title: &str,
        caption: &str,
    ) -> Result<Message> {
        let part = Part::bytes(bytes).file_name(filename.to_string());
        let mut form = Form::new()
            .text("chat_id", chat_id.to_string())
            .text("title", title.to_string())
            .text("performer", "Amir Tools AI")
            .text("caption", truncate_str(caption, 1000))
            .text("parse_mode", "HTML")
            .part("audio", part);
        if let Some(mid) = reply_to {
            form = form.text("reply_to_message_id", mid.to_string());
        }
        let url = format!("{}/sendAudio", self.base);
        let resp: Value = self.http.post(&url).multipart(form).send().await?.json().await?;
        if resp.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            anyhow::bail!("sendAudio failed: {}", resp);
        }
        Ok(serde_json::from_value(resp["result"].clone())?)
    }

    pub async fn download_tg_file(&self, file_id: &str, dest: &Path) -> Result<()> {
        let info = self.call("getFile", &json!({ "file_id": file_id })).await?;
        let file_path = info["file_path"]
            .as_str()
            .context("missing file_path in getFile")?;
        let url = format!("{}/{}", self.file_base, file_path);
        let bytes = self.http.get(&url).send().await?.bytes().await?;
        tokio::fs::write(dest, &bytes).await?;
        Ok(())
    }

    // ── Group Moderation Methods ──
    pub async fn is_admin(&self, chat_id: i64, user_id: i64) -> bool {
        if let Ok(v) = self
            .call(
                "getChatMember",
                &json!({ "chat_id": chat_id, "user_id": user_id }),
            )
            .await
        {
            matches!(
                v.get("status").and_then(|s| s.as_str()),
                Some("creator") | Some("administrator")
            )
        } else {
            false
        }
    }

    pub async fn ban_member(&self, chat_id: i64, user_id: i64) -> Result<()> {
        self.call(
            "banChatMember",
            &json!({ "chat_id": chat_id, "user_id": user_id }),
        )
        .await?;
        Ok(())
    }

    pub async fn unban_member(&self, chat_id: i64, user_id: i64) -> Result<()> {
        self.call(
            "unbanChatMember",
            &json!({ "chat_id": chat_id, "user_id": user_id, "only_if_banned": true }),
        )
        .await?;
        Ok(())
    }

    pub async fn mute_member(&self, chat_id: i64, user_id: i64, can_send: bool) -> Result<()> {
        let perms = json!({
            "can_send_messages": can_send,
            "can_send_audios": can_send,
            "can_send_documents": can_send,
            "can_send_photos": can_send,
            "can_send_videos": can_send,
            "can_send_video_notes": can_send,
            "can_send_voice_notes": can_send,
            "can_send_polls": can_send,
            "can_send_other_messages": can_send,
            "can_add_web_page_previews": can_send,
        });
        self.call(
            "restrictChatMember",
            &json!({
                "chat_id": chat_id,
                "user_id": user_id,
                "permissions": perms
            }),
        )
        .await?;
        Ok(())
    }

    pub async fn pin_message(&self, chat_id: i64, message_id: i64) -> Result<()> {
        self.call(
            "pinChatMessage",
            &json!({ "chat_id": chat_id, "message_id": message_id }),
        )
        .await?;
        Ok(())
    }
}

pub fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

pub fn strip_html_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
}

pub fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max_chars.saturating_sub(3)).collect();
        t.push_str("...");
        t
    }
}

pub fn split_message(s: &str, max_chars: usize) -> Vec<String> {
    if s.is_empty() {
        return vec!["(پاسخ خالی)".to_string()];
    }
    if s.chars().count() <= max_chars {
        return vec![s.to_string()];
    }
    let mut out = Vec::new();
    let mut cur = String::new();
    for line in s.split('\n') {
        if cur.chars().count() + line.chars().count() + 1 > max_chars && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        if line.chars().count() > max_chars {
            let chars: Vec<char> = line.chars().collect();
            for ch in chars.chunks(max_chars) {
                out.push(ch.iter().collect());
            }
        } else {
            if !cur.is_empty() {
                cur.push('\n');
            }
            cur.push_str(line);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

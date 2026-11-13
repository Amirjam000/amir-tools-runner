//! Persistent, thread-safe JSON storage for users, bans, ghostbans, scores,
//! user states, per-user AI chat history, group settings, URL callback tokens,
//! anonymous chat tokens, and admin rules.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserRec {
    pub id: i64,
    pub first_name: String,
    pub username: Option<String>,
    pub joined_at: i64,
    pub messages: u64,
    pub downloads: u64,
    #[serde(default)]
    pub score: i64,
    #[serde(default)]
    pub warnings: u32,
    #[serde(default)]
    pub banned: bool,
    #[serde(default)]
    pub ghostbanned: bool,
    pub state: Option<String>,
    #[serde(default)]
    pub ai_model: Option<String>,
    #[serde(default)]
    pub image_model: Option<String>,
    #[serde(default)]
    pub tts_voice: Option<String>,
    #[serde(default)]
    pub anon_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupSettings {
    pub id: i64,
    pub title: String,
    pub lock_links: bool,
    pub lock_forwards: bool,
    pub lock_media: bool,
    pub welcome_enabled: bool,
    pub char_limit: usize,
    #[serde(default)]
    pub filtered_words: Vec<String>,
}

impl GroupSettings {
    pub fn new(id: i64, title: String, default_char_limit: usize) -> Self {
        Self {
            id,
            title,
            lock_links: false,
            lock_forwards: false,
            lock_media: false,
            welcome_enabled: true,
            char_limit: default_char_limit,
            filtered_words: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatTurn {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DbData {
    users: HashMap<i64, UserRec>,
    groups: HashMap<i64, GroupSettings>,
    histories: HashMap<i64, Vec<ChatTurn>>,
    url_cache: HashMap<String, String>,
    anon_map: HashMap<String, i64>,
    #[serde(default)]
    challenges: HashMap<i64, HashSet<i64>>,
    #[serde(default)]
    global_rules: Vec<String>,
    #[serde(default)]
    force_join_channel: Option<String>,
}

#[derive(Clone)]
pub struct Store {
    path: PathBuf,
    inner: Arc<RwLock<DbData>>,
    default_char_limit: usize,
}

impl Store {
    pub fn open(dir: &Path, default_char_limit: usize) -> Result<Self> {
        let path = dir.join("store.json");
        let data: DbData = if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            DbData::default()
        };
        Ok(Self {
            path,
            inner: Arc::new(RwLock::new(data)),
            default_char_limit,
        })
    }

    pub async fn save(&self) {
        let snapshot = {
            let g = self.inner.read().await;
            serde_json::to_string_pretty(&*g).ok()
        };
        if let Some(json) = snapshot {
            let tmp = self.path.with_extension("json.tmp");
            if tokio::fs::write(&tmp, json).await.is_ok() {
                let _ = tokio::fs::rename(&tmp, &self.path).await;
            }
        }
    }

    /// Touches user record and returns `(is_banned, is_ghostbanned, current_state)`.
    pub async fn touch_user(
        &self,
        id: i64,
        first_name: &str,
        username: Option<&str>,
    ) -> (bool, bool, Option<String>) {
        let mut g = self.inner.write().await;
        let now = chrono::Utc::now().timestamp();
        let entry = g.users.entry(id).or_insert_with(|| UserRec {
            id,
            first_name: first_name.to_string(),
            username: username.map(|s| s.to_string()),
            joined_at: now,
            messages: 0,
            downloads: 0,
            score: 10,
            warnings: 0,
            banned: false,
            ghostbanned: false,
            state: None,
            ai_model: None,
            image_model: None,
            tts_voice: None,
            anon_token: None,
        });
        entry.first_name = first_name.to_string();
        if let Some(u) = username {
            entry.username = Some(u.to_string());
        }
        entry.messages += 1;
        entry.score += 1;
        (entry.banned, entry.ghostbanned, entry.state.clone())
    }

    pub async fn get_user(&self, id: i64) -> Option<UserRec> {
        self.inner.read().await.users.get(&id).cloned()
    }

    pub async fn set_state(&self, user_id: i64, state: Option<String>) {
        if let Some(u) = self.inner.write().await.users.get_mut(&user_id) {
            u.state = state;
        }
    }

    pub async fn set_user_ai_model(&self, user_id: i64, model: &str) {
        if let Some(u) = self.inner.write().await.users.get_mut(&user_id) {
            u.ai_model = Some(model.to_string());
        }
        self.save().await;
    }

    pub async fn set_user_image_model(&self, user_id: i64, model: &str) {
        if let Some(u) = self.inner.write().await.users.get_mut(&user_id) {
            u.image_model = Some(model.to_string());
        }
        self.save().await;
    }

    pub async fn set_user_tts_voice(&self, user_id: i64, voice: &str) {
        if let Some(u) = self.inner.write().await.users.get_mut(&user_id) {
            u.tts_voice = Some(voice.to_string());
        }
        self.save().await;
    }

    pub async fn inc_download(&self, user_id: i64) {
        if let Some(u) = self.inner.write().await.users.get_mut(&user_id) {
            u.downloads += 1;
            u.score += 5;
        }
        self.save().await;
    }

    pub async fn add_score(&self, user_id: i64, delta: i64) -> i64 {
        let mut g = self.inner.write().await;
        if let Some(u) = g.users.get_mut(&user_id) {
            u.score += delta;
            return u.score;
        }
        0
    }

    pub async fn top_users(&self, limit: usize) -> Vec<UserRec> {
        let g = self.inner.read().await;
        let mut v: Vec<UserRec> = g.users.values().cloned().collect();
        v.sort_by(|a, b| b.score.cmp(&a.score));
        v.truncate(limit);
        v
    }

    pub async fn set_ban(&self, user_id: i64, banned: bool) {
        let mut g = self.inner.write().await;
        let entry = g.users.entry(user_id).or_insert_with(|| UserRec {
            id: user_id,
            first_name: format!("User {}", user_id),
            joined_at: chrono::Utc::now().timestamp(),
            ..Default::default()
        });
        entry.banned = banned;
        drop(g);
        self.save().await;
    }

    pub async fn set_ghostban(&self, user_id: i64, ghostbanned: bool) {
        let mut g = self.inner.write().await;
        let entry = g.users.entry(user_id).or_insert_with(|| UserRec {
            id: user_id,
            first_name: format!("User {}", user_id),
            joined_at: chrono::Utc::now().timestamp(),
            ..Default::default()
        });
        entry.ghostbanned = ghostbanned;
        drop(g);
        self.save().await;
    }

    pub async fn get_or_create_anon_token(&self, user_id: i64) -> String {
        let mut g = self.inner.write().await;
        if let Some(u) = g.users.get(&user_id) {
            if let Some(ref tok) = u.anon_token {
                return tok.clone();
            }
        }
        let tok = format!("anon_{:x}", (user_id as u64) ^ 0xA55A_5AA5);
        g.anon_map.insert(tok.clone(), user_id);
        if let Some(u) = g.users.get_mut(&user_id) {
            u.anon_token = Some(tok.clone());
        }
        drop(g);
        self.save().await;
        tok
    }

    pub async fn resolve_anon_token(&self, tok: &str) -> Option<i64> {
        let g = self.inner.read().await;
        if let Some(&uid) = g.anon_map.get(tok) {
            return Some(uid);
        }
        // Fallback decode
        if let Some(hex_part) = tok.strip_prefix("anon_") {
            if let Ok(v) = u64::from_str_radix(hex_part, 16) {
                return Some((v ^ 0xA55A_5AA5) as i64);
            }
        }
        None
    }

    pub async fn group_settings(&self, id: i64, title: &str) -> GroupSettings {
        let mut g = self.inner.write().await;
        let e = g
            .groups
            .entry(id)
            .or_insert_with(|| GroupSettings::new(id, title.to_string(), self.default_char_limit));
        if !title.is_empty() {
            e.title = title.to_string();
        }
        e.clone()
    }

    pub async fn update_group<F: FnOnce(&mut GroupSettings)>(
        &self,
        id: i64,
        f: F,
    ) -> GroupSettings {
        let mut g = self.inner.write().await;
        let e = g
            .groups
            .entry(id)
            .or_insert_with(|| GroupSettings::new(id, String::new(), self.default_char_limit));
        f(e);
        let cloned = e.clone();
        drop(g);
        self.save().await;
        cloned
    }

    pub async fn push_history(&self, user_id: i64, role: &str, content: &str) {
        let mut g = self.inner.write().await;
        let h = g.histories.entry(user_id).or_default();
        h.push(ChatTurn {
            role: role.to_string(),
            content: content.to_string(),
        });
        if h.len() > 16 {
            let drain = h.len() - 16;
            h.drain(0..drain);
        }
    }

    pub async fn get_history(&self, user_id: i64) -> Vec<ChatTurn> {
        self.inner
            .read()
            .await
            .histories
            .get(&user_id)
            .cloned()
            .unwrap_or_default()
    }

    pub async fn clear_history(&self, user_id: i64) {
        self.inner.write().await.histories.remove(&user_id);
        self.save().await;
    }

    /// Stores a URL or query under a short 10-char hex token for Telegram's 64-byte `callback_data`.
    pub async fn stash_url(&self, url: &str) -> String {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in url.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        let key = format!("{:010x}", h & 0xff_ffff_ffff);
        let mut g = self.inner.write().await;
        if g.url_cache.len() > 4000 {
            g.url_cache.clear();
        }
        g.url_cache.insert(key.clone(), url.to_string());
        key
    }

    pub async fn recall_url(&self, key: &str) -> Option<String> {
        self.inner.read().await.url_cache.get(key).cloned()
    }

    pub async fn join_challenge(&self, chat_id: i64, user_id: i64) -> (bool, usize) {
        let mut g = self.inner.write().await;
        let set = g.challenges.entry(chat_id).or_default();
        let inserted = set.insert(user_id);
        let count = set.len();
        drop(g);
        self.save().await;
        (inserted, count)
    }

    pub async fn pick_challenge_winner(&self, chat_id: i64) -> Option<i64> {
        use rand::seq::IteratorRandom;
        let g = self.inner.read().await;
        let set = g.challenges.get(&chat_id)?;
        let mut rng = rand::thread_rng();
        set.iter().copied().choose(&mut rng)
    }

    pub async fn clear_challenge(&self, chat_id: i64) {
        self.inner.write().await.challenges.remove(&chat_id);
        self.save().await;
    }

    pub async fn add_rule(&self, rule: &str) {
        self.inner.write().await.global_rules.push(rule.to_string());
        self.save().await;
    }

    pub async fn get_rules(&self) -> Vec<String> {
        self.inner.read().await.global_rules.clone()
    }

    pub async fn clear_rules(&self) {
        self.inner.write().await.global_rules.clear();
        self.save().await;
    }

    pub async fn stats(&self) -> (usize, usize, u64, u64) {
        let g = self.inner.read().await;
        let users = g.users.len();
        let groups = g.groups.len();
        let msgs: u64 = g.users.values().map(|u| u.messages).sum();
        let dls: u64 = g.users.values().map(|u| u.downloads).sum();
        (users, groups, msgs, dls)
    }

    pub async fn all_user_ids(&self) -> Vec<i64> {
        self.inner.read().await.users.keys().copied().collect()
    }
}

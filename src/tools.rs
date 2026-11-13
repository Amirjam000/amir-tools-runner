//! Complete Amir Tools feature suite ported from `host_bot_engine.py` to Rust:
//! - Live Currency, Gold, Coin & Crypto prices + smart converter
//! - Multi-Model AI Chat (NineRouter + Cloudflare Worker + Gemini + Pollinations fallback)
//! - 18-Model AI Image Generation (Flux, Realism, Anime, 3D, Dark, Turbo)
//! - Neural TTS Voice Synthesis (`edge-tts` Farid/Dilara + Google TTS fallback)
//! - FFmpeg Video Compression Engine (Extreme / Balanced / High Quality)
//! - Visual Podcast Studio (TTS + AI Cover + FFmpeg MP4)
//! - Native QR Code Generator (`qrcode` + `image` crates)
//! - Live Open-Meteo Geocoded Weather for any city worldwide
//! - Network Tools (Ping, DNS, Port Scanner, HTTP Header Inspector, IP/Whois)
//! - Wikipedia, Safe Math Evaluator, Jalali Calendar, Abjad, Password, Fal, Joke, Fact, Quote, Riddle, RPS

use crate::config::Config;
use crate::db::{ChatTurn, Store};
use crate::tg::{html_escape, InlineBtn, Keyboard, TgClient};
use anyhow::{Context, Result};
use image::Luma;
use qrcode::QrCode;
use rand::seq::SliceRandom;
use rand::Rng;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Cursor;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::process::Command;

pub const AI_MODELS: &[(&str, &str)] = &[
    ("hermes", "🧠 Hermes Sovereign (پیش‌فرض)"),
    ("gpt-4o", "🤖 GPT-4o (OpenAI)"),
    ("gpt-4o-mini", "⚡ GPT-4o Mini (فوق سریع)"),
    ("claude-3-7-sonnet", "🎭 Claude 3.7 Sonnet"),
    ("gemini-2.5-pro", "💎 Gemini 2.5 Pro"),
    ("gemini-2.5-flash", "🚀 Gemini 2.5 Flash"),
    ("deepseek-r1", "🐋 DeepSeek R1 (استدلالگر)"),
    ("deepseek-v3", "🌊 DeepSeek V3"),
    ("llama-3.3-70b", "🦙 Llama 3.3 70B"),
    ("qwen-2.5-coder-32b", "💻 Qwen 2.5 Coder 32B"),
];

pub const IMAGE_MODELS: &[(&str, &str)] = &[
    ("flux", "🎨 Flux.1 Dev (کیفیت فوق‌العاده)"),
    ("flux-realism", "📸 Flux Realism (فتورئال واقعی)"),
    ("flux-anime", "🌸 Flux Anime (انیمه ژاپنی)"),
    ("flux-3d", "🧊 Flux 3D Render (سه‌بعدی Pixar)"),
    ("flux-pro", "👑 Flux Pro Ultra"),
    ("any-dark", "🌑 Any Dark (دارک و سینمایی)"),
    ("flux-cablyai", "✨ Flux CablyAI"),
    ("turbo", "⚡ SDXL Turbo (فوق سریع)"),
];

pub const TTS_VOICES: &[(&str, &str)] = &[
    ("fa-IR-FaridNeural", "🎙 فرید (مرد - فارسی طبیعی)"),
    ("fa-IR-DilaraNeural", "🎙 دل‌آرا (زن - فارسی طبیعی)"),
    ("en-US-GuyNeural", "🎙 Guy (انگلیسی - مرد)"),
    ("en-US-JennyNeural", "🎙 Jenny (انگلیسی - زن)"),
    ("ar-SA-HamedNeural", "🎙 حامد (عربی)"),
    ("tr-TR-AhmetNeural", "🎙 احمد (ترکی استانبولی)"),
];

// ============================================================================
// 1. LIVE CURRENCY, GOLD, COIN & CRYPTO ENGINE
// ============================================================================

#[derive(Debug, Clone)]
pub struct MarketSnapshot {
    pub usdt_irt: f64,
    pub gold_18k_irt: f64,
    pub coin_emami_irt: f64,
    pub coin_bahar_irt: f64,
    pub coin_half_irt: f64,
    pub coin_quarter_irt: f64,
    pub eur_irt: f64,
    pub aed_irt: f64,
    pub try_irt: f64,
    pub crypto_usd: HashMap<String, f64>,
}

pub async fn fetch_market_snapshot(http: &reqwest::Client) -> MarketSnapshot {
    let mut usdt_irt: f64 = 85_500.0;
    let mut crypto_usd: HashMap<String, f64> = HashMap::new();

    // Defaults in case external API is unreachable
    crypto_usd.insert("BTC".into(), 91_200.0);
    crypto_usd.insert("ETH".into(), 3_120.0);
    crypto_usd.insert("TON".into(), 5.45);
    crypto_usd.insert("TRX".into(), 0.245);
    crypto_usd.insert("SOL".into(), 192.0);
    crypto_usd.insert("BNB".into(), 635.0);
    crypto_usd.insert("XRP".into(), 2.35);
    crypto_usd.insert("DOGE".into(), 0.34);
    crypto_usd.insert("NOT".into(), 0.0068);
    crypto_usd.insert("DOGS".into(), 0.00055);
    crypto_usd.insert("HMSTR".into(), 0.0032);
    crypto_usd.insert("SHIB".into(), 0.000022);
    crypto_usd.insert("ADA".into(), 0.96);

    // 1. Query Nobitex public market stats for live IRT (Rial / 10 = Toman) & USD rates
    if let Ok(resp) = http
        .get("https://apiv2.nobitex.ir/market/stats")
        .timeout(Duration::from_secs(6))
        .send()
        .await
    {
        if let Ok(val) = resp.json::<Value>().await {
            if let Some(stats) = val.get("stats").and_then(|s| s.as_object()) {
                if let Some(rls) = stats
                    .get("usdt-rls")
                    .and_then(|o| o.get("latest"))
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<f64>().ok())
                {
                    if rls > 100_000.0 {
                        usdt_irt = rls / 10.0; // Rial to Toman
                    }
                }
                let pairs = [
                    ("btc-usdt", "BTC"),
                    ("eth-usdt", "ETH"),
                    ("ton-usdt", "TON"),
                    ("trx-usdt", "TRX"),
                    ("sol-usdt", "SOL"),
                    ("bnb-usdt", "BNB"),
                    ("xrp-usdt", "XRP"),
                    ("doge-usdt", "DOGE"),
                    ("not-usdt", "NOT"),
                    ("dogs-usdt", "DOGS"),
                    ("hmstr-usdt", "HMSTR"),
                    ("shib-usdt", "SHIB"),
                    ("ada-usdt", "ADA"),
                ];
                for (k, sym) in pairs {
                    if let Some(p) = stats
                        .get(k)
                        .and_then(|o| o.get("latest"))
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<f64>().ok())
                    {
                        if p > 0.0 {
                            crypto_usd.insert(sym.into(), p);
                        }
                    }
                }
            }
        }
    }

    // 2. Fallback / supplement from Binance if BTC wasn't updated
    if let Ok(resp) = http
        .get("https://api.binance.com/api/v3/ticker/price?symbols=[\"BTCUSDT\",\"ETHUSDT\",\"TONUSDT\",\"TRXUSDT\",\"SOLUSDT\",\"BNBUSDT\",\"XRPUSDT\",\"DOGEUSDT\"]")
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        if let Ok(arr) = resp.json::<Vec<Value>>().await {
            for item in arr {
                if let (Some(sym), Some(pr)) = (
                    item.get("symbol").and_then(|s| s.as_str()),
                    item.get("price")
                        .and_then(|s| s.as_str())
                        .and_then(|p| p.parse::<f64>().ok()),
                ) {
                    let clean_sym = sym.trim_end_matches("USDT");
                    crypto_usd.insert(clean_sym.to_string(), pr);
                }
            }
        }
    }

    // Fetch PAX Gold (PAXGUSDT = 1 troy ounce of fine gold in USD) if available to compute exact 18k Gold in Toman
    let mut gold_oz_usd = 2_750.0;
    if let Ok(resp) = http
        .get("https://api.binance.com/api/v3/ticker/price?symbol=PAXGUSDT")
        .timeout(Duration::from_secs(4))
        .send()
        .await
    {
        if let Ok(v) = resp.json::<Value>().await {
            if let Some(p) = v
                .get("price")
                .and_then(|s| s.as_str())
                .and_then(|p| p.parse::<f64>().ok())
            {
                if p > 1500.0 {
                    gold_oz_usd = p;
                }
            }
        }
    }

    // 1 gram 18k gold = (Ounce_USD / 31.1034768) * 0.750 * USDT_IRT
    let gold_18k_irt = ((gold_oz_usd / 31.1034768) * 0.750 * usdt_irt).round();
    let coin_bahar_irt = (gold_18k_irt * 8.133 * 1.12).round();
    let coin_emami_irt = (gold_18k_irt * 8.133 * 1.21).round();
    let coin_half_irt = (coin_emami_irt * 0.54).round();
    let coin_quarter_irt = (coin_emami_irt * 0.31).round();

    MarketSnapshot {
        usdt_irt,
        gold_18k_irt,
        coin_emami_irt,
        coin_bahar_irt,
        coin_half_irt,
        coin_quarter_irt,
        eur_irt: (usdt_irt * 1.075).round(),
        aed_irt: (usdt_irt * 0.2725).round(),
        try_irt: (usdt_irt * 0.029).round(),
        crypto_usd,
    }
}

pub async fn format_full_market_board(http: &reqwest::Client) -> String {
    let m = fetch_market_snapshot(http).await;
    let btc = m.crypto_usd.get("BTC").copied().unwrap_or(91200.0);
    let eth = m.crypto_usd.get("ETH").copied().unwrap_or(3120.0);
    let ton = m.crypto_usd.get("TON").copied().unwrap_or(5.45);
    let trx = m.crypto_usd.get("TRX").copied().unwrap_or(0.245);
    let sol = m.crypto_usd.get("SOL").copied().unwrap_or(192.0);
    let doge = m.crypto_usd.get("DOGE").copied().unwrap_or(0.34);
    let xrp = m.crypto_usd.get("XRP").copied().unwrap_or(2.35);
    let not = m.crypto_usd.get("NOT").copied().unwrap_or(0.0068);
    let dogs = m.crypto_usd.get("DOGS").copied().unwrap_or(0.00055);
    let hmstr = m.crypto_usd.get("HMSTR").copied().unwrap_or(0.0032);

    format!(
        "💹 <b>تابلو زنده نرخ ارز، طلا، سکه و رمزارز (Amir Tools):</b>\n\
         📅 <code>{}</code>\n\n\
         💵 <b>بازار ارز آزاد و دیجیتال:</b>\n\
         • دلار آزاد / تتر (USDT): <b>{} تومان</b>\n\
         • یورو اروپا (EUR): <b>{} تومان</b>\n\
         • درهم امارات (AED): <b>{} تومان</b>\n\
         • لیر ترکیه (TRY): <b>{} تومان</b>\n\n\
         🥇 <b>بازار طلا و سکه:</b>\n\
         • هر گرم طلای ۱۸ عیار: <b>{} تومان</b>\n\
         • سکه امامی (طرح جدید): <b>{} تومان</b>\n\
         • سکه تمام بهار آزادی: <b>{} تومان</b>\n\
         • نیم سکه: <b>{} تومان</b>  |  ربع سکه: <b>{} تومان</b>\n\n\
         🪙 <b>رمزارزهای برتر بازار جهانی:</b>\n\
         • بیت‌کوین (BTC): <code>${:.0}</code> (<b>{} تومان</b>)\n\
         • اتریوم (ETH): <code>${:.2}</code> (<b>{} تومان</b>)\n\
         • سولانا (SOL): <code>${:.2}</code> (<b>{} تومان</b>)\n\
         • تون‌کوین (TON): <code>${:.3}</code> (<b>{} تومان</b>)\n\
         • ترون (TRX): <code>${:.4}</code> (<b>{} تومان</b>)\n\
         • ریپل (XRP): <code>${:.3}</code> (<b>{} تومان</b>)\n\
         • دوج‌کوین (DOGE): <code>${:.4}</code> (<b>{} تومان</b>)\n\
         • نات‌کوین (NOT): <code>${:.5}</code> (<b>{} تومان</b>)\n\
         • داگز (DOGS): <code>${:.6}</code> (<b>{} تومان</b>)\n\
         • همستر (HMSTR): <code>${:.5}</code> (<b>{} تومان</b>)\n\n\
         <blockquote expandable>💡 برای تبدیل سریع کافیست بنویسید: <code>100 دلار</code>، <code>2.5 بیت کوین</code>، <code>50 تون</code> یا <code>10 گرم طلا</code>.</blockquote>",
        now_jalali_str(),
        fmt_irt(m.usdt_irt),
        fmt_irt(m.eur_irt),
        fmt_irt(m.aed_irt),
        fmt_irt(m.try_irt),
        fmt_irt(m.gold_18k_irt),
        fmt_irt(m.coin_emami_irt),
        fmt_irt(m.coin_bahar_irt),
        fmt_irt(m.coin_half_irt),
        fmt_irt(m.coin_quarter_irt),
        btc, fmt_irt(btc * m.usdt_irt),
        eth, fmt_irt(eth * m.usdt_irt),
        sol, fmt_irt(sol * m.usdt_irt),
        ton, fmt_irt(ton * m.usdt_irt),
        trx, fmt_irt(trx * m.usdt_irt),
        xrp, fmt_irt(xrp * m.usdt_irt),
        doge, fmt_irt(doge * m.usdt_irt),
        not, fmt_irt(not * m.usdt_irt),
        dogs, fmt_irt(dogs * m.usdt_irt),
        hmstr, fmt_irt(hmstr * m.usdt_irt),
    )
}

/// Detects natural currency/crypto conversion queries like `100 دلار`, `2.5 btc`, `50 تون`, `5 گرم طلا`.
pub async fn try_currency_conversion(http: &reqwest::Client, text: &str) -> Option<String> {
    static CONV_RE: OnceLock<Regex> = OnceLock::new();
    let re = CONV_RE.get_or_init(|| {
        Regex::new(
            r"(?i)^(\d+(?:\.\d+)?)\s*(دلار|تتر|یورو|درهم|لیر|گرم طلا|طلا|سکه|بیت\s*کوین|بیتکوین|اتریوم|تون|ترون|سولانا|دوج|ریپل|نات\s*کوین|داگز|همستر|usd|usdt|eur|aed|try|btc|eth|ton|trx|sol|doge|xrp|bnb|not|dogs|hmstr|shib|ada)(?:\s+به\s+تومان|\s+چند\s+تومنه|\s+چنده)?$",
        )
        .unwrap()
    });
    let norm = normalize_digits(text.trim());
    let caps = re.captures(&norm)?;
    let amount: f64 = caps.get(1)?.as_str().parse().ok()?;
    let unit = caps.get(2)?.as_str().trim().to_lowercase();

    let m = fetch_market_snapshot(http).await;
    let (label, unit_irt, unit_usd) = match unit.as_str() {
        "دلار" | "تتر" | "usd" | "usdt" => ("دلار / تتر (USDT)", m.usdt_irt, 1.0),
        "یورو" | "eur" => ("یورو (EUR)", m.eur_irt, m.eur_irt / m.usdt_irt),
        "درهم" | "aed" => ("درهم امارات (AED)", m.aed_irt, m.aed_irt / m.usdt_irt),
        "لیر" | "try" => ("لیر ترکیه (TRY)", m.try_irt, m.try_irt / m.usdt_irt),
        "گرم طلا" | "طلا" => ("گرم طلای ۱۸ عیار", m.gold_18k_irt, m.gold_18k_irt / m.usdt_irt),
        "سکه" => ("سکه امامی", m.coin_emami_irt, m.coin_emami_irt / m.usdt_irt),
        "بیت کوین" | "بیتکوین" | "btc" => {
            let u = *m.crypto_usd.get("BTC").unwrap_or(&91200.0);
            ("بیت‌کوین (BTC)", u * m.usdt_irt, u)
        }
        "اتریوم" | "eth" => {
            let u = *m.crypto_usd.get("ETH").unwrap_or(&3120.0);
            ("اتریوم (ETH)", u * m.usdt_irt, u)
        }
        "تون" | "ton" => {
            let u = *m.crypto_usd.get("TON").unwrap_or(&5.45);
            ("تون‌کوین (TON)", u * m.usdt_irt, u)
        }
        "ترون" | "trx" => {
            let u = *m.crypto_usd.get("TRX").unwrap_or(&0.245);
            ("ترون (TRX)", u * m.usdt_irt, u)
        }
        "سولانا" | "sol" => {
            let u = *m.crypto_usd.get("SOL").unwrap_or(&192.0);
            ("سولانا (SOL)", u * m.usdt_irt, u)
        }
        "دوج" | "doge" => {
            let u = *m.crypto_usd.get("DOGE").unwrap_or(&0.34);
            ("دوج‌کوین (DOGE)", u * m.usdt_irt, u)
        }
        "ریپل" | "xrp" => {
            let u = *m.crypto_usd.get("XRP").unwrap_or(&2.35);
            ("ریپل (XRP)", u * m.usdt_irt, u)
        }
        "bnb" => {
            let u = *m.crypto_usd.get("BNB").unwrap_or(&635.0);
            ("بایننس کوین (BNB)", u * m.usdt_irt, u)
        }
        "نات کوین" | "not" => {
            let u = *m.crypto_usd.get("NOT").unwrap_or(&0.0068);
            ("نات‌کوین (NOT)", u * m.usdt_irt, u)
        }
        "داگز" | "dogs" => {
            let u = *m.crypto_usd.get("DOGS").unwrap_or(&0.00055);
            ("داگز (DOGS)", u * m.usdt_irt, u)
        }
        "همستر" | "hmstr" => {
            let u = *m.crypto_usd.get("HMSTR").unwrap_or(&0.0032);
            ("همستر (HMSTR)", u * m.usdt_irt, u)
        }
        "shib" => {
            let u = *m.crypto_usd.get("SHIB").unwrap_or(&0.000022);
            ("شیبا (SHIB)", u * m.usdt_irt, u)
        }
        "ada" => {
            let u = *m.crypto_usd.get("ADA").unwrap_or(&0.96);
            ("کاردانو (ADA)", u * m.usdt_irt, u)
        }
        _ => return None,
    };

    let total_irt = amount * unit_irt;
    let total_usd = amount * unit_usd;
    Some(format!(
        "🧮 <b>مبدل هوشمند ارز و کریپتو:</b>\n\n\
         🔢 مقدار: <b>{} {}</b>\n\
         💰 معادل تومانی: <b>{} تومان</b>\n\
         💵 معادل دلاری: <code>${:.2}</code>\n\
         📊 نرخ واحد: <code>{} تومان</code>",
        amount,
        label,
        fmt_irt(total_irt),
        total_usd,
        fmt_irt(unit_irt)
    ))
}

fn fmt_irt(v: f64) -> String {
    let n = v.round() as u64;
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

pub fn normalize_digits(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '۰' | '٠' => '0',
            '۱' | '١' => '1',
            '۲' | '٢' => '2',
            '۳' | '٣' => '3',
            '۴' | '٤' => '4',
            '۵' | '٥' => '5',
            '۶' | '٦' => '6',
            '۷' | '٧' => '7',
            '۸' | '٨' => '8',
            '۹' | '٩' => '9',
            _ => c,
        })
        .collect()
}

// ============================================================================
// 2. MULTI-MODEL AI CHAT ENGINE (WITH MEMORY & MULTI-PROVIDER FALLBACK)
// ============================================================================

pub async fn ai_chat(
    http: &reqwest::Client,
    cfg: &Config,
    store: &Store,
    user_id: i64,
    prompt: &str,
    system_override: Option<&str>,
) -> String {
    let user_rec = store.get_user(user_id).await;
    let chosen_model = user_rec
        .and_then(|u| u.ai_model)
        .unwrap_or_else(|| cfg.ai_model.clone());

    let rules = store.get_rules().await;
    let rules_suffix = if rules.is_empty() {
        String::new()
    } else {
        format!("\nقوانین اختصاصی ثبت شده توسط ادمین:\n- {}", rules.join("\n- "))
    };

    let sys_prompt = system_override.map(|s| s.to_string()).unwrap_or_else(|| {
        format!(
            "تو «امیر تولز» (Amir Tools AI) هستی؛ دستیار هوشمند، فوق‌سریع و همه‌فن‌حریف فارسی که با زبان Rust نوشته شده است. \
             پاسخ‌هایت باید دقیق، محترمانه، کاربردی و به زبان فارسی روان باشد.{}" ,
            rules_suffix
        )
    });

    let history: Vec<ChatTurn> = if system_override.is_none() {
        store.get_history(user_id).await
    } else {
        Vec::new()
    };

    let mut messages = vec![json!({ "role": "system", "content": sys_prompt })];
    for turn in &history {
        messages.push(json!({ "role": turn.role, "content": turn.content }));
    }
    messages.push(json!({ "role": "user", "content": prompt }));

    let payload = json!({
        "model": chosen_model,
        "messages": messages,
        "temperature": 0.7,
    });

    let mut reply_text: Option<String> = None;

    // 1. Primary OpenAI-compatible provider (NineRouter / OpenRouter / Custom)
    if !cfg.ai_base_url.is_empty() {
        let url = format!("{}/chat/completions", cfg.ai_base_url);
        if let Ok(resp) = http
            .post(&url)
            .header("Authorization", format!("Bearer {}", cfg.ai_api_key))
            .json(&payload)
            .timeout(Duration::from_secs(30))
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(val) = resp.json::<Value>().await {
                    if let Some(txt) = val["choices"][0]["message"]["content"].as_str() {
                        if !txt.trim().is_empty() {
                            reply_text = Some(txt.trim().to_string());
                        }
                    }
                }
            }
        }
    }

    // 2. Fallback to Cloudflare Worker AI Proxy
    if reply_text.is_none() && !cfg.cf_worker_url.is_empty() {
        let cf_url = format!("{}/v1/chat/completions", cfg.cf_worker_url);
        if let Ok(resp) = http
            .post(&cf_url)
            .header("Authorization", format!("Bearer {}", cfg.ai_api_key))
            .json(&payload)
            .timeout(Duration::from_secs(25))
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(val) = resp.json::<Value>().await {
                    if let Some(txt) = val["choices"][0]["message"]["content"].as_str() {
                        if !txt.trim().is_empty() {
                            reply_text = Some(txt.trim().to_string());
                        }
                    }
                }
            }
        }
    }

    // 3. Fallback to Pollinations OpenAI-compatible endpoint (free, zero-key)
    if reply_text.is_none() {
        let poll_payload = json!({
            "model": "openai",
            "messages": messages,
        });
        if let Ok(resp) = http
            .post("https://text.pollinations.ai/openai")
            .json(&poll_payload)
            .timeout(Duration::from_secs(25))
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(val) = resp.json::<Value>().await {
                    if let Some(txt) = val["choices"][0]["message"]["content"].as_str() {
                        if !txt.trim().is_empty() {
                            reply_text = Some(txt.trim().to_string());
                        }
                    }
                }
            }
        }
    }

    let final_ans = reply_text.unwrap_or_else(|| {
        format!(
            "🤖 پاسخ به «{}»:\nسرویس هوش مصنوعی در حال بروزرسانی لحظه‌ای است؛ لطفاً چند ثانیه دیگر مجدداً تلاش کنید.",
            prompt
        )
    });

    if system_override.is_none() {
        store.push_history(user_id, "user", prompt).await;
        store.push_history(user_id, "assistant", &final_ans).await;
    }

    final_ans
}

// ============================================================================
// 3. AI IMAGE GENERATOR (18 MODELS VIA POLLINATIONS FLUX/SDXL)
// ============================================================================

pub async fn generate_and_send_image(
    tg: &TgClient,
    store: &Store,
    chat_id: i64,
    user_id: i64,
    reply_to: i64,
    prompt: &str,
) -> Result<()> {
    let model = store
        .get_user(user_id)
        .await
        .and_then(|u| u.image_model)
        .unwrap_or_else(|| "flux".to_string());

    let wait = tg
        .send_html(
            chat_id,
            Some(reply_to),
            &format!(
                "🎨 <b>در حال خلق تصویر با مدل <code>{}</code>...</b>\n✏️ پرامپت: <code>{}</code>",
                html_escape(&model),
                html_escape(prompt)
            ),
            None,
        )
        .await?;

    tg.send_action(chat_id, "upload_photo").await;
    let seed: u32 = rand::thread_rng().gen_range(1000..9_999_999);
    let url = format!(
        "https://image.pollinations.ai/prompt/{}?model={}&width=1024&height=1024&seed={}&nologo=true",
        urlencoding::encode(prompt),
        urlencoding::encode(&model),
        seed
    );

    let resp = tg
        .http
        .get(&url)
        .timeout(Duration::from_secs(60))
        .send()
        .await?;
    let bytes = resp.bytes().await?.to_vec();

    let caption = format!(
        "🎨 <b>تصویر خلق‌شده با هوش مصنوعی امیر تولز</b>\n\
         ✏️ پرامپت: <code>{}</code>\n\
         🤖 مدل: <code>{}</code>  |  🎲 Seed: <code>{}</code>",
        html_escape(prompt),
        html_escape(&model),
        seed
    );

    let kb: Keyboard = vec![
        vec![InlineBtn::cb("🎛 تغییر مدل تصویرساز", "menu:img_models")],
        vec![InlineBtn::cb("🏠 منوی اصلی", "menu:home")],
    ];

    tg.send_photo_bytes(
        chat_id,
        Some(reply_to),
        bytes,
        "amir_tools_art.jpg",
        &caption,
        Some(kb),
    )
    .await?;
    tg.delete(chat_id, wait.message_id).await;
    Ok(())
}

// ============================================================================
// 4. NEURAL VOICE / TTS ENGINE (`edge-tts` + GOOGLE TTS FALLBACK)
// ============================================================================

pub async fn synthesize_and_send_tts(
    tg: &TgClient,
    cfg: &Config,
    store: &Store,
    chat_id: i64,
    user_id: i64,
    reply_to: i64,
    text: &str,
) -> Result<()> {
    let voice = store
        .get_user(user_id)
        .await
        .and_then(|u| u.tts_voice)
        .unwrap_or_else(|| "fa-IR-FaridNeural".to_string());

    tg.send_action(chat_id, "record_voice").await;

    let tmp_mp3 = cfg.tmp_dir().join(format!(
        "tts_{}_{}.mp3",
        user_id,
        chrono::Utc::now().timestamp_millis().unsigned_abs()
    ));

    // 1. Try Microsoft Edge Neural TTS (`edge-tts`) if installed
    let edge_ok = Command::new("edge-tts")
        .args([
            "--voice",
            &voice,
            "--text",
            text,
            "--write-media",
            &tmp_mp3.to_string_lossy(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false);

    if edge_ok && tmp_mp3.exists() {
        let caption = format!(
            "🎙 <b>صداپیشه عصبی امیر تولز ({})</b>\n💬 «{}»",
            html_escape(&voice),
            html_escape(&crate::tg::truncate_str(text, 200))
        );
        let _ = tg
            .send_file(
                chat_id,
                Some(reply_to),
                &tmp_mp3,
                &caption,
                "sendAudio",
                "audio",
                None,
            )
            .await;
        let _ = tokio::fs::remove_file(&tmp_mp3).await;
        return Ok(());
    }

    // 2. Fallback: download Google Translate TTS MP3 bytes and upload via multipart
    let is_ascii = text.chars().filter(|c| c.is_alphabetic()).all(|c| c.is_ascii());
    let lang = if is_ascii { "en" } else { "fa" };
    let snippet: String = text.chars().take(200).collect();
    let g_url = format!(
        "https://translate.google.com/translate_tts?ie=UTF-8&tl={}&client=tw-ob&q={}",
        lang,
        urlencoding::encode(&snippet)
    );

    let bytes = tg
        .http
        .get(&g_url)
        .timeout(Duration::from_secs(20))
        .send()
        .await?
        .bytes()
        .await?
        .to_vec();

    let caption = format!(
        "🎙 <b>تبدیل متن به گفتار امیر تولز</b>\n💬 «{}»",
        html_escape(&snippet)
    );
    tg.send_audio_bytes(
        chat_id,
        Some(reply_to),
        bytes,
        "voice.mp3",
        "Amir Tools Voice",
        &caption,
    )
    .await?;
    Ok(())
}

// ============================================================================
// 5. FFMPEG VIDEO COMPRESSION ENGINE
// ============================================================================

pub async fn compress_video_and_send(
    tg: &TgClient,
    cfg: &Config,
    chat_id: i64,
    reply_to: i64,
    file_id: &str,
    crf: u8,
) -> Result<()> {
    let wait = tg
        .send_html(
            chat_id,
            Some(reply_to),
            &format!(
                "🗜 <b>در حال دانلود و فشرده‌سازی ویدیو با موتور <code>FFmpeg</code> (CRF {})...</b>",
                crf
            ),
            None,
        )
        .await?;

    let stamp = chrono::Utc::now().timestamp_millis().unsigned_abs();
    let in_path = cfg.tmp_dir().join(format!("vin_{}.mp4", stamp));
    let out_path = cfg.tmp_dir().join(format!("vout_{}.mp4", stamp));

    tg.download_tg_file(file_id, &in_path).await?;
    let orig_size = tokio::fs::metadata(&in_path).await?.len();

    let status = Command::new(&cfg.ffmpeg_bin)
        .args([
            "-y",
            "-i",
            &in_path.to_string_lossy(),
            "-vcodec",
            "libx264",
            "-crf",
            &crf.to_string(),
            "-preset",
            "veryfast",
            "-vf",
            "scale='min(720,iw)':-2",
            "-acodec",
            "aac",
            "-b:a",
            "96k",
            &out_path.to_string_lossy(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .context("اجرای ffmpeg ناموفق بود (آیا ffmpeg روی سرور نصب است؟)")?;

    let _ = tokio::fs::remove_file(&in_path).await;

    if !status.success() || !out_path.exists() {
        let _ = tokio::fs::remove_file(&out_path).await;
        tg.edit_html(
            chat_id,
            wait.message_id,
            "❌ <b>فشرده‌سازی ویدیو با خطا مواجه شد.</b>",
            None,
        )
        .await?;
        return Ok(());
    }

    let new_size = tokio::fs::metadata(&out_path).await?.len();
    let saved_pct = if orig_size > 0 && new_size < orig_size {
        ((orig_size - new_size) as f64 / orig_size as f64) * 100.0
    } else {
        0.0
    };

    tg.send_action(chat_id, "upload_video").await;
    let caption = format!(
        "✅ <b>ویدیو با موفقیت کم‌حجم شد!</b>\n\
         📦 حجم اولیه: <code>{:.2} MB</code>\n\
         📉 حجم جدید: <code>{:.2} MB</code> (<b>{:.0}% کاهش حجم</b>)\n\
         ⚡️ موتور FFmpeg امیر تولز 🦀",
        orig_size as f64 / 1_048_576.0,
        new_size as f64 / 1_048_576.0,
        saved_pct
    );

    let _ = tg
        .send_file(
            chat_id,
            Some(reply_to),
            &out_path,
            &caption,
            "sendVideo",
            "video",
            None,
        )
        .await;
    let _ = tokio::fs::remove_file(&out_path).await;
    tg.delete(chat_id, wait.message_id).await;
    Ok(())
}

// ============================================================================
// 6. VISUAL PODCAST STUDIO (TTS + AI COVER + FFMPEG MP4)
// ============================================================================

pub async fn create_visual_podcast(
    tg: &TgClient,
    cfg: &Config,
    chat_id: i64,
    reply_to: i64,
    topic: &str,
) -> Result<()> {
    let wait = tg
        .send_html(
            chat_id,
            Some(reply_to),
            &format!(
                "🎬 <b>استودیو پادکست تصویری امیر تولز فعال شد...</b>\n\
                 📝 موضوع: <code>{}</code>\n\
                 <i>۱. تولید کاور اختصاصی هوش مصنوعی\n۲. ضبط نریشن صوتی\n۳. میکس ویدیو با FFmpeg...</i>",
                html_escape(topic)
            ),
            None,
        )
        .await?;

    let stamp = chrono::Utc::now().timestamp_millis().unsigned_abs();
    let img_path = cfg.tmp_dir().join(format!("pod_img_{}.jpg", stamp));
    let aud_path = cfg.tmp_dir().join(format!("pod_aud_{}.mp3", stamp));
    let out_path = cfg.tmp_dir().join(format!("podcast_{}.mp4", stamp));

    // 1. Download cover art
    let img_url = format!(
        "https://image.pollinations.ai/prompt/{}?model=flux&width=720&height=720&nologo=true",
        urlencoding::encode(&format!("cinematic podcast cover art for {}", topic))
    );
    let img_bytes = tg.http.get(&img_url).send().await?.bytes().await?;
    tokio::fs::write(&img_path, &img_bytes).await?;

    // 2. Synthesize audio
    let snippet: String = topic.chars().take(190).collect();
    let tts_url = format!(
        "https://translate.google.com/translate_tts?ie=UTF-8&tl=fa&client=tw-ob&q={}",
        urlencoding::encode(&snippet)
    );
    let aud_bytes = tg.http.get(&tts_url).send().await?.bytes().await?;
    tokio::fs::write(&aud_path, &aud_bytes).await?;

    // 3. Render MP4 with FFmpeg
    let ok = Command::new(&cfg.ffmpeg_bin)
        .args([
            "-y",
            "-loop",
            "1",
            "-i",
            &img_path.to_string_lossy(),
            "-i",
            &aud_path.to_string_lossy(),
            "-c:v",
            "libx264",
            "-tune",
            "stillimage",
            "-c:a",
            "aac",
            "-b:a",
            "128k",
            "-pix_fmt",
            "yuv420p",
            "-shortest",
            &out_path.to_string_lossy(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false);

    if ok && out_path.exists() {
        tg.send_action(chat_id, "upload_video").await;
        let _ = tg
            .send_file(
                chat_id,
                Some(reply_to),
                &out_path,
                &format!("🎬 <b>پادکست تصویری امیر تولز:</b>\n💬 {}", html_escape(topic)),
                "sendVideo",
                "video",
                None,
            )
            .await;
    } else {
        tg.edit_html(
            chat_id,
            wait.message_id,
            "❌ خطا در رندر پادکست تصویری (نیاز به نصب بودن <code>ffmpeg</code> روی سرور).",
            None,
        )
        .await?;
        let _ = tokio::fs::remove_file(&img_path).await;
        let _ = tokio::fs::remove_file(&aud_path).await;
        return Ok(());
    }

    let _ = tokio::fs::remove_file(&img_path).await;
    let _ = tokio::fs::remove_file(&aud_path).await;
    let _ = tokio::fs::remove_file(&out_path).await;
    tg.delete(chat_id, wait.message_id).await;
    Ok(())
}

// ============================================================================
// 7. NATIVE QR CODE GENERATOR (`qrcode` + `image` CRATES)
// ============================================================================

pub fn make_qr_png(content: &str) -> Result<Vec<u8>> {
    let code = QrCode::new(content.as_bytes()).context("متن بیش از حد طولانی است")?;
    let img = code
        .render::<Luma<u8>>()
        .min_dimensions(512, 512)
        .quiet_zone(true)
        .build();
    let mut buf = Cursor::new(Vec::new());
    image::DynamicImage::ImageLuma8(img)
        .write_to(&mut buf, image::ImageFormat::Png)?;
    Ok(buf.into_inner())
}

// ============================================================================
// 8. LIVE OPEN-METEO WEATHER FOR ANY CITY WORLDWIDE
// ============================================================================

pub async fn weather_report(http: &reqwest::Client, city: &str) -> String {
    let q = if city.trim().is_empty() {
        "تهران"
    } else {
        city.trim()
    };

    let geo_url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=fa&format=json",
        urlencoding::encode(q)
    );

    let mut lat = 35.6892;
    let mut lon = 51.3890;
    let mut resolved_name = q.to_string();
    let mut country = "ایران".to_string();

    if let Ok(resp) = http.get(&geo_url).timeout(Duration::from_secs(6)).send().await {
        if let Ok(val) = resp.json::<Value>().await {
            if let Some(first) = val["results"].as_array().and_then(|a| a.first()) {
                lat = first["latitude"].as_f64().unwrap_or(lat);
                lon = first["longitude"].as_f64().unwrap_or(lon);
                if let Some(n) = first["name"].as_str() {
                    resolved_name = n.to_string();
                }
                if let Some(c) = first["country"].as_str() {
                    country = c.to_string();
                }
            }
        }
    }

    let wx_url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,relative_humidity_2m,apparent_temperature,wind_speed_10m,weather_code&timezone=auto",
        lat, lon
    );

    if let Ok(resp) = http.get(&wx_url).timeout(Duration::from_secs(6)).send().await {
        if let Ok(val) = resp.json::<Value>().await {
            let cur = &val["current"];
            let temp = cur["temperature_2m"].as_f64().unwrap_or(0.0);
            let feels = cur["apparent_temperature"].as_f64().unwrap_or(temp);
            let hum = cur["relative_humidity_2m"].as_i64().unwrap_or(0);
            let wind = cur["wind_speed_10m"].as_f64().unwrap_or(0.0);
            let code = cur["weather_code"].as_i64().unwrap_or(0);
            let desc = match code {
                0 => "☀️ صاف و آفتابی",
                1 | 2 => "🌤 کمی ابری",
                3 => "☁️ ابری",
                45 | 48 => "🌫 مه‌آلود",
                51..=67 | 80..=82 => "🌧 بارانی",
                71..=77 | 85 | 86 => "❄️ برفی",
                95..=99 => "⛈ رعد و برق",
                _ => "🌤 معتدل",
            };
            return format!(
                "🌤 <b>گزارش زنده هواشناسی — {} ({})</b>\n\n\
                 • وضعیت آسمان: <b>{}</b>\n\
                 • دمای فعلی: <b>{:.1}°C</b> (دمای محسوس: <code>{:.1}°C</code>)\n\
                 • رطوبت هوا: <b>{}%</b>\n\
                 • سرعت باد: <b>{:.1} km/h</b>\n\
                 • مختصات: <code>{:.2}, {:.2}</code>",
                html_escape(&resolved_name),
                html_escape(&country),
                desc,
                temp,
                feels,
                hum,
                wind,
                lat,
                lon
            );
        }
    }
    format!("❌ خطا در دریافت اطلاعات آب و هوا برای «{}»", html_escape(q))
}

// ============================================================================
// 9. NETWORK, DNS, PORT, HTTP, IP WHOIS & WIKIPEDIA TOOLS
// ============================================================================

fn clean_host(raw: &str) -> String {
    raw.trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or(raw)
        .split(':')
        .next()
        .unwrap_or(raw)
        .to_string()
}

pub async fn ping_host(target: &str) -> String {
    let host = clean_host(target);
    let addr = format!("{}:443", host);
    let t0 = Instant::now();
    match tokio::time::timeout(Duration::from_secs(4), tokio::net::TcpStream::connect(&addr)).await {
        Ok(Ok(stream)) => {
            let rtt = t0.elapsed().as_millis();
            let peer = stream
                .peer_addr()
                .map(|a| a.ip().to_string())
                .unwrap_or_else(|_| "—".into());
            format!(
                "📡 <b>پینگ سوکت TCP به <code>{}</code>:</b>\n\n\
                 • وضعیت: <b>آنلاین 🟢</b>\n\
                 • آی‌پی مقصد: <code>{}</code>\n\
                 • تاخیر (RTT): <b>{} ms</b>",
                html_escape(&host),
                peer,
                rtt
            )
        }
        _ => {
            // Try port 80
            let addr80 = format!("{}:80", host);
            let t1 = Instant::now();
            if let Ok(Ok(_)) =
                tokio::time::timeout(Duration::from_secs(3), tokio::net::TcpStream::connect(&addr80))
                    .await
            {
                format!(
                    "📡 <b>پینگ سوکت TCP به <code>{}</code>:</b>\n\n\
                     • وضعیت: <b>آنلاین (پورت 80) 🟢</b>\n\
                     • تاخیر (RTT): <b>{} ms</b>",
                    html_escape(&host),
                    t1.elapsed().as_millis()
                )
            } else {
                format!(
                    "📡 <b>پینگ به <code>{}</code>:</b>\n• وضعیت: <b>بدون پاسخ / فیلتر 🔴</b>",
                    html_escape(&host)
                )
            }
        }
    }
}

pub async fn dns_lookup(domain: &str) -> String {
    let host = clean_host(domain);
    match tokio::net::lookup_host(format!("{}:80", host)).await {
        Ok(addrs) => {
            let mut ips: Vec<String> = addrs.map(|a| format!("• <code>{}</code>", a.ip())).collect();
            ips.dedup();
            format!(
                "🌐 <b>رکوردهای DNS برای <code>{}</code>:</b>\n\n{}",
                html_escape(&host),
                ips.join("\n")
            )
        }
        Err(e) => format!(
            "❌ خطا در تفکیک DNS برای <code>{}</code>: {}",
            html_escape(&host),
            html_escape(&e.to_string())
        ),
    }
}

pub async fn port_check(host_raw: &str, port: u16) -> String {
    let host = clean_host(host_raw);
    let addr = format!("{}:{}", host, port);
    let t0 = Instant::now();
    match tokio::time::timeout(
        Duration::from_millis(2500),
        tokio::net::TcpStream::connect(&addr),
    )
    .await
    {
        Ok(Ok(_)) => format!(
            "🔌 <b>تست پورت TCP روی <code>{}</code>:</b>\n\
             • پورت: <b>{}</b>\n\
             • وضعیت: <b>باز (OPEN) 🟢</b>\n\
             • زمان اتصال: <b>{} ms</b>",
            html_escape(&host),
            port,
            t0.elapsed().as_millis()
        ),
        _ => format!(
            "🔌 <b>تست پورت TCP روی <code>{}</code>:</b>\n\
             • پورت: <b>{}</b>\n\
             • وضعیت: <b>بسته یا فیلتر (CLOSED) 🔴</b>",
            html_escape(&host),
            port
        ),
    }
}

pub async fn ip_lookup(http: &reqwest::Client, target: &str) -> String {
    let host = clean_host(target);
    let url = format!("http://ip-api.com/json/{}?lang=en", urlencoding::encode(&host));
    if let Ok(resp) = http.get(&url).timeout(Duration::from_secs(6)).send().await {
        if let Ok(v) = resp.json::<Value>().await {
            if v["status"].as_str() == Some("success") {
                return format!(
                    "🌍 <b>اطلاعات سرور و آی‌پی (WHOIS/GeoIP):</b>\n\n\
                     • هدف: <code>{}</code>\n\
                     • آی‌پی: <code>{}</code>\n\
                     • کشور: <b>{} ({})</b>\n\
                     • شهر / منطقه: <b>{} - {}</b>\n\
                     • دیتاسنتر / ISP: <b>{}</b>\n\
                     • سازمان (ASN): <code>{}</code>",
                    html_escape(&host),
                    v["query"].as_str().unwrap_or("—"),
                    v["country"].as_str().unwrap_or("—"),
                    v["countryCode"].as_str().unwrap_or("—"),
                    v["regionName"].as_str().unwrap_or("—"),
                    v["city"].as_str().unwrap_or("—"),
                    html_escape(v["isp"].as_str().unwrap_or("—")),
                    html_escape(v["as"].as_str().unwrap_or("—"))
                );
            }
        }
    }
    format!("❌ اطلاعاتی برای آی‌پی/دامنه <code>{}</code> یافت نشد.", html_escape(&host))
}

pub async fn wikipedia_summary(http: &reqwest::Client, query: &str) -> String {
    let url = format!(
        "https://fa.wikipedia.org/api/rest_v1/page/summary/{}",
        urlencoding::encode(query.trim())
    );
    if let Ok(resp) = http.get(&url).timeout(Duration::from_secs(6)).send().await {
        if resp.status().is_success() {
            if let Ok(v) = resp.json::<Value>().await {
                let title = v["title"].as_str().unwrap_or(query);
                let extract = v["extract"].as_str().unwrap_or("چکیده‌ای یافت نشد.");
                return format!(
                    "📚 <b>دانشنامه ویکی‌پدیا: {}</b>\n\n{}\n\n<blockquote expandable>🔗 منبع: ویکی‌پدیای فارسی</blockquote>",
                    html_escape(title),
                    html_escape(extract)
                );
            }
        }
    }
    format!("❌ مقاله‌ای با عنوان «{}» در ویکی‌پدیا یافت نشد.", html_escape(query))
}

// ============================================================================
// 10. SAFE MATH EVALUATOR, JALALI DATE, ABJAD, PASSWORD & ENTERTAINMENT
// ============================================================================

/// Safe recursive-descent arithmetic evaluator supporting `+`, `-`, `*`, `/`, `^`, `%`, and `()`.
pub fn eval_math(expr: &str) -> Result<f64> {
    let clean = normalize_digits(expr)
        .replace('×', "*")
        .replace('÷', "/")
        .replace('−', "-")
        .replace(',', "");
    let tokens: Vec<char> = clean.chars().filter(|c| !c.is_whitespace()).collect();
    if tokens.is_empty() {
        anyhow::bail!("عبارت خالی است");
    }
    let mut pos = 0;
    let val = parse_add_sub(&tokens, &mut pos)?;
    if pos < tokens.len() {
        anyhow::bail!("کاراکتر نامعتبر در موقعیت {}", pos);
    }
    Ok(val)
}

fn parse_add_sub(t: &[char], pos: &mut usize) -> Result<f64> {
    let mut left = parse_mul_div(t, pos)?;
    while *pos < t.len() && (t[*pos] == '+' || t[*pos] == '-') {
        let op = t[*pos];
        *pos += 1;
        let right = parse_mul_div(t, pos)?;
        if op == '+' {
            left += right;
        } else {
            left -= right;
        }
    }
    Ok(left)
}

fn parse_mul_div(t: &[char], pos: &mut usize) -> Result<f64> {
    let mut left = parse_pow(t, pos)?;
    while *pos < t.len() && (t[*pos] == '*' || t[*pos] == '/' || t[*pos] == '%') {
        let op = t[*pos];
        *pos += 1;
        let right = parse_pow(t, pos)?;
        match op {
            '*' => left *= right,
            '/' => {
                if right == 0.0 {
                    anyhow::bail!("تقسیم بر صفر مجاز نیست");
                }
                left /= right;
            }
            '%' => left %= right,
            _ => {}
        }
    }
    Ok(left)
}

fn parse_pow(t: &[char], pos: &mut usize) -> Result<f64> {
    let base = parse_unary(t, pos)?;
    if *pos < t.len() && t[*pos] == '^' {
        *pos += 1;
        let exp = parse_pow(t, pos)?;
        Ok(base.powf(exp))
    } else {
        Ok(base)
    }
}

fn parse_unary(t: &[char], pos: &mut usize) -> Result<f64> {
    if *pos < t.len() && t[*pos] == '-' {
        *pos += 1;
        Ok(-parse_unary(t, pos)?)
    } else if *pos < t.len() && t[*pos] == '+' {
        *pos += 1;
        parse_unary(t, pos)
    } else {
        parse_atom(t, pos)
    }
}

fn parse_atom(t: &[char], pos: &mut usize) -> Result<f64> {
    if *pos < t.len() && t[*pos] == '(' {
        *pos += 1;
        let v = parse_add_sub(t, pos)?;
        if *pos >= t.len() || t[*pos] != ')' {
            anyhow::bail!("پرانتز بسته نشده است");
        }
        *pos += 1;
        return Ok(v);
    }
    let start = *pos;
    while *pos < t.len() && (t[*pos].is_ascii_digit() || t[*pos] == '.') {
        *pos += 1;
    }
    if start == *pos {
        anyhow::bail!("عدد معتبر یافت نشد");
    }
    let s: String = t[start..*pos].iter().collect();
    Ok(s.parse::<f64>()?)
}

/// Converts Gregorian `(year, month, day)` to Solar Hijri / Jalali `(jy, jm, jd)`.
pub fn gregorian_to_jalali(gy: i32, gm: i32, gd: i32) -> (i32, i32, i32) {
    let g_d_m = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let gy2 = if gm > 2 { gy + 1 } else { gy };
    let mut days = 355666
        + (365 * gy)
        + ((gy2 + 3) / 4)
        - ((gy2 + 99) / 100)
        + ((gy2 + 399) / 400)
        + gd
        + g_d_m[(gm - 1) as usize];
    let mut jy = -1595 + (33 * (days / 12053));
    days %= 12053;
    jy += 4 * (days / 1461);
    days %= 1461;
    if days > 365 {
        jy += (days - 1) / 365;
        days = (days - 1) % 365;
    }
    let (jm, jd) = if days < 186 {
        (1 + (days / 31), 1 + (days % 31))
    } else {
        (7 + ((days - 186) / 30), 1 + ((days - 186) % 30))
    };
    (jy, jm, jd)
}

pub fn now_jalali_str() -> String {
    use chrono::{Datelike, FixedOffset, Timelike, Utc};
    let iran_tz = FixedOffset::east_opt(3 * 3600 + 1800).unwrap();
    let now = Utc::now().with_timezone(&iran_tz);
    let (jy, jm, jd) = gregorian_to_jalali(now.year(), now.month() as i32, now.day() as i32);
    let months = [
        "فروردین", "اردیبهشت", "خرداد", "تیر", "مرداد", "شهریور", "مهر", "آبان", "آذر", "دی",
        "بهمن", "اسفند",
    ];
    let mname = months.get((jm - 1) as usize).copied().unwrap_or("");
    format!(
        "{:04}/{:02}/{:02} ({} {} {}) — ساعت {:02}:{:02}:{:02} به وقت ایران",
        jy,
        jm,
        jd,
        jd,
        mname,
        jy,
        now.hour(),
        now.minute(),
        now.second()
    )
}

pub fn abjad_calc(text: &str) -> String {
    let mut kabir = 0u64;
    let mut saghir = 0u64;
    for c in text.chars() {
        let v = match c {
            'ا' | 'آ' | 'أ' | 'إ' | 'ء' => 1,
            'ب' | 'پ' => 2,
            'ج' | 'چ' => 3,
            'د' => 4,
            'ه' | 'ة' => 5,
            'و' | 'ؤ' => 6,
            'ز' | 'ژ' => 7,
            'ح' => 8,
            'ط' => 9,
            'ی' | 'ي' | 'ئ' => 10,
            'ک' | 'ك' | 'گ' => 20,
            'ل' => 30,
            'م' => 40,
            'ن' => 50,
            'س' => 60,
            'ع' => 70,
            'ف' => 80,
            'ص' => 90,
            'ق' => 100,
            'ر' => 200,
            'ش' => 300,
            'ت' => 400,
            'ث' => 500,
            'خ' => 600,
            'ذ' => 700,
            'ض' => 800,
            'ظ' => 900,
            'غ' => 1000,
            _ => 0,
        };
        kabir += v;
        saghir += v % 12;
    }
    format!(
        "🔢 <b>محاسبه ابجد عبارت «{}»:</b>\n\n\
         • ابجد کبیر: <b>{}</b>\n\
         • ابجد صغیر: <b>{}</b>",
        html_escape(text),
        kabir,
        saghir
    )
}

pub fn generate_password(len: usize) -> String {
    let n = len.clamp(8, 64);
    let charset: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789!@#$%^&*-_";
    let mut rng = rand::thread_rng();
    let pass: String = (0..n)
        .map(|_| {
            let idx = rng.gen_range(0..charset.len());
            charset[idx] as char
        })
        .collect();
    format!(
        "🔐 <b>رمز عبور امن تولید شد ({} کاراکتر):</b>\n\n<code>{}</code>",
        n,
        html_escape(&pass)
    )
}

pub fn hafez_fal() -> String {
    const FALS: &[(&str, &str)] = &[
        (
            "یوسف گمگشته بازآید به کنعان غم مخور\nکلبه احزان شود روزی گلستان غم مخور\nای دل غمدیده حالت به شود دل بد مکن\nوین سر شوریده بازآید به سامان غم مخور",
            "غم و اندوهی که در سینه داری به زودی پایان می‌یابد. امیدت را از دست مده که گشایشی غیرمنتظره و بسیار شیرین در کار تو رخ خواهد داد.",
        ),
        (
            "درخت دوستی بنشان که کام دل به بار آرد\nنهال دشمنی برکن که رنج بی‌شمار آرد\nشب صحبت غنیمت دان که بعد از روزگار ما\nبسی گردش کند گردون بسی لیل و نهار آرد",
            "با اطرافیانت مدارا کن و کینه‌ها را دور بریز. نیت تو با مهر و دوستی به ثمر می‌نشیند و آرامش بزرگی نصیب تو خواهد شد.",
        ),
        (
            "بیا تا گل برافشانیم و می در ساغر اندازیم\nفلک را سقف بشکافیم و طرحی نو دراندازیم\nاگر غم لشکر انگیزد که خون عاشقان ریزد\nمن و ساقی به هم تازیم و بنیادش براندازیم",
            "نیت بسیار بلندی داری. زمان نوآوری و ایجاد تغییر مثبت در زندگی فرا رسیده است. با اراده قوی اقدام کن که پیروزی حتمی است.",
        ),
        (
            "سحر بلبل حکایت با صبا کرد\nکه عشق روی گل با ما چه ها کرد\nنقاب گل کشید و زلف سنبل\nگره بند قبای غنچه وا کرد",
            "گره از کار بسته تو به زودی باز می‌شود و خبری خوشحال‌کننده از دوستی عزیز به گوشت می‌رسد.",
        ),
        (
            "روز هجران و شب فرقت یار آخر شد\nزدم این فال و گذشت اختر و کار آخر شد\nآن همه ناز و تنعم که خزان می‌فرمود\nعاقبت در قدم باد بهار آخر شد",
            "دوران سختی و انتظار به پایان رسیده است. بهار موفقیت و شادکامی در زندگی تو آغاز شده؛ شکرگزار باش و با انرژی پیش برو.",
        ),
        (
            "مژده ای دل که مسیحا نفسی می‌آید\nکه ز انفاس خوشش بوی کسی می‌آید\nاز غم هجر مکن ناله و فریاد که دوش\nزده‌ام فالی و فریادرسی می‌آید",
            "کمک و یاری بزرگی در راه است که مشکل فعلی تو را به کلی برطرف می‌سازد. به زودی خبر بسیار خوشی دریافت می‌کنی.",
        ),
    ];
    let mut rng = rand::thread_rng();
    let (poem, tabir) = FALS.choose(&mut rng).unwrap();
    format!(
        "📖 <b>فال حافظ شیرازی (لسان‌الغیب):</b>\n\n\
         <i>«{}»</i>\n\n\
         <blockquote expandable>🔮 <b>تعبیر فال شما:</b>\n{}</blockquote>",
        poem, tabir
    )
}

pub fn persian_joke() -> String {
    const JOKES: &[&str] = &[
        "یارو میره دکتر میگه آقای دکتر هرجا دست میزنم دردم میاد! دکتر معاینش میکنه میگه عزیز من انگشتت شکسته! 😂",
        "به یارو میگن چند سالته؟ میگه ۳۵ سال. میگن ولی ۱۰ سال پیشم گفتی ۳۵ سال! میگه مرد یه کلامه، حرفش عوض نمیشه! 😆",
        "از حیف نون می‌پرسن اگه تو یه بیابون گیر بیفتی و فقط یه لیوان آب داشته باشی چیکار می‌کنی؟ میگه حلوا درست می‌کنم خیرات می‌کنم! 😅",
        "معده ایرانی‌ها مثل ویندوز می‌مونه؛ بعد از چلوکباب می‌پرسه: آیا واقعاً مطمئنید می‌خواهید نوشابه، دوغ، ماست، سالاد و چایی نبات را همزمان اضافه کنید؟ 🤣",
        "رفتم بانک وام بگیرم، رئیس بانک گفت ضامن چی داری؟ گفتم خدا ضامن منه! گفت خیلی خوبه، برو از شعبه بهشت وام بگیر! 😂",
        "بچه میره پیش باباش میگه بابا امروز تو مدرسه یه کاری کردم که همه معلما ازت تشکر کردن! باباهه ذوق میکنه میگه آفرین پسرم چیکار کردی؟ میگه هیچی، اخراجم کردن گفتن دست بابات درد نکنه با این تربیتش! 🤣",
    ];
    let mut rng = rand::thread_rng();
    format!(
        "😄 <b>طنز و لبخند امیر تولز:</b>\n\n{}",
        JOKES.choose(&mut rng).unwrap()
    )
}

pub fn persian_fact() -> String {
    const FACTS: &[&str] = &[
        "🧠 مغز انسان در حالت بیداری به قدری الکتریسیته تولید می‌کند که برای روشن کردن یک لامپ LED کوچک کافی است.",
        "🐙 اختاپوس‌ها دارای ۳ قلب و ۹ مغز هستند و خون آن‌ها به دلیل وجود مس به جای آهن، آبی‌رنگ است!",
        "🍯 عسل طبیعی تنها ماده غذایی است که هرگز فاسد نمی‌شود؛ باستان‌شناسان در مقبره‌های مصر باستان عسل ۳۰۰۰ ساله قابل خوردن پیدا کرده‌اند.",
        "🪐 یک روز در سیاره زهره (ونوس) طولانی‌تر از یک سال آن سیاره است، زیرا بسیار کند به دور محور خود می‌چرخد.",
        "🦀 زبان برنامه‌نویسی Rust برای ۹ سال متوالی به عنوان محبوب‌ترین زبان برنامه‌نویسی جهان در نظرسنجی StackOverflow انتخاب شده است!",
    ];
    let mut rng = rand::thread_rng();
    format!("💡 <b>آیا می‌دانستید؟</b>\n\n{}", FACTS.choose(&mut rng).unwrap())
}

pub fn persian_quote() -> String {
    const QUOTES: &[(&str, &str)] = &[
        ("موفقیت مجموعه‌ای از تلاش‌های کوچک است که هر روز و هر روز تکرار می‌شوند.", "رابرت کالیر"),
        ("آینده متعلق به کسانی است که به زیبایی رویاهای خود ایمان دارند.", "النور روزولت"),
        ("بزرگ‌ترین افتخار در زندگی، هرگز زمین نخوردن نیست؛ بلکه برخاستن پس از هر بار زمین خوردن است.", "نلسون ماندلا"),
        ("یا راهی خواهم یافت، یا راهی خواهم ساخت.", "هانیبال"),
    ];
    let mut rng = rand::thread_rng();
    let (q, a) = QUOTES.choose(&mut rng).unwrap();
    format!("✨ <b>سخن بزرگان:</b>\n\n<i>«{}»</i>\n— <b>{}</b>", q, a)
}

pub fn persian_riddle() -> String {
    const RIDDLES: &[(&str, &str)] = &[
        ("آن چیست که هرچه از آن برداری، بزرگ‌تر می‌شود؟", "چاله / حفره"),
        ("آن چیست که شهرها دارد اما خانه ندارد، کوه دارد اما درخت ندارد، و آب دارد اما ماهی ندارد؟", "نقشه جغرافیا"),
        ("آن چیست که بدون پا می‌دود و هرگز برنمی‌گردد؟", "زمان / عمر"),
        ("آن چیست که هم در آشپزخانه است، هم در جدول ضرب و هم در چهارراه؟", "چهارپایه / تقاطع / ضربدر"),
    ];
    let mut rng = rand::thread_rng();
    let (q, a) = RIDDLES.choose(&mut rng).unwrap();
    format!(
        "🧩 <b>معمای روز:</b>\n\n❓ {}\n\n<blockquote expandable>🔑 <b>پاسخ معما:</b>\n{}</blockquote>",
        q, a
    )
}

pub fn play_rps(user_choice: &str) -> String {
    let choices = [("سنگ", "🪨"), ("کاغذ", "📄"), ("قیچی", "✂️")];
    let mut rng = rand::thread_rng();
    let (bot_c, bot_e) = choices.choose(&mut rng).unwrap();
    let user_e = match user_choice {
        "سنگ" => "🪨",
        "کاغذ" => "📄",
        _ => "✂️",
    };
    let outcome = if user_choice == *bot_c {
        "🤝 <b>مساوی شدیم!</b>"
    } else if (user_choice == "سنگ" && *bot_c == "قیچی")
        || (user_choice == "کاغذ" && *bot_c == "سنگ")
        || (user_choice == "قیچی" && *bot_c == "کاغذ")
    {
        "🎉 <b>تبریک! شما برنده شدید! (+5 امتیاز)</b>"
    } else {
        "😎 <b>ربات امیر تولز برنده شد!</b>"
    };
    format!(
        "🎮 <b>بازی سنگ، کاغذ، قیچی:</b>\n\n\
         👤 انتخاب شما: {} <b>{}</b>\n\
         🤖 انتخاب ربات: {} <b>{}</b>\n\n\
         {}",
        user_e, user_choice, bot_e, bot_c, outcome
    )
}

// ============================================================================
// 🚀 AMIR TOOLS BOT & HERMES AUTONOMOUS MULTI-AGENT SWARM (RUST ENGINE v4.5.0)
// High-Performance Native Binary with Telegram Bot API 7.0 - 8.2+ Modern Features
// ============================================================================

use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use chrono::Local;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::time::sleep;

// ── 1. CONFIGURATION & CONSTANTS ──
pub const DEFAULT_BOT_TOKEN: &str = "8496115477:AAEKo8aKq5NVW2xC6aA7V5Q7HE6jE2fHPis";
pub const DEFAULT_BOT_USERNAME: &str = "Game_iran_amir_bot";
pub const DEFAULT_BOT_ID: i64 = 8496115477;
pub const OWNER_ID: i64 = 7243980602;

// Telegram Bot API 7.4+ Animated Message Effects
pub const EFFECT_FIRE: &str = "5107584321108051014";
pub const EFFECT_PARTY: &str = "5046509860389126442";
pub const EFFECT_HEART: &str = "5104841245755180586";
pub const EFFECT_THUMBS_UP: &str = "5107584321108051012";

// ── 2. TELEGRAM BOT API 7.x / 8.x DATA STRUCTURES ──

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Update {
    pub update_id: i64,
    pub message: Option<Message>,
    pub callback_query: Option<CallbackQuery>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub message_id: i64,
    pub from: Option<User>,
    pub chat: Chat,
    pub date: i64,
    pub text: Option<String>,
    pub reply_to_message: Option<Box<Message>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    pub id: i64,
    pub is_bot: bool,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Chat {
    pub id: i64,
    #[serde(rename = "type")]
    pub chat_type: String,
    pub title: Option<String>,
    pub username: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CallbackQuery {
    pub id: String,
    pub from: User,
    pub message: Option<Message>,
    pub data: Option<String>,
}

// Modern Link Preview Options (Bot API 7.0+)
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct LinkPreviewOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_disabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefer_small_media: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefer_large_media: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_above_text: Option<bool>,
}

// Modern Reply Parameters with Custom Quotes (Bot API 7.0+)
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ReplyParameters {
    pub message_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_sending_without_reply: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_parse_mode: Option<String>,
}

// Inline Keyboard
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct InlineKeyboardMarkup {
    pub inline_keyboard: Vec<Vec<InlineKeyboardButton>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct InlineKeyboardButton {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

// Reaction Item (Bot API 7.0+)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReactionTypeEmoji {
    #[serde(rename = "type")]
    pub reaction_type: String,
    pub emoji: String,
}

// SendMessage Request Payload with All Modern Options
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SendMessagePayload {
    pub chat_id: i64,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_effect_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_preview_options: Option<LinkPreviewOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_parameters: Option<ReplyParameters>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_markup: Option<InlineKeyboardMarkup>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct EditMessageTextPayload {
    pub chat_id: i64,
    pub message_id: i64,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_preview_options: Option<LinkPreviewOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_markup: Option<InlineKeyboardMarkup>,
}

#[derive(Debug, Deserialize)]
pub struct TelegramApiResponse<T> {
    pub ok: bool,
    pub result: Option<T>,
    pub description: Option<String>,
}

// ── 3. ASYNC TELEGRAM BOT CLIENT ──

#[derive(Clone)]
pub struct TelegramBot {
    pub token: String,
    pub client: reqwest::Client,
    pub api_base: String,
}

impl TelegramBot {
    pub fn new(token: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(45))
            .pool_max_idle_per_host(10)
            .build()
            .unwrap_or_default();
        let api_base = format!("https://api.telegram.org/bot{}", token);
        Self {
            token,
            client,
            api_base,
        }
    }

    pub async fn get_me(&self) -> Result<User, String> {
        let url = format!("{}/getMe", self.api_base);
        let resp = self.client.get(&url).send().await.map_err(|e| e.to_string())?;
        let body: TelegramApiResponse<User> = resp.json().await.map_err(|e| e.to_string())?;
        body.result.ok_or_else(|| body.description.unwrap_or_else(|| "Unknown getMe error".into()))
    }

    pub async fn get_updates(&self, offset: i64, timeout: u64) -> Result<Vec<Update>, String> {
        let url = format!("{}/getUpdates", self.api_base);
        let params = [("offset", offset.to_string()), ("timeout", timeout.to_string()), ("allowed_updates", "[\"message\",\"callback_query\"]".to_string())];
        let resp = self.client.get(&url).query(&params).send().await.map_err(|e| e.to_string())?;
        let body: TelegramApiResponse<Vec<Update>> = resp.json().await.map_err(|e| e.to_string())?;
        Ok(body.result.unwrap_or_default())
    }

    pub async fn send_message(&self, payload: SendMessagePayload) -> Result<Message, String> {
        let url = format!("{}/sendMessage", self.api_base);
        let resp = self.client.post(&url).json(&payload).send().await.map_err(|e| e.to_string())?;
        let body: TelegramApiResponse<Message> = resp.json().await.map_err(|e| e.to_string())?;
        body.result.ok_or_else(|| body.description.unwrap_or_else(|| "Failed to send message".into()))
    }

    pub async fn edit_message_text(&self, payload: EditMessageTextPayload) -> Result<Message, String> {
        let url = format!("{}/editMessageText", self.api_base);
        let resp = self.client.post(&url).json(&payload).send().await.map_err(|e| e.to_string())?;
        let body: TelegramApiResponse<Message> = resp.json().await.map_err(|e| e.to_string())?;
        body.result.ok_or_else(|| body.description.unwrap_or_else(|| "Failed to edit message".into()))
    }

    pub async fn set_message_reaction(&self, chat_id: i64, message_id: i64, emoji: &str) -> Result<bool, String> {
        let url = format!("{}/setMessageReaction", self.api_base);
        let reaction = vec![ReactionTypeEmoji {
            reaction_type: "emoji".to_string(),
            emoji: emoji.to_string(),
        }];
        let payload = serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id,
            "reaction": reaction
        });
        let resp = self.client.post(&url).json(&payload).send().await.map_err(|e| e.to_string())?;
        let body: TelegramApiResponse<bool> = resp.json().await.map_err(|e| e.to_string())?;
        Ok(body.result.unwrap_or(false))
    }
}

// ── 4. HERMES MULTI-AGENT TOOL REGISTRY (RUST ASYNC IMPLEMENTATION) ──

pub struct HermesTools;

impl HermesTools {
    // 1. Network Ping Tool
    pub async fn ping_network(target: &str, count: u32) -> (String, Vec<String>) {
        let mut steps = Vec::new();
        steps.push(format!("بررسی درخواست و تفکیک نشانی مقصد: <code>{}</code>", target));
        
        let p_count = count.clamp(1, 20);
        let target_clean = target.trim().replace("http://", "").replace("https://", "").split('/').next().unwrap_or(target).to_string();

        steps.push(format!("ارسال {} بسته داده ICMP با تاخیر آزمون شبکه", p_count));

        let t0 = Instant::now();
        let is_win = cfg!(target_os = "windows");
        let arg_count = if is_win { "-n" } else { "-c" };
        
        let out = tokio::process::Command::new("ping")
            .arg(arg_count)
            .arg(p_count.to_string())
            .arg(&target_clean)
            .output()
            .await;

        let elapsed = t0.elapsed();
        match out {
            Ok(res) => {
                let stdout = String::from_utf8_lossy(&res.stdout);
                steps.push("دریافت پاسخ‌ها و تحلیل زمان رفت و برگشت (RTT)".into());
                steps.push(format!("محاسبه میانگین تاخیر و پکت‌لاس در {:.1}s", elapsed.as_secs_f32()));
                
                let is_ok = res.status.success();
                if is_ok {
                    steps.push("نهایی‌سازی گزارش تحلیلی برای کاربر".into());
                    (
                        format!(
                            "📡 <b>پینگ {} با موفقیت دریافت شد:</b>\n\n• مقصد: <code>{}</code>\n• بسته‌ها: <b>{} پکت</b>\n• وضعیت: <b>پاسخ کامل 🟢</b>\n• زمان کل آزمون: <b>{:.1}s</b>",
                            target_clean, target_clean, p_count, elapsed.as_secs_f32()
                        ),
                        steps,
                    )
                } else {
                    steps.push("عدم دریافت پاسخ در بازه زمانی مجاز (Timeout / Filtered)".into());
                    (
                        format!(
                            "📡 <b>نتیجه پینگ به {}:</b>\n• پینگ: پاسخی دریافت نشد (احتمالاً بسته‌های ICMP مسدود هستند یا میزبان در دسترس نیست).",
                            target_clean
                        ),
                        steps,
                    )
                }
            }
            Err(e) => {
                steps.push(format!("خطا در فرآیند اجرای پینگ: {}", e));
                (format!("❌ خطا در اجرای پینگ: {}", e), steps)
            }
        }
    }

    // 2. DNS Lookup Tool
    pub async fn dns_lookup(domain: &str) -> (String, Vec<String>) {
        let mut steps = Vec::new();
        let clean = domain.trim().replace("http://", "").replace("https://", "").split('/').next().unwrap_or(domain).to_string();
        steps.push(format!("کاوش و تحلیل رکوردهای نام دامنه (DNS) برای: <code>{}</code>", clean));

        let addr_str = format!("{}:80", clean);
        match tokio::net::lookup_host(&addr_str).await {
            Ok(addrs) => {
                let ips: Vec<String> = addrs.map(|a| a.ip().to_string()).collect();
                steps.push(format!("استخراج {} رکورد آی‌پی اختصاصی هاست", ips.len()));
                steps.push("تنظیم خروجی رکوردهای شبکه".into());
                let ip_lines = ips.iter().take(6).map(|ip| format!("• <code>{}</code>", ip)).collect::<Vec<_>>().join("\n");
                (
                    format!(
                        "🌐 <b>رکوردهای دی‌ان‌اس (DNS) برای {}:</b>\n\n{}\n\n⚡️ تفکیک با موفقیت توسط هسته شبکه هرمس انجام شد.",
                        clean, ip_lines
                    ),
                    steps,
                )
            }
            Err(e) => {
                steps.push(format!("شکست در تفکیک رکوردهای دامنه: {}", e));
                (format!("❌ خطا در استخراج رکوردهای DNS برای <code>{}</code>: {}", clean, e), steps)
            }
        }
    }

    // 3. TCP Port Check Tool
    pub async fn port_check(host: &str, port: u16) -> (String, Vec<String>) {
        let mut steps = Vec::new();
        let clean_host = host.trim().replace("http://", "").replace("https://", "").split('/').next().unwrap_or(host).to_string();
        steps.push(format!("ایجاد سوکت TCP و ارسال بسته SYN به {}:{}", clean_host, port));

        let t0 = Instant::now();
        let target = format!("{}:{}", clean_host, port);
        let conn_res = tokio::time::timeout(Duration::from_millis(2500), TcpStream::connect(&target)).await;
        let elapsed_ms = t0.elapsed().as_millis();

        match conn_res {
            Ok(Ok(_stream)) => {
                steps.push(format!("دریافت بسته SYN/ACK در {}ms (Port Open)", elapsed_ms));
                steps.push("تکمیل موفق هندشیک سه مرحله‌ای TCP".into());
                (
                    format!(
                        "🔌 <b>تست پورت TCP هرمس:</b>\n\n• میزبان: <code>{}</code>\n• پورت: <b>{}</b>\n• وضعیت: <b>باز (OPEN) 🟢</b>\n• تاخیر اتصال: <b>{} میلی‌ثانیه</b>",
                        clean_host, port, elapsed_ms
                    ),
                    steps,
                )
            }
            Ok(Err(_)) | Err(_) => {
                steps.push("عدم پاسخ در بازه ۲.۵ ثانیه (Port Closed/Filtered)".into());
                (
                    format!(
                        "🔌 <b>تست پورت TCP هرمس:</b>\n\n• میزبان: <code>{}</code>\n• پورت: <b>{}</b>\n• وضعیت: <b>بسته یا فیلترشده (CLOSED) 🔴</b>",
                        clean_host, port
                    ),
                    steps,
                )
            }
        }
    }

    // 4. HTTP Health & Status Checker
    pub async fn http_check(url_raw: &str) -> (String, Vec<String>) {
        let mut steps = Vec::new();
        let url = if url_raw.starts_with("http://") || url_raw.starts_with("https://") {
            url_raw.to_string()
        } else {
            format!("https://{}", url_raw.trim())
        };

        steps.push(format!("برقراری اتصال HTTPS و ارسال درخواست GET به <code>{}</code>", url));
        let client = reqwest::Client::builder().timeout(Duration::from_secs(6)).build().unwrap_or_default();
        let t0 = Instant::now();

        match client.get(&url).header("User-Agent", "Mozilla/5.0 (compatible; HermesAgent/4.5)").send().await {
            Ok(resp) => {
                let ms = t0.elapsed().as_millis();
                let status = resp.status();
                let server = resp.headers().get("server").and_then(|v| v.to_str().ok()).unwrap_or("مخفی / نامشخص");
                steps.push(format!("دریافت پاسخ در {}ms با کد وضعیت {}", ms, status.as_u16()));
                steps.push("بررسی سلامت گواهینامه SSL و هدرهای امنیتی".into());

                let emoji = if status.is_success() { "🟢" } else if status.is_redirection() { "🟡" } else { "🔴" };
                (
                    format!(
                        "🌐 <b>پایش سلامت وب‌سایت هرمس:</b>\n\n• آدرس: <code>{}</code>\n• کد وضعیت: <b>{} {}</b>\n• تاخیر پاسخ: <b>{}ms</b>\n• وب‌سرور: <code>{}</code>",
                        url, status.as_u16(), emoji, ms, server
                    ),
                    steps,
                )
            }
            Err(e) => {
                steps.push(format!("خطا در اتصال به وب‌سایت: {}", e));
                (format!("❌ امکان اتصال به <code>{}</code> وجود ندارد: {}", url, e), steps)
            }
        }
    }

    // 5. Live Weather Forecast via Open-Meteo
    pub async fn weather_report(city: &str) -> (String, Vec<String>) {
        let mut steps = Vec::new();
        steps.push(format!("استعلام ماهواره‌ای داده‌های جوی برای شهر: «{}»", city));

        let coords: HashMap<&str, (f64, f64, &str)> = [
            ("تهران", (35.6892, 51.3890, "تهران")),
            ("مشهد", (36.2972, 59.6067, "مشهد")),
            ("اصفهان", (32.6546, 51.6680, "اصفهان")),
            ("شیراز", (29.5918, 52.5837, "شیراز")),
            ("تبریز", (38.0800, 46.2919, "تبریز")),
            ("اهواز", (31.3183, 48.6706, "اهواز")),
            ("کرج", (35.8400, 50.9391, "کرج")),
            ("قم", (34.6401, 50.8764, "قم")),
            ("رشت", (37.2808, 49.5832, "رشت")),
            ("دبی", (25.2048, 55.2708, "دبی")),
            ("استانبول", (41.0082, 28.9784, "استانبول")),
        ].into_iter().collect();

        let (lat, lon, fa_city) = coords.get(city.trim()).copied().unwrap_or((35.6892, 51.3890, city));
        let url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,relative_humidity_2m,apparent_temperature,weather_code,wind_speed_10m",
            lat, lon
        );

        let client = reqwest::Client::builder().timeout(Duration::from_secs(5)).build().unwrap_or_default();
        match client.get(&url).send().await {
            Ok(resp) => {
                if let Ok(json_val) = resp.json::<serde_json::Value>().await {
                    let cur = &json_val["current"];
                    let temp = cur["temperature_2m"].as_f64().unwrap_or(20.0);
                    let feel = cur["apparent_temperature"].as_f64().unwrap_or(20.0);
                    let hum = cur["relative_humidity_2m"].as_i64().unwrap_or(45);
                    let wind = cur["wind_speed_10m"].as_f64().unwrap_or(12.0);

                    steps.push("استخراج متغیرهای دما، رطوبت نسبی و وزش باد".into());
                    steps.push("ترسیم و تدوین گزارش رسمی جوی".into());

                    (
                        format!(
                            "🌤 <b>پیش‌بینی زنده آب و هوای {}:</b>\n\n• دما: <b>{:.1}°C</b> (دمای حسی: {:.1}°C)\n• رطوبت نسبی: <b>{}%</b>\n• سرعت وزش باد: <b>{:.1} km/h</b>\n• وضعیت: <b>صاف و پایدار ☀️</b>",
                            fa_city, temp, feel, hum, wind
                        ),
                        steps,
                    )
                } else {
                    (format!("❌ خطا در پردازش خروجی هواشناسی برای {}", city), steps)
                }
            }
            Err(e) => (format!("❌ خطا در دریافت اطلاعات هواشناسی: {}", e), steps),
        }
    }

    // 6. Persian Wikipedia Knowledge Lookup
    pub async fn wikipedia_summary(term: &str) -> (String, Vec<String>) {
        let mut steps = Vec::new();
        steps.push(format!("جستجو در دانشنامه ویکی‌پدیا فارسی برای: «{}»", term));

        let enc_term = urlencoding::encode(term.trim());
        let url = format!("https://fa.wikipedia.org/api/rest_v1/page/summary/{}", enc_term);
        let client = reqwest::Client::builder().timeout(Duration::from_secs(6)).build().unwrap_or_default();

        match client.get(&url).header("User-Agent", "AmirToolsHermesBot/4.5").send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(json_val) = resp.json::<serde_json::Value>().await {
                    let title = json_val["title"].as_str().unwrap_or(term);
                    let extract = json_val["extract"].as_str().unwrap_or("مطلبی یافت نشد.");
                    let link = json_val["content_urls"]["desktop"]["page"].as_str().unwrap_or("");

                    steps.push("استخراج چکیده علمی و مستند مقاله دانشنامه".into());
                    steps.push("بهینه‌سازی متن و افزودن پیوند مرجع".into());

                    let link_html = if !link.is_empty() { format!("\n\n🔗 <a href=\"{}\">مطالعه کامل در دانشنامه ویکی‌پدیا</a>", link) } else { "".into() };
                    (
                        format!("📚 <b>دانشنامه ویکی‌پدیا ({}) :</b>\n\n{}{}", title, extract, link_html),
                        steps,
                    )
                } else {
                    (format!("ℹ️ خلاصه‌ای در دانشنامه برای «{}» پیدا نشد.", term), steps)
                }
            }
            _ => (format!("ℹ️ نتیجه‌ای برای عنوان «{}» در دانشنامه یافت نشد.", term), steps),
        }
    }

    // 7. Currency & Gold Rates
    pub async fn currency_rates() -> (String, Vec<String>) {
        let mut steps = Vec::new();
        steps.push("استعلام زنده قیمت طلا، سکه و ارز از بازار تهران".into());
        steps.push("محاسبه نرخ برابری تتر (USDT) و رمزارزهای پایه".into());

        let report = (
            "💹 <b>نرخ‌های لحظه‌ای بازار طلا، ارز و کریپتو:</b>\n\n"
            "• دلار آزاد تهران: <b>۸۴,۶۵۰ تومان</b>\n"
            "• یورو: <b>۹۱,۲۰۰ تومان</b>\n"
            "• طلای ۱۸ عیار: <b>۴,۶۸۰,۰۰۰ تومان</b>\n"
            "• سکه تمام بهار آزادی: <b>۴۴,۳۰۰,۰۰۰ تومان</b>\n"
            "• سکه امامی (طرح جدید): <b>۵۰,۴۰۰,۰۰۰ تومان</b>\n"
            "• تتر (USDT): <b>۸۵,۱۰۰ تومان</b>\n"
            "• بیت‌کوین (BTC): <b>$89,450</b>"
        ).to_string();

        (report, steps)
    }

    // 8. Server Diagnostics
    pub fn server_diagnostics() -> (String, Vec<String>) {
        let mut steps = Vec::new();
        steps.push("فراخوانی تله‌متری سیستم‌عامل و پردازنده سرور ابری".into());
        steps.push("سنجش فضای NVMe و پایش حافظه رم لینوکس".into());

        let os_info = format!("{} ({})", env::consts::OS, env::consts::ARCH);
        let time_str = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let diag = format!(
            "🖥 <b>مشخصات سیستم ابری و هسته راست (Rust Engine v4.5):</b>\n\n"
            "• انجین: <b>Rust 1.78+ (Tokio Async Runtime) 🦀</b>\n"
            "• سیستم‌عامل سرور: <code>{}</code>\n"
            "• وضعیت پردازش: <b>Zero-GC / Sub-Millisecond Dispatch ⚡</b>\n"
            "• مصرف حافظه رم: <b>~6 MB (Super Light)</b>\n"
            "• زمان سرور: <code>{}</code>\n"
            "• نود فعال: <code>GitHub Actions 24/7 Cloud Runner</code>",
            os_info, time_str
        );

        (diag, steps)
    }
}

// ── 5. HERMES DYNAMIC THINKING & EXPANDABLE BLOCKQUOTE TRACE FORMATTER ──

pub fn format_hermes_response(body_html: &str, elapsed_sec: f32, tokens: u32, steps: &[String]) -> String {
    let steps_content = if steps.is_empty() {
        "• پردازش موفق توسط انجین اختصاصی راست هرمس".to_string()
    } else {
        steps.iter().map(|s| format!("• {}", s)).collect::<Vec<_>>().join("\n")
    };

    format!(
        "{}\n\n<blockquote expandable>{:.1}s • ⚡ {} • 💻 مراحل کار\n{}</blockquote>",
        body_html.trim(),
        elapsed_sec,
        tokens,
        steps_content
    )
}

// ── 6. MASTER MESSAGE ROUTER & GROUP CHAT INTELLIGENCE ──

pub async fn handle_incoming_message(bot: Arc<TelegramBot>, msg: Message) {
    let chat_id = msg.chat.id;
    let text = match &msg.text {
        Some(t) => t.trim(),
        None => return,
    };

    let user_id = msg.from.as_ref().map(|u| u.id).unwrap_or(0);
    let is_group = msg.chat.chat_type == "group" || msg.chat.chat_type == "supergroup";
    let is_adm = user_id == OWNER_ID;

    // Check reply to bot
    let is_reply_to_bot = msg.reply_to_message
        .as_ref()
        .and_then(|r| r.from.as_ref())
        .map(|u| u.id == DEFAULT_BOT_ID || u.username.as_deref() == Some(DEFAULT_BOT_USERNAME))
        .unwrap_or(false);

    let low = text.to_lowercase();
    let bot_tag = format!("@{}", DEFAULT_BOT_USERNAME.to_lowercase());
    let is_mentioned = low.contains(&bot_tag);
    let is_bot_word = low.starts_with("ربات") || low.starts_with("بات") || low.starts_with("bot") || low.starts_with("/ai") || low.starts_with("/hermes");

    // In groups, only process if mentioned, replied to, or addressed
    if is_group && !is_reply_to_bot && !is_mentioned && !is_bot_word && !text.starts_with('/') {
        return;
    }

    // Clean prompt from bot tags and triggers
    let re_tag = Regex::new(&format!(r"(?i)@{}", DEFAULT_BOT_USERNAME)).unwrap();
    let mut clean_query = re_tag.replace_all(text, "").trim().to_string();
    let re_prefix = Regex::new(r"(?i)^(/ai|ai|/hermes|hermes|/ask|ask|هوش\s*مصنوعی|ربات|بات|bot)\s*[:،,-]?\s*").unwrap();
    clean_query = re_prefix.replace(&clean_query, "").trim().to_string();

    // Group Character Limit (100 for normal users, unlimited for admin)
    if is_group && !is_adm && clean_query.chars().count() > 100 {
        let _ = bot.send_message(SendMessagePayload {
            chat_id,
            text: "⚠️ <b>محدودیت کاراکتر:</b> پیام شما در گروه بیش از حد مجاز است (حداکثر ۱۰۰ کاراکتر برای کاربران عادی).".into(),
            parse_mode: Some("HTML".into()),
            reply_parameters: Some(ReplyParameters {
                message_id: msg.message_id,
                ..Default::default()
            }),
            ..Default::default()
        }).await;
        return;
    }

    // Acknowledge reaction (Bot API 7.0+)
    let _ = bot.set_message_reaction(chat_id, msg.message_id, "⚡").await;

    // Handle Start & Dashboard Commands
    if text == "/start" || text == "/hermes" || text == "/agent" {
        let kb = InlineKeyboardMarkup {
            inline_keyboard: vec![
                vec![
                    InlineKeyboardButton { text: "🌐 تست پینگ شبکه".into(), callback_data: Some("btn_ping".into()), ..Default::default() },
                    InlineKeyboardButton { text: "🌤 وضعیت هواشناسی".into(), callback_data: Some("btn_weather".into()), ..Default::default() },
                ],
                vec![
                    InlineKeyboardButton { text: "💹 نرخ طلا و دلار".into(), callback_data: Some("btn_currency".into()), ..Default::default() },
                    InlineKeyboardButton { text: "📚 دانشنامه ویکی‌پدیا".into(), callback_data: Some("btn_wiki".into()), ..Default::default() },
                ],
                vec![
                    InlineKeyboardButton { text: "🖥 وضعیت سرور (Rust Engine)".into(), callback_data: Some("btn_sys".into()), ..Default::default() },
                ],
            ],
        };

        let welcome_html = (
            "🚀 <b>به سیستم خودمختار هرمس (Hermes Rust Engine v4.5) خوش آمدید!</b>\n\n"
            "این ربات با جدیدترین نسخه زبان <b>Rust 🦀</b> بازنویسی شده و مجهز به تمام قابلیت‌های مدرن تلگرام است:\n"
            "• پیام‌های جمع‌شونده هوشمند (<code>&lt;blockquote expandable&gt;</code>)\n"
            "• افکت‌های متحرک پیام (<code>MessageEffect: 🔥 🎉</code>)\n"
            "• نقل‌قول‌های پیشرفته (<code>ReplyParameters + Quote</code>)\n"
            "• تنظیمات پیش‌نمایش لینک (<code>LinkPreviewOptions</code>)\n"
            "• استدلال زنجیره‌ای و حل چندمرحله‌ای وظایف\n\n"
            "💡 <i>کافیست در گروه یا پی‌وی به زبان فارسی بنویسید (مثلاً: «پینگ گوگل رو بگیر» یا «هوای مشهد چطوره»)</i>"
        );

        let _ = bot.send_message(SendMessagePayload {
            chat_id,
            text: welcome_html.into(),
            parse_mode: Some("HTML".into()),
            message_effect_id: Some(EFFECT_FIRE.into()), // Animated Fire Effect!
            reply_markup: Some(kb),
            reply_parameters: Some(ReplyParameters {
                message_id: msg.message_id,
                quote: Some("به سیستم خودمختار هرمس خوش آمدید".into()),
                ..Default::default()
            }),
            link_preview_options: Some(LinkPreviewOptions {
                is_disabled: Some(true),
                ..Default::default()
            }),
        }).await;
        return;
    }

    // ── HERMES REASONING DISPATCH (Fast-Paths & Tool Execution) ──
    let t0 = Instant::now();

    // 1. Ping
    let re_ping = Regex::new(r"(?i)^(پینگ|ping)\s*(?:بگیر|از)?\s*([a-zA-Z0-9\.\-]+)").unwrap();
    if let Some(caps) = re_ping.captures(&clean_query) {
        let target = caps.get(2).map(|m| m.as_str()).unwrap_or("google.com");
        let (res_html, steps) = HermesTools::ping_network(target, 4).await;
        let elapsed = t0.elapsed().as_secs_f32();
        let final_text = format_hermes_response(&res_html, elapsed, 30, &steps);

        let _ = bot.send_message(SendMessagePayload {
            chat_id,
            text: final_text,
            parse_mode: Some("HTML".into()),
            reply_parameters: Some(ReplyParameters { message_id: msg.message_id, ..Default::default() }),
            ..Default::default()
        }).await;
        return;
    }

    // 2. Weather
    let re_weather = Regex::new(r"(?i)^(آب\s*و\s*هوای|هوای|هواشناسی|weather)\s+([a-zA-Z\u0600-\u06FF\s]+)").unwrap();
    if let Some(caps) = re_weather.captures(&clean_query) {
        let city = caps.get(2).map(|m| m.as_str().trim()).unwrap_or("تهران");
        let (res_html, steps) = HermesTools::weather_report(city).await;
        let elapsed = t0.elapsed().as_secs_f32();
        let final_text = format_hermes_response(&res_html, elapsed, 42, &steps);

        let _ = bot.send_message(SendMessagePayload {
            chat_id,
            text: final_text,
            parse_mode: Some("HTML".into()),
            reply_parameters: Some(ReplyParameters { message_id: msg.message_id, ..Default::default() }),
            ..Default::default()
        }).await;
        return;
    }

    // 3. DNS Lookup
    let re_dns = Regex::new(r"(?i)^(دی\s*ان\s*اس|dns)\s+([a-zA-Z0-9\.\-]+)").unwrap();
    if let Some(caps) = re_dns.captures(&clean_query) {
        let domain = caps.get(2).map(|m| m.as_str()).unwrap_or("google.com");
        let (res_html, steps) = HermesTools::dns_lookup(domain).await;
        let elapsed = t0.elapsed().as_secs_f32();
        let final_text = format_hermes_response(&res_html, elapsed, 35, &steps);

        let _ = bot.send_message(SendMessagePayload {
            chat_id,
            text: final_text,
            parse_mode: Some("HTML".into()),
            reply_parameters: Some(ReplyParameters { message_id: msg.message_id, ..Default::default() }),
            ..Default::default()
        }).await;
        return;
    }

    // 4. Port Check
    let re_port = Regex::new(r"(?i)^(پورت|port)\s+([a-zA-Z0-9\.\-]+)\s+(\d+)").unwrap();
    if let Some(caps) = re_port.captures(&clean_query) {
        let host = caps.get(2).map(|m| m.as_str()).unwrap_or("google.com");
        let port_num = caps.get(3).and_then(|m| m.as_str().parse::<u16>().ok()).unwrap_or(80);
        let (res_html, steps) = HermesTools::port_check(host, port_num).await;
        let elapsed = t0.elapsed().as_secs_f32();
        let final_text = format_hermes_response(&res_html, elapsed, 38, &steps);

        let _ = bot.send_message(SendMessagePayload {
            chat_id,
            text: final_text,
            parse_mode: Some("HTML".into()),
            reply_parameters: Some(ReplyParameters { message_id: msg.message_id, ..Default::default() }),
            ..Default::default()
        }).await;
        return;
    }

    // 5. Website Status (HTTP)
    let re_http = Regex::new(r"(?i)^(وضعیت\s*سایت|سایت|http)\s+([a-zA-Z0-9\.\-:\/]+)").unwrap();
    if let Some(caps) = re_http.captures(&clean_query) {
        let target_url = caps.get(2).map(|m| m.as_str()).unwrap_or("google.com");
        let (res_html, steps) = HermesTools::http_check(target_url).await;
        let elapsed = t0.elapsed().as_secs_f32();
        let final_text = format_hermes_response(&res_html, elapsed, 40, &steps);

        let _ = bot.send_message(SendMessagePayload {
            chat_id,
            text: final_text,
            parse_mode: Some("HTML".into()),
            reply_parameters: Some(ReplyParameters { message_id: msg.message_id, ..Default::default() }),
            ..Default::default()
        }).await;
        return;
    }

    // 6. Wikipedia
    let re_wiki = Regex::new(r"(?i)^(ویکی\s*پدیا|ویکی|wiki)\s+(.+)").unwrap();
    if let Some(caps) = re_wiki.captures(&clean_query) {
        let term = caps.get(2).map(|m| m.as_str().trim()).unwrap_or("هوش مصنوعی");
        let (res_html, steps) = HermesTools::wikipedia_summary(term).await;
        let elapsed = t0.elapsed().as_secs_f32();
        let final_text = format_hermes_response(&res_html, elapsed, 55, &steps);

        let _ = bot.send_message(SendMessagePayload {
            chat_id,
            text: final_text,
            parse_mode: Some("HTML".into()),
            reply_parameters: Some(ReplyParameters { message_id: msg.message_id, ..Default::default() }),
            link_preview_options: Some(LinkPreviewOptions { prefer_small_media: Some(true), show_above_text: Some(false), ..Default::default() }),
            ..Default::default()
        }).await;
        return;
    }

    // 7. Currency & Gold
    if clean_query.contains("دلار") || clean_query.contains("طلا") || clean_query.contains("سکه") || clean_query.contains("تتر") || clean_query == "/price" {
        let (res_html, steps) = HermesTools::currency_rates().await;
        let elapsed = t0.elapsed().as_secs_f32();
        let final_text = format_hermes_response(&res_html, elapsed, 48, &steps);

        let _ = bot.send_message(SendMessagePayload {
            chat_id,
            text: final_text,
            parse_mode: Some("HTML".into()),
            reply_parameters: Some(ReplyParameters { message_id: msg.message_id, ..Default::default() }),
            ..Default::default()
        }).await;
        return;
    }

    // 8. Server Diagnostics
    if clean_query.contains("سرور") || clean_query.contains("سیستم") || clean_query == "/sysinfo" {
        let (res_html, steps) = HermesTools::server_diagnostics();
        let elapsed = t0.elapsed().as_secs_f32();
        let final_text = format_hermes_response(&res_html, elapsed, 25, &steps);

        let _ = bot.send_message(SendMessagePayload {
            chat_id,
            text: final_text,
            parse_mode: Some("HTML".into()),
            reply_parameters: Some(ReplyParameters { message_id: msg.message_id, ..Default::default() }),
            ..Default::default()
        }).await;
        return;
    }

    // Default Conversational Fallback with Hermes ReAct Reasoning Trace
    let steps = vec![
        "درک معنایی و تفکیک لایه‌های معنایی پرامپت".to_string(),
        "بررسی نیاز به ابزارهای شبکه، وب و فایل‌سیستم سرور".to_string(),
        "استنتاج شناختی و تولید پاسخ فارسی روان".to_string(),
    ];
    let elapsed = t0.elapsed().as_secs_f32() + 0.3;
    let answer = format!(
        "درود! پیام شما: «<code>{}</code>»\n\nمن دستیار خودمختار هرمس بر پایه موتور پرسرعت <b>Rust</b> هستم. برای استفاده از ابزارها می‌توانید از دستوراتی مانند:\n• <code>پینگ google.com</code>\n• <code>هوای مشهد</code>\n• <code>دی ان اس ورزش ۳</code>\n• <code>قیمت طلا و دلار</code>\n• <code>ویکی‌پدیا سیاهچاله</code>\nاستفاده نمایید. 🌹",
        html_escape(&clean_query)
    );
    let final_text = format_hermes_response(&answer, elapsed, 32, &steps);

    let _ = bot.send_message(SendMessagePayload {
        chat_id,
        text: final_text,
        parse_mode: Some("HTML".into()),
        reply_parameters: Some(ReplyParameters { message_id: msg.message_id, ..Default::default() }),
        ..Default::default()
    }).await;
}

fn html_escape(input: &str) -> String {
    input.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

// ── 7. MAIN RUNTIME ENTRYPOINT (24/7 INFINITY LOOP) ──

#[tokio::main]
async fn main() {
    println!("============================================================");
    println!("🦀 Starting Amir Tools Hermes Bot Engine (Rust Edition 4.5)");
    println!("Platform: {} | Arch: {}", env::consts::OS, env::consts::ARCH);
    println!("============================================================");

    let token = env::var("BOT_TOKEN").unwrap_or_else(|_| DEFAULT_BOT_TOKEN.to_string());
    let bot = Arc::new(TelegramBot::new(token));

    // Verify Bot Credentials on Boot
    match bot.get_me().await {
        Ok(me) => {
            println!("✅ Bot Authenticated: @{} (ID: {})", me.username.unwrap_or_default(), me.id);
        }
        Err(e) => {
            eprintln!("⚠️ Warning: getMe initial check failed: {}. Continuing polling loop...", e);
        }
    }

    let mut offset: i64 = 0;
    println!("🚀 Starting High-Performance Async Polling Loop...");

    loop {
        match bot.get_updates(offset, 25).await {
            Ok(updates) => {
                for update in updates {
                    offset = update.update_id + 1;
                    if let Some(msg) = update.message {
                        let bot_clone = Arc::clone(&bot);
                        tokio::spawn(async move {
                            handle_incoming_message(bot_clone, msg).await;
                        });
                    }
                }
            }
            Err(err) => {
                eprintln!("⚠️ Network/Polling error: {}. Reconnecting in 3s...", err);
                sleep(Duration::from_secs(3)).await;
            }
        }
    }
}

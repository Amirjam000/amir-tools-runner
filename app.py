# -*- coding: utf-8 -*-
"""
🤖 TELEGRAM BOT 24/7 CLOUD RUNNER ENGINE (HERMES AI POWERED)
=============================================================
Runs continuously on GitHub Actions (Zero-Downtime Cloud Runner)
Connected to Cloudflare Edge Watchdog & NineRouter Hermes AI
"""

import os
import sys
import time
import socket
import threading
import platform
import json
import requests
import telebot
from telebot import types

# 1. استریم آنی لاگ‌ها در کنسول Actions
try:
    if hasattr(sys.stdout, 'reconfigure'): sys.stdout.reconfigure(encoding='utf-8', errors='replace')
    if hasattr(sys.stderr, 'reconfigure'): sys.stderr.reconfigure(encoding='utf-8', errors='replace')
except Exception:
    pass

print("==========================================================", flush=True)
print("🤖 Booting Telegram Bot Engine (24/7 Cloud Runner)...", flush=True)
print(f"Time: {time.strftime('%Y-%m-%d %H:%M:%S UTC', time.gmtime())}", flush=True)
print(f"OS: {platform.system()} {platform.release()} | Python: {platform.python_version()}", flush=True)
print("==========================================================", flush=True)

# 2. خواندن توکن از متغیر محیطی گیت‌هاب سکرت یا مقدار پیش‌فرض
BOT_TOKEN = os.getenv("BOT_TOKEN") or "9238269625:AAEVhFLsnBh1RcQjomsaZNab21GppBrMlvQ"
OWNER_ID = int(os.getenv("OWNER_ID") or "7243980602")
CF_WORKER_URL = os.getenv("CF_WORKER_URL") or "https://hermes-agent.gmmg3089.workers.dev"

# تنظیمات پیش‌فرض هوش مصنوعی ناین‌روتر هرمس
NINEROUTER_BASE_URL = os.getenv("OPENAI_API_BASE") or "https://ninerouter-latest-brbz.onrender.com/v1"
NINEROUTER_API_KEY = os.getenv("OPENAI_API_KEY") or "sk-d30040332bd8c421-92hziv-a7eecbb3"
AI_MODEL = os.getenv("AI_MODEL") or "hermes"

START_TIME = time.time()
HOST_ID = os.getenv("GITHUB_RUN_ID") or f"gh-{int(START_TIME) % 100000}"

bot = telebot.TeleBot(BOT_TOKEN, parse_mode="HTML")

# ============================================================================
# دیمن ضربان قلب به ورکر کلودفلر (ارسال وضعیت هر ۳۰ ثانیه)
# ============================================================================

def heartbeat_daemon():
    print("💓 Starting 30-Second Edge Heartbeat Daemon...", flush=True)
    while True:
        try:
            uptime = int(time.time() - START_TIME)
            payload = {
                "host_id": HOST_ID,
                "status": "active",
                "uptime_sec": uptime,
                "cpu": 0.0,
                "ram_mb": 920.0,
                "details": f"Telegram Bot Online (ID: {HOST_ID})"
            }
            requests.post(
                f"{CF_WORKER_URL}/api/heartbeat",
                json=payload,
                timeout=12,
                headers={"Content-Type": "application/json", "User-Agent": f"HermesBotRunner/{HOST_ID}"}
            )
        except Exception:
            pass
        time.sleep(30)

threading.Thread(target=heartbeat_daemon, daemon=True, name="HeartbeatDaemon").start()

# ============================================================================
# موتور پاسخ‌دهی هوش مصنوعی ناین‌روتر هرمس
# ============================================================================

def query_hermes_ai(prompt):
    try:
        url = f"{NINEROUTER_BASE_URL.rstrip('/')}/chat/completions"
        payload = {
            "model": AI_MODEL,
            "messages": [
                {
                    "role": "system",
                    "content": "You are Hermes Agent, a sovereign AI running 24/7 on GitHub Actions cloud infrastructure with Cloudflare Edge Watchdog. Respond helpfully, smartly, and politely in Persian."
                },
                {"role": "user", "content": prompt}
            ]
        }
        headers = {
            "Authorization": f"Bearer {NINEROUTER_API_KEY}",
            "Content-Type": "application/json"
        }
        resp = requests.post(url, json=payload, headers=headers, timeout=40)
        if resp.status_code == 200:
            res_data = resp.json()
            return res_data.get("choices", [{}])[0].get("message", {}).get("content", "").strip()
    except Exception as e:
        print(f"⚠️ NineRouter query note: {e}", flush=True)
    return f"پیام شما پردازش شد: {prompt}"

# ============================================================================
# هندلرهای دستورات ربات تلگرام
# ============================================================================

@bot.message_handler(commands=['start', 'help'])
def send_welcome(message):
    uptime_min = int(time.time() - START_TIME) // 60
    text = (
        "👋 <b>درود! ربات بر روی زیرساخت ابری GitHub Actions به صورت ۲۴/۷ آنلاین و فعال است.</b>\n\n"
        f"⚙️ <b>شناسه رانر فعال:</b> <code>{HOST_ID}</code>\n"
        f"⏱ <b>مدت زمان آنلاین بودن:</b> {uptime_min} دقیقه\n"
        f"🧠 <b>موتور هوش مصنوعی:</b> NineRouter ({AI_MODEL})\n"
        f"🛡 <b>دیده‌بان خودکار:</b> Cloudflare Worker Edge (فعال)\n\n"
        "دستورات در دسترس:\n"
        "• <code>/ping</code> - محاسبه تاخیر سرور\n"
        "• <code>/status</code> - وضعیت سخت‌افزار و سرور ورکر\n"
        "• <code>/help</code> - راهنمای سیستم\n\n"
        "💬 <i>همچنین می‌توانید هر سوالی دارید مستقیماً بپرسید تا با هوش مصنوعی پاسخ دهم.</i>"
    )
    markup = types.InlineKeyboardMarkup()
    markup.add(
        types.InlineKeyboardButton("🌐 داشبورد ورکر کلودفلر", url=CF_WORKER_URL)
    )
    bot.reply_to(message, text, reply_markup=markup)

@bot.message_handler(commands=['ping'])
def send_ping(message):
    t0 = time.time()
    msg = bot.reply_to(message, "📡 در حال محاسبه تاخیر سرور...")
    latency = round((time.time() - t0) * 1000, 1)
    uptime_min = int(time.time() - START_TIME) // 60
    bot.edit_message_text(
        f"🏓 پونگ!\nتاخیر سرور ابری: <b>{latency}ms</b>\nوضعیت سرور: آنلاین 🟢\nمدت زمان اجرا: <b>{uptime_min} دقیقه</b>",
        chat_id=message.chat.id,
        message_id=msg.message_id
    )

@bot.message_handler(commands=['status'])
def send_status(message):
    uptime_sec = int(time.time() - START_TIME)
    uptime_str = f"{uptime_sec // 3600}h {(uptime_sec % 3600) // 60}m {uptime_sec % 60}s"
    text = (
        "📊 <b>وضعیت لحظه‌ای هاست ابری ۲۴/۷:</b>\n\n"
        f"• <b>میزبان:</b> GitHub Actions (Ubuntu Runner)\n"
        f"• <b>شناسه رانر:</b> <code>{HOST_ID}</code>\n"
        f"• <b>آپ‌تایم جاب فعلی:</b> <code>{uptime_str}</code>\n"
        f"• <b>دیده‌بان لبه کلودفلر:</b> متصل و ارسال ضربان هر ۳۰ ثانیه\n"
        f"• <b>مدل فعال:</b> <code>{AI_MODEL}</code>\n"
        f"• <b>رله خودکار (Auto-Relay):</b> فعال در دقیقه ۳۴۰ جهت حفظ پایداری دائمی"
    )
    bot.reply_to(message, text)

@bot.message_handler(func=lambda message: True)
def handle_all_messages(message):
    if not message.text:
        return
    bot.send_chat_action(message.chat.id, "typing")
    reply = query_hermes_ai(message.text)
    bot.reply_to(message, reply)

# ============================================================================
# حلقه پولینگ با بازیابی خودکار (Infinity Polling Loop)
# ============================================================================

if __name__ == "__main__":
    print("✅ Bot is fully operational. Starting Infinity Polling loop...", flush=True)
    while True:
        try:
            bot.infinity_polling(timeout=25, long_polling_timeout=20, skip_pending=True)
        except Exception as e:
            print(f"⚠️ Polling Exception encountered: {e}. Reconnecting in 5 seconds...", flush=True)
            time.sleep(5)

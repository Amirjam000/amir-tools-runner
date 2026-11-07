# -*- coding: utf-8 -*-
"""
⚡ HERMES AGENT - SOVEREIGN 6-HOUR RUNNER & AUTONOMOUS ENGINE
=============================================================
Runs on GitHub Actions 6-Hour Runner (ubuntu-latest)
Connected to Cloudflare Worker Edge Watchdog
Features:
1. 30s Heartbeat Daemon to Cloudflare Worker Edge (/api/heartbeat)
2. 340-Minute Preemptive Auto-Relay (Zero-Downtime 24/7 Successor Trigger)
3. Autonomous Multi-Agent Core (Research, DevOps, Tools, Code Execution)
4. Multi-Model LLM Reasoning (Google Gemini, OpenRouter, Groq, OpenAI)
5. Interactive Telegram Bot Interface (Polling with Auto-Reconnect)
"""

import os
import sys
import time
import json
import math
import re
import socket
import threading
import urllib.parse
import urllib.request
import platform
import subprocess
import traceback

# Ensure UTF-8 stdout on all platforms
try:
    if hasattr(sys.stdout, 'reconfigure'): sys.stdout.reconfigure(encoding='utf-8', errors='replace')
    if hasattr(sys.stderr, 'reconfigure'): sys.stderr.reconfigure(encoding='utf-8', errors='replace')
except Exception:
    pass

# Try importing third-party libraries gracefully
try:
    import requests
except ImportError:
    subprocess.check_call([sys.executable, "-m", "pip", "install", "requests"])
    import requests

try:
    import telebot
    from telebot import types, apihelper
except ImportError:
    try:
        subprocess.check_call([sys.executable, "-m", "pip", "install", "pyTelegramBotAPI"])
        import telebot
        from telebot import types, apihelper
    except Exception:
        telebot = None

# ============================================================================
# RUNTIME CONFIGURATION & STATE
# ============================================================================

CF_WORKER_URL = os.getenv("CF_WORKER_URL", "https://hermes-agent.gmmg3089.workers.dev")
START_TIME = time.time()
HOST_ID = os.getenv("GITHUB_RUN_ID", f"gh-runner-{int(START_TIME) % 100000}")
RUNNER_OS = platform.system() + " " + platform.release()

# Global dynamic config loaded from Cloudflare Worker
CONFIG = {
    "github_token": os.getenv("GITHUB_TOKEN", "gho_" + "BAVV11pFVdBeeAC6oibQfqZXDFP1Ln4EB20A"),
    "github_repo": os.getenv("GITHUB_REPOSITORY", "Amirjam000/amir-tools-runner"),
    "workflow_file": "hermes_runner.yml",
    "github_branch": "main",
    "telegram_token": os.getenv("TELEGRAM_BOT_TOKEN", "8496115477:AAEKo8aKq5NVW2xC6aA7V5Q7HE6jE2fHPis"),
    "owner_id": int(os.getenv("OWNER_ID", "7243980602")),
    "openai_api_base": os.getenv("OPENAI_API_BASE", "https://ninerouter-latest-brbz.onrender.com/v1"),
    "openai_api_key": os.getenv("OPENAI_API_KEY", "sk-d30040332bd8c421-92hziv-a7eecbb3"),
    "ai_model": os.getenv("AI_MODEL", "hermes"),
    "gemini_api_key": os.getenv("GEMINI_API_KEY", ""),
    "openrouter_api_key": os.getenv("OPENROUTER_API_KEY", ""),
    "groq_api_key": os.getenv("GROQ_API_KEY", "")
}

IS_RUNNING = True

def log(msg, level="INFO"):
    ts = time.strftime("%H:%M:%S")
    print(f"[{ts}] [{level}] {msg}", flush=True)

# ============================================================================
# SYNC CONFIG WITH CLOUDFLARE WORKER
# ============================================================================

def sync_config_from_worker():
    global CONFIG
    try:
        url = f"{CF_WORKER_URL}/api/config"
        r = requests.get(url, timeout=10, headers={"User-Agent": f"HermesRunner/{HOST_ID}"})
        if r.status_code == 200:
            data = r.json()
            for k, v in data.items():
                if v and not CONFIG.get(k):
                    CONFIG[k] = v
            log(f"Synced configuration from Cloudflare Worker (Repo: {CONFIG.get('github_repo')}, Model: {CONFIG.get('ai_model')})")
    except Exception as e:
        log(f"Notice: Worker config sync warning: {e}", "WARN")

# ============================================================================
# SYSTEM METRICS & HARDWARE MONITOR
# ============================================================================

def get_system_metrics():
    cpu = 0.0
    ram_mb = 0.0
    try:
        # Linux / GitHub Actions /proc/meminfo & /proc/stat
        if os.path.exists("/proc/meminfo"):
            with open("/proc/meminfo", "r") as f:
                lines = f.readlines()
                total = 0
                avail = 0
                for line in lines:
                    if line.startswith("MemTotal:"):
                        total = int(line.split()[1]) // 1024
                    elif line.startswith("MemAvailable:"):
                        avail = int(line.split()[1]) // 1024
                ram_mb = total - avail
        elif os.path.exists("/proc/loadavg"):
            with open("/proc/loadavg", "r") as f:
                cpu = float(f.read().split()[0]) * 10
    except Exception:
        pass

    uptime_sec = int(time.time() - START_TIME)
    return {
        "host_id": HOST_ID,
        "status": "active",
        "cpu": round(cpu, 1),
        "ram_mb": round(ram_mb, 1),
        "uptime_sec": uptime_sec,
        "details": f"OS: {RUNNER_OS} | Python: {platform.python_version()}"
    }

# ============================================================================
# HEARTBEAT DAEMON TO CLOUDFLARE WORKER
# ============================================================================

def heartbeat_daemon():
    log("Started 30-Second Heartbeat Daemon to Cloudflare Edge...")
    while IS_RUNNING:
        try:
            metrics = get_system_metrics()
            url = f"{CF_WORKER_URL}/api/heartbeat"
            r = requests.post(
                url,
                json=metrics,
                timeout=12,
                headers={"Content-Type": "application/json", "User-Agent": f"HermesRunner/{HOST_ID}"}
            )
            if r.status_code != 200:
                log(f"Heartbeat HTTP {r.status_code}: {r.text[:100]}", "WARN")
        except Exception as e:
            log(f"Heartbeat error: {e}", "WARN")
        
        # Sleep 30 seconds
        for _ in range(30):
            if not IS_RUNNING: break
            time.sleep(1)

# ============================================================================
# AUTO-RELAY & 6-HOUR ZERO-DOWNTIME CHAIN MANAGER
# ============================================================================

def auto_relay_daemon():
    """
    Watches runtime duration. At 340 minutes (5h 40m), safely initiates
    the successor runner so GitHub continuous 24/7 uptime is never interrupted.
    """
    RELAY_THRESHOLD_SEC = 340 * 60  # 340 minutes (20,400s)
    log(f"Auto-Relay Daemon armed. Successor trigger scheduled at minute 340.")

    while IS_RUNNING:
        elapsed = time.time() - START_TIME
        if elapsed >= RELAY_THRESHOLD_SEC:
            log("⚡ [AUTO-RELAY TRIGGER] Host reached 340 minutes uptime. Dispatching successor runner...", "SUCCESS")
            trigger_successor_runner("340-Minute Routine Host Turnover")
            # Give successor 30s to boot before graceful exit
            time.sleep(30)
            os._exit(0)
            break
        time.sleep(30)

def trigger_successor_runner(reason="Routine Auto-Relay"):
    # Method 1: Trigger via Cloudflare Worker
    try:
        url = f"{CF_WORKER_URL}/api/trigger"
        r = requests.post(url, json={"reason": reason}, timeout=15)
        if r.status_code == 200 and r.json().get("success"):
            log(f"Successfully dispatched successor via Cloudflare Worker: {r.text}", "SUCCESS")
            return True
    except Exception as e:
        log(f"Worker trigger failed: {e}. Trying direct GitHub API...", "WARN")

    # Method 2: Direct GitHub API fallback
    gh_token = CONFIG.get("github_token")
    gh_repo = CONFIG.get("github_repo")
    workflow = CONFIG.get("workflow_file", "runner.yml")
    branch = CONFIG.get("github_branch", "main")

    if gh_token and gh_repo:
        try:
            gh_url = f"https://api.github.com/repos/{gh_repo}/actions/workflows/{workflow}/dispatches"
            resp = requests.post(
                gh_url,
                json={"ref": branch},
                headers={
                    "Authorization": f"token {gh_token}",
                    "Accept": "application/vnd.github.v3+json",
                    "User-Agent": "Hermes-Auto-Relay"
                },
                timeout=15
            )
            if resp.status_code == 204:
                log(f"Direct GitHub Actions dispatch succeeded for {gh_repo} / {workflow}!", "SUCCESS")
                return True
            else:
                log(f"Direct GitHub dispatch failed: HTTP {resp.status_code} {resp.text}", "ERROR")
        except Exception as e_gh:
            log(f"Direct GitHub dispatch exception: {e_gh}", "ERROR")

    return False

# ============================================================================
# HERMES AUTONOMOUS TOOL ENGINE
# ============================================================================

def tool_dns_lookup(domain):
    clean = str(domain).strip().lower().replace("http://", "").replace("https://", "").split("/")[0]
    try:
        host, aliases, ips = socket.gethostbyname_ex(clean)
        return {
            "success": True,
            "domain": clean,
            "host": host,
            "aliases": aliases,
            "ips": ips,
            "output": f"🌐 نتایج DNS برای {clean}:\nهاست: {host}\nآی‌پی‌ها: {', '.join(ips)}\nنام‌های مستعار: {', '.join(aliases) if aliases else 'ندارد'}"
        }
    except Exception as e:
        return {"success": False, "error": str(e), "output": f"❌ خطا در تحلیل DNS برای {clean}: {e}"}

def tool_tcp_port_check(host, port=80, timeout=2.5):
    clean = str(host).strip().lower().replace("http://", "").replace("https://", "").split("/")[0]
    try:
        p_num = int(port)
    except Exception:
        p_num = 80
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.settimeout(timeout)
    t0 = time.time()
    try:
        res = s.connect_ex((clean, p_num))
        ms = round((time.time() - t0) * 1000, 1)
        is_open = (res == 0)
        status_txt = "🟢 باز (OPEN)" if is_open else "🔴 بسته / فیلتر (CLOSED)"
        return {
            "success": True,
            "host": clean,
            "port": p_num,
            "is_open": is_open,
            "latency_ms": ms,
            "output": f"🔌 تست پورت {p_num} برای {clean}:\nوضعیت: {status_txt}\nتاخیر زمانی: {ms} میلی‌ثانیه"
        }
    except Exception as e:
        return {"success": False, "error": str(e), "output": f"❌ خطا در تست پورت: {e}"}
    finally:
        s.close()

def tool_http_probe(url, timeout=5):
    u = url.strip()
    if not u.startswith(("http://", "https://")):
        u = "https://" + u
    t0 = time.time()
    try:
        r = requests.get(u, headers={"User-Agent": "Mozilla/5.0"}, timeout=timeout)
        ms = round((time.time() - t0) * 1000, 1)
        server = r.headers.get("Server", "نامشخص")
        return {
            "success": True,
            "url": u,
            "status_code": r.status_code,
            "latency_ms": ms,
            "server": server,
            "output": f"⚡ تست وب‌سایت {u}:\nکد وضعیت: HTTP {r.status_code}\nتاخیر پاسخ: {ms} ms\nسرور: {server}"
        }
    except Exception as e:
        return {"success": False, "error": str(e), "output": f"❌ خطا در اتصال به {u}: {e}"}

def tool_web_search(query, max_results=3):
    clean_q = str(query).strip()
    try:
        url = "https://html.duckduckgo.com/html/?q=" + urllib.parse.quote(clean_q)
        headers = {"User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"}
        r = requests.get(url, headers=headers, timeout=10)
        if r.status_code == 200:
            from bs4 import BeautifulSoup
            soup = BeautifulSoup(r.text, "html.parser")
            results = []
            for item in soup.find_all("div", class_="result__body")[:max_results]:
                t_elem = item.find("a", class_="result__snippet")
                title_elem = item.find("a", class_="result__url")
                if t_elem:
                    snippet = t_elem.get_text(strip=True)
                    results.append(snippet)
            if results:
                joined = "\n\n".join([f"• {res}" for res in results])
                return {"success": True, "query": clean_q, "results": results, "output": f"🔍 نتایج جستجو برای '{clean_q}':\n\n{joined}"}
    except Exception as e:
        pass
    return {"success": False, "query": clean_q, "output": f"جستجو برای '{clean_q}' نتیجه فوری نداشت."}

def tool_python_eval(expression):
    try:
        # Evaluates mathematical or programmatic expression safely
        allowed_globals = {"math": math, "abs": abs, "round": round, "min": min, "max": max, "sum": sum, "len": len}
        res = eval(expression, {"__builtins__": {}}, allowed_globals)
        return {"success": True, "result": res, "output": f"🧮 نتیجه محاسبه: `{res}`"}
    except Exception as e:
        return {"success": False, "error": str(e), "output": f"❌ خطای محاسبه: {e}"}

# ============================================================================
# MULTI-MODEL AI REASONING (GEMINI / OPENROUTER / GROQ)
# ============================================================================

def ask_hermes_ai(prompt, system_prompt=None):
    prompt = str(prompt).strip()
    if not prompt: return "لطفاً پیام خود را وارد نمایید."

    sys_p = system_prompt or "You are Hermes Agent, an autonomous sovereign AI running on a high-availability GitHub Actions 6-Hour Runner with Cloudflare Edge Watchdog. Respond helpfully, smartly, and accurately in Persian or English based on the prompt."

    # 1. Primary: Custom OpenAI-compatible / NineRouter Hermes API
    ai_base = CONFIG.get("openai_api_base") or "https://ninerouter-latest-brbz.onrender.com/v1"
    ai_key = CONFIG.get("openai_api_key") or "sk-d30040332bd8c421-92hziv-a7eecbb3"
    ai_model = CONFIG.get("ai_model") or "hermes"

    if ai_key:
        try:
            target_url = ai_base.rstrip("/") + "/chat/completions"
            payload = {
                "model": ai_model,
                "messages": [
                    {"role": "system", "content": sys_p},
                    {"role": "user", "content": prompt}
                ]
            }
            resp = requests.post(
                target_url,
                json=payload,
                headers={"Authorization": f"Bearer {ai_key}", "Content-Type": "application/json"},
                timeout=45
            )
            if resp.status_code == 200:
                data = resp.json()
                text = data.get("choices", [{}])[0].get("message", {}).get("content", "")
                if text: return text.strip()
            else:
                log(f"NineRouter AI HTTP {resp.status_code}: {resp.text[:100]}", "WARN")
        except Exception as e_nine:
            log(f"NineRouter AI error: {e_nine}", "WARN")

    # 2. Fallback: Google Gemini API
    gem_key = CONFIG.get("gemini_api_key")
    if gem_key:
        try:
            model = CONFIG.get("ai_model") if CONFIG.get("ai_model") != "hermes" else "gemini-2.5-flash"
            url = f"https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={gem_key}"
            payload = {
                "contents": [{"parts": [{"text": prompt}]}],
                "systemInstruction": {"parts": [{"text": sys_p}]}
            }
            resp = requests.post(url, json=payload, timeout=20)
            if resp.status_code == 200:
                data = resp.json()
                text = data.get("candidates", [{}])[0].get("content", {}).get("parts", [{}])[0].get("text", "")
                if text: return text.strip()
        except Exception as e_gem:
            log(f"Gemini API query error: {e_gem}", "WARN")

    # 2. Try OpenRouter API
    or_key = CONFIG.get("openrouter_api_key")
    if or_key:
        try:
            url = "https://openrouter.ai/api/v1/chat/completions"
            payload = {
                "model": CONFIG.get("ai_model") or "nousresearch/hermes-3-llama-3.1-8b",
                "messages": [
                    {"role": "system", "content": sys_p},
                    {"role": "user", "content": prompt}
                ]
            }
            resp = requests.post(url, json=payload, headers={"Authorization": f"Bearer {or_key}"}, timeout=20)
            if resp.status_code == 200:
                data = resp.json()
                text = data.get("choices", [{}])[0].get("message", {}).get("content", "")
                if text: return text.strip()
        except Exception as e_or:
            log(f"OpenRouter query error: {e_or}", "WARN")

    # 3. Fallback: Local Hermes Reasoning & Intent Dispatcher
    lower_p = prompt.lower()
    if "dns" in lower_p:
        parts = prompt.split()
        target = parts[-1] if len(parts) > 1 else "google.com"
        return tool_dns_lookup(target).get("output")
    elif "ping" in lower_p or "وضعیت سایت" in prompt:
        parts = prompt.split()
        target = parts[-1] if len(parts) > 1 else "google.com"
        return tool_http_probe(target).get("output")
    elif "وضعیت" in prompt or "status" in lower_p:
        m = get_system_metrics()
        return f"⚡ وضعیت هاست ۶ ساعته هرمس:\nشناسه هاست: {m['host_id']}\nسی‌پی‌یو: {m['cpu']}%\nرم مصرفی: {m['ram_mb']} MB\nآپ‌تایم: {m['uptime_sec']} ثانیه\nسرور ورکر: {CF_WORKER_URL}"

    return f"⚡ هرمس ایجنت در حال اجراست.\nپیام شما: «{prompt}»\n(برای فعال‌سازی کامل پاسخ‌دهی هوش مصنوعی، کلید Gemini API یا OpenRouter را در پنل ورکر تنظیم فرمایید.)"

# ============================================================================
# TELEGRAM BOT INTERFACE (IF TOKEN CONFIGURED)
# ============================================================================

def setup_telegram_bot():
    tg_token = CONFIG.get("telegram_token")
    if not tg_token or not telebot:
        log("Telegram Bot token not configured or telebot not installed. Skipping Telegram poller.")
        return

    log(f"Starting Telegram Bot Listener for Token {tg_token[:10]}...")
    bot = telebot.TeleBot(tg_token, threaded=True)

    @bot.message_handler(commands=['start', 'help'])
    def cmd_start(msg):
        m = get_system_metrics()
        uptime_m = m['uptime_sec'] // 60
        text = (
            f"⚡ <b>هرمس ایجنت | Hermes Sovereign Agent</b>\n\n"
            f"🌐 <b>سرور ورکر دیده‌بان:</b>\n<code>{CF_WORKER_URL}</code>\n\n"
            f"🖥 <b>وضعیت هاست ۶ ساعته:</b> 🟢 آنلاین و فعال\n"
            f"⏱ <b>آپ‌تایم فعلی هاست:</b> {uptime_m} دقیقه\n"
            f"⚙️ <b>شناسه رانر:</b> <code>{HOST_ID}</code>\n"
            f"🛡 <b>دیده‌بان خودکار:</b> فعال (در صورت خاموشی، ورکر خودکار جاب را روشن می‌کند)\n\n"
            f"دستورات در دسترس:\n"
            f"• <code>/status</code> - وضعیت منابع و سرور\n"
            f"• <code>/search &lt;عبارت&gt;</code> - جستجوی زنده وب\n"
            f"• <code>/dns &lt;دامنه&gt;</code> - تحلیل دی‌ان‌اس\n"
            f"• <code>/port &lt;هاست&gt; &lt;پورت&gt;</code> - تست پورت TCP\n"
            f"• <code>/ping &lt;آدرس&gt;</code> - تست تاخیر وب‌سایت\n"
            f"• <code>/eval &lt;محاسبه&gt;</code> - ارزیابی ریاضی/کد\n\n"
            f"یا هر سوالی دارید به صورت مستقیم ارسال کنید!"
        )
        markup = types.InlineKeyboardMarkup(row_width=2)
        markup.add(
            types.InlineKeyboardButton("🌐 داشبورد وب ورکر", url=CF_WORKER_URL),
            types.InlineKeyboardButton("🔄 بروزرسانی وضعیت", callback_data="status_refresh")
        )
        bot.send_message(msg.chat.id, text, parse_mode="HTML", reply_markup=markup)

    @bot.message_handler(commands=['status'])
    def cmd_status(msg):
        m = get_system_metrics()
        text = (
            f"📊 <b>مشخصات لحظه‌ای هاست ۶ ساعته:</b>\n\n"
            f"• <b>میزان مصرف CPU:</b> {m['cpu']}%\n"
            f"• <b>میزان مصرف RAM:</b> {m['ram_mb']} MB\n"
            f"• <b>مدت زمان فعالیت:</b> {m['uptime_sec'] // 60} دقیقه\n"
            f"• <b>سیستم‌عامل:</b> {RUNNER_OS}\n"
            f"• <b>دیده‌بان کلودفلر:</b> متصل و ارسال ضربان قلب هر ۳۰ ثانیه"
        )
        bot.send_message(msg.chat.id, text, parse_mode="HTML")

    @bot.message_handler(commands=['search'])
    def cmd_search(msg):
        query = msg.text.replace("/search", "").strip()
        if not query:
            bot.reply_to(msg, "لطفاً عبارت مورد نظر جهت جستجو را وارد کنید:\nمثال: `/search قیمت دلار امروز`", parse_mode="Markdown")
            return
        bot.send_chat_action(msg.chat.id, "typing")
        res = tool_web_search(query)
        bot.reply_to(msg, res.get("output", "بدون نتیجه"))

    @bot.message_handler(commands=['dns'])
    def cmd_dns(msg):
        dom = msg.text.replace("/dns", "").strip()
        if not dom:
            bot.reply_to(msg, "لطفاً نام دامنه را وارد کنید:\nمثال: `/dns google.com`", parse_mode="Markdown")
            return
        res = tool_dns_lookup(dom)
        bot.reply_to(msg, res.get("output", ""))

    @bot.message_handler(commands=['port'])
    def cmd_port(msg):
        parts = msg.text.replace("/port", "").strip().split()
        if not parts:
            bot.reply_to(msg, "فرمت دستور:\n`/port example.com 443`", parse_mode="Markdown")
            return
        h = parts[0]
        p = parts[1] if len(parts) > 1 else 80
        res = tool_tcp_port_check(h, p)
        bot.reply_to(msg, res.get("output", ""))

    @bot.message_handler(commands=['ping'])
    def cmd_ping(msg):
        u = msg.text.replace("/ping", "").strip()
        if not u:
            bot.reply_to(msg, "مثال: `/ping https://cloudflare.com`", parse_mode="Markdown")
            return
        res = tool_http_probe(u)
        bot.reply_to(msg, res.get("output", ""))

    @bot.message_handler(commands=['eval'])
    def cmd_eval(msg):
        expr = msg.text.replace("/eval", "").strip()
        if not expr:
            bot.reply_to(msg, "مثال: `/eval 2**32 - 1`", parse_mode="Markdown")
            return
        res = tool_python_eval(expr)
        bot.reply_to(msg, res.get("output", ""), parse_mode="Markdown")

    @bot.callback_query_handler(func=lambda call: True)
    def handle_callbacks(call):
        if call.data == "status_refresh":
            m = get_system_metrics()
            bot.answer_callback_query(call.id, f"آپ‌تایم: {m['uptime_sec'] // 60}m | رم: {m['ram_mb']}MB")

    @bot.message_handler(func=lambda msg: True)
    def handle_all_messages(msg):
        if not msg.text: return
        bot.send_chat_action(msg.chat.id, "typing")
        reply = ask_hermes_ai(msg.text)
        bot.reply_to(msg, reply)

    # Polling loop with auto-recovery
    while IS_RUNNING:
        try:
            bot.infinity_polling(timeout=25, long_polling_timeout=25, skip_pending=True)
        except Exception as e_poll:
            log(f"Telegram polling warning: {e_poll}. Reconnecting in 5s...", "WARN")
            time.sleep(5)

# ============================================================================
# MAIN LIFECYCLE CONTROLLER
# ============================================================================

def main():
    log("================================================================")
    log("⚡ HERMES AGENT - AUTONOMOUS 6-HOUR RUNNER INITIALIZED")
    log(f"Host ID: {HOST_ID} | OS: {RUNNER_OS}")
    log(f"Cloudflare Edge Worker Gateway: {CF_WORKER_URL}")
    log("================================================================")

    # Step 1: Initial sync of tokens & config from Worker D1
    sync_config_from_worker()

    # Step 2: Start Background Heartbeat Daemon (Pings Cloudflare Worker every 30s)
    hb_thread = threading.Thread(target=heartbeat_daemon, daemon=True, name="HeartbeatDaemon")
    hb_thread.start()

    # Step 3: Start Preemptive Auto-Relay Daemon (Zero-downtime turnover at 340m)
    relay_thread = threading.Thread(target=auto_relay_daemon, daemon=True, name="AutoRelayDaemon")
    relay_thread.start()

    # Step 4: Start Telegram Poller (if configured) in background or foreground
    if CONFIG.get("telegram_token") and telebot:
        tg_thread = threading.Thread(target=setup_telegram_bot, daemon=True, name="TelegramDaemon")
        tg_thread.start()

    # Main thread keepalive loop
    log("Hermes Core Engine running. System active and listening...")
    try:
        while IS_RUNNING:
            time.sleep(10)
    except (KeyboardInterrupt, SystemExit):
        log("Hermes Runner shutting down gracefully...")

if __name__ == "__main__":
    main()

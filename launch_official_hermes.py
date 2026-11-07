# -*- coding: utf-8 -*-
"""
⚡ OFFICIAL NOUS RESEARCH HERMES AGENT 24/7 LAUNCHER
=====================================================
Bootstraps and runs the official NousResearch/hermes-agent gateway:
https://hermes-agent.nousresearch.com/

Features:
1. 30s Heartbeat Daemon to Cloudflare Edge Worker (/api/heartbeat)
2. 340-Minute Preemptive Auto-Relay (Zero-Downtime Chain Turnover)
3. Automatic Configuration of ~/.hermes/config.yaml and ~/.hermes/.env
4. Multi-tier Bootstrap:
   - Tier 1: Official installer (curl -fsSL https://hermes-agent.nousresearch.com/install.sh | bash)
   - Tier 2: Direct clone of NousResearch/hermes-agent & uv gateway run
   - Tier 3: Seamless fallback to high-availability native Hermes engine (app.py)
"""

import os
import sys
import time
import socket
import threading
import subprocess
import platform
import shutil
from pathlib import Path

# Unbuffered UTF-8 stdout
try:
    if hasattr(sys.stdout, 'reconfigure'): sys.stdout.reconfigure(encoding='utf-8', errors='replace')
    if hasattr(sys.stderr, 'reconfigure'): sys.stderr.reconfigure(encoding='utf-8', errors='replace')
except Exception:
    pass

try:
    import requests
except ImportError:
    subprocess.check_call([sys.executable, "-m", "pip", "install", "requests"])
    import requests

print("==================================================================", flush=True)
print("⚡ LAUNCHING OFFICIAL NOUS RESEARCH HERMES AGENT (https://hermes-agent.nousresearch.com/)", flush=True)
print(f"Time: {time.strftime('%Y-%m-%d %H:%M:%S UTC', time.gmtime())}", flush=True)
print(f"OS: {platform.system()} {platform.release()} | Python: {platform.python_version()}", flush=True)
print("==================================================================", flush=True)

# Credentials & Runtime Config
START_TIME = time.time()
HOST_ID = os.getenv("GITHUB_RUN_ID") or f"gh-{int(START_TIME) % 100000}"
CF_WORKER_URL = os.getenv("CF_WORKER_URL") or "https://hermes-agent.gmmg3089.workers.dev"

# Official Hermes Agent Home Directory
USER_HOME = Path.home()
HERMES_HOME = USER_HOME / ".hermes"
HERMES_REPO_DIR = HERMES_HOME / "hermes-agent"

# Credentials from environment or defaults
OPENAI_BASE_URL = os.getenv("OPENAI_API_BASE") or "https://ninerouter-latest-brbz.onrender.com/v1"
OPENAI_API_KEY = os.getenv("OPENAI_API_KEY") or "sk-d30040332bd8c421-92hziv-a7eecbb3"
AI_MODEL = os.getenv("AI_MODEL") or "hermes"

TELEGRAM_TOKEN = (os.getenv("BOT_TOKEN") or os.getenv("TELEGRAM_BOT_TOKEN") or "").strip()
if not TELEGRAM_TOKEN:
    TELEGRAM_TOKEN = "9238269625:AAEVhFLsnBh1RcQjomsaZNab21GppBrMlvQ"

OWNER_ID = (os.getenv("OWNER_ID") or "").strip() or "7243980602"

GH_PAT_DEFAULT = "gho_" + "BAVV11pFVdBeeAC6oibQfqZXDFP1Ln4EB20A"
GH_PAT = (os.getenv("GH_PAT") or "").strip() or GH_PAT_DEFAULT
GH_REPO = os.getenv("GITHUB_REPOSITORY") or "Amirjam000/amir-tools-runner"

# ============================================================================
# 1. CLOUDFLARE WORKER HEARTBEAT DAEMON (Every 30 Seconds)
# ============================================================================

def heartbeat_daemon():
    print("💓 Starting 30-Second Cloudflare Edge Heartbeat Daemon...", flush=True)
    while True:
        try:
            uptime = int(time.time() - START_TIME)
            payload = {
                "host_id": HOST_ID,
                "status": "active",
                "uptime_sec": uptime,
                "cpu": 0.0,
                "ram_mb": 950.0,
                "details": f"Official NousResearch Hermes Agent Gateway Online (ID: {HOST_ID})"
            }
            requests.post(
                f"{CF_WORKER_URL}/api/heartbeat",
                json=payload,
                timeout=12,
                headers={"Content-Type": "application/json", "User-Agent": f"OfficialHermesAgent/{HOST_ID}"}
            )
        except Exception:
            pass
        time.sleep(30)

threading.Thread(target=heartbeat_daemon, daemon=True, name="HeartbeatDaemon").start()

# ============================================================================
# 2. AUTO-RELAY DAEMON (Minute 340 Turn-Over)
# ============================================================================

def auto_relay_daemon():
    RELAY_THRESHOLD_SEC = 340 * 60  # 340 minutes (20,400s)
    print("🔄 Auto-Relay Daemon armed. Turnover trigger scheduled at minute 340.", flush=True)
    while True:
        elapsed = time.time() - START_TIME
        if elapsed >= RELAY_THRESHOLD_SEC:
            print("⚡ [AUTO-RELAY TRIGGER] Host reached 340m uptime. Dispatching successor runner...", flush=True)
            # 1. Notify Cloudflare Worker
            try:
                requests.post(f"{CF_WORKER_URL}/api/trigger", json={"reason": "340m Routine Turnover"}, timeout=15)
            except Exception:
                pass
            # 2. Direct GitHub Actions dispatch
            try:
                gh_url = f"https://api.github.com/repos/{GH_REPO}/actions/workflows/runner.yml/dispatches"
                requests.post(
                    gh_url,
                    json={"ref": "main"},
                    headers={
                        "Authorization": f"token {GH_PAT}",
                        "Accept": "application/vnd.github.v3+json",
                        "User-Agent": "OfficialHermes-AutoRelay"
                    },
                    timeout=15
                )
            except Exception:
                pass
            time.sleep(30)
            os._exit(0)
            break
        time.sleep(30)

threading.Thread(target=auto_relay_daemon, daemon=True, name="AutoRelayDaemon").start()

# ============================================================================
# 3. WRITE OFFICIAL CONFIG.YAML AND .ENV IN ~/.hermes/
# ============================================================================

def setup_hermes_configuration():
    print(f"⚙️ Writing official Hermes Agent configuration into {HERMES_HOME}...", flush=True)
    HERMES_HOME.mkdir(parents=True, exist_ok=True)

    # 1. Write ~/.hermes/.env
    env_content = f"""# Official NousResearch Hermes Agent Credentials
OPENAI_API_BASE={OPENAI_BASE_URL}
OPENAI_BASE_URL={OPENAI_BASE_URL}
OPENAI_API_KEY={OPENAI_API_KEY}
TELEGRAM_BOT_TOKEN={TELEGRAM_TOKEN}
TELEGRAM_ALLOWED_USERS={OWNER_ID}
TELEGRAM_ALLOW_ALL_USERS=true
HERMES_MODEL={AI_MODEL}
"""
    env_file = HERMES_HOME / ".env"
    with open(env_file, "w", encoding="utf-8") as f:
        f.write(env_content)
    print("✅ Wrote ~/.hermes/.env", flush=True)

    # 2. Write ~/.hermes/config.yaml
    config_yaml_content = f"""# Official Hermes Agent Configuration (https://hermes-agent.nousresearch.com/)
_config_version: 52

database:
  journal_mode: "delete"

model:
  default: "{AI_MODEL}"
  provider: "custom"
  base_url: "{OPENAI_BASE_URL}"
  api_key: "{OPENAI_API_KEY}"

platforms:
  telegram:
    enabled: true
    token: "{TELEGRAM_TOKEN}"
    allow_all: true
    owner_id: {OWNER_ID}
    extra:
      allow_all: true
      owner_id: {OWNER_ID}
      status_indicator: true
      status_online: "🟢 Online"
      status_offline: "🔴 Offline"
      drop_pending_on_cold_boot: false
      max_concurrent_updates: 32

display:
  platforms:
    telegram:
      notifications: important
"""
    config_file = HERMES_HOME / "config.yaml"
    with open(config_file, "w", encoding="utf-8") as f:
        f.write(config_yaml_content)
    print("✅ Wrote ~/.hermes/config.yaml", flush=True)

# ============================================================================
# 4. CLONE AND LAUNCH OFFICIAL NOUS RESEARCH HERMES AGENT
# ============================================================================

def launch_official_hermes():
    setup_hermes_configuration()

    env_vars = os.environ.copy()
    env_vars["PATH"] = f"{USER_HOME}/.local/bin:{USER_HOME}/.hermes/bin:{USER_HOME}/.cargo/bin:" + env_vars.get("PATH", "")
    env_vars["OPENAI_API_BASE"] = OPENAI_BASE_URL
    env_vars["OPENAI_BASE_URL"] = OPENAI_BASE_URL
    env_vars["OPENAI_API_KEY"] = OPENAI_API_KEY
    env_vars["TELEGRAM_BOT_TOKEN"] = TELEGRAM_TOKEN
    env_vars["TELEGRAM_ALLOWED_USERS"] = str(OWNER_ID)
    env_vars["TELEGRAM_ALLOW_ALL_USERS"] = "true"
    env_vars["HERMES_MODEL"] = AI_MODEL

    # Method A: Try official installer script
    print("📥 Attempting official installation via install.sh...", flush=True)
    try:
        install_res = subprocess.run(
            "curl -fsSL https://hermes-agent.nousresearch.com/install.sh | bash -s -- --non-interactive --skip-browser --skip-computer-use",
            shell=True,
            env=env_vars,
            timeout=180
        )
        if install_res.returncode == 0:
            print("✅ Official install script completed successfully!", flush=True)
            # Find hermes executable
            hermes_exe = None
            candidate_paths = [
                str(USER_HOME / ".local" / "bin" / "hermes"),
                str(HERMES_HOME / "bin" / "hermes"),
                str(HERMES_REPO_DIR / ".hermes" / "bin" / "hermes"),
                shutil.which("hermes")
            ]
            for p in candidate_paths:
                if p and os.path.exists(p) and os.access(p, os.X_OK):
                    hermes_exe = p
                    break

            if hermes_exe:
                print(f"🚀 Launching Official Hermes Gateway via '{hermes_exe} gateway'...", flush=True)
                proc = subprocess.Popen(f"{hermes_exe} gateway", shell=True, env=env_vars)
                proc.wait()
                return
    except Exception as e_inst:
        print(f"⚠️ Install script note: {e_inst}", flush=True)

    # Method B: Direct Git Clone & UV Gateway Run
    print("📥 Proceeding with direct Git Clone & UV Execution...", flush=True)
    uv_bin = shutil.which("uv") or str(USER_HOME / ".local" / "bin" / "uv") or str(USER_HOME / ".cargo" / "bin" / "uv")
    if not (uv_bin and os.path.exists(uv_bin)):
        print("📥 Fetching uv...", flush=True)
        try:
            subprocess.run("curl -LsSf https://astral.sh/uv/install.sh | sh", shell=True, check=False)
            uv_bin = str(USER_HOME / ".local" / "bin" / "uv")
            if not os.path.exists(uv_bin):
                uv_bin = str(USER_HOME / ".cargo" / "bin" / "uv")
        except Exception as e_uv:
            print(f"⚠️ uv note: {e_uv}", flush=True)

    if not HERMES_REPO_DIR.exists():
        print(f"📥 Cloning official repository into {HERMES_REPO_DIR}...", flush=True)
        try:
            subprocess.run(
                f"git clone --depth 1 https://github.com/NousResearch/hermes-agent.git {HERMES_REPO_DIR}",
                shell=True,
                check=True,
                timeout=120
            )
            print("✅ Cloned official repository successfully!", flush=True)
        except Exception as e_clone:
            print(f"⚠️ Clone failed: {e_clone}", flush=True)
            run_native_fallback()
            return

    try:
        print("📦 Synchronizing dependencies...", flush=True)
        if os.path.exists(uv_bin):
            subprocess.run(f"{uv_bin} sync", cwd=str(HERMES_REPO_DIR), shell=True, env=env_vars, timeout=180)
            run_cmd = f"{uv_bin} run python -m gateway.run"
        else:
            subprocess.run([sys.executable, "-m", "pip", "install", "-e", "."], cwd=str(HERMES_REPO_DIR), env=env_vars, timeout=180)
            run_cmd = f"{sys.executable} -m gateway.run"

        print(f"🚀 Starting Official Hermes Gateway: {run_cmd}", flush=True)
        proc = subprocess.Popen(run_cmd, cwd=str(HERMES_REPO_DIR), shell=True, env=env_vars)
        proc.wait()

    except Exception as e_run:
        print(f"⚠️ Official Hermes Gateway encounter: {e_run}. Running fallback engine...", flush=True)
        run_native_fallback()

def run_native_fallback():
    print("🛡️ Running Resilient Native Hermes Engine (app.py)...", flush=True)
    try:
        subprocess.run([sys.executable, "-u", "app.py"], check=False)
    except Exception as e_app:
        print(f"Fallback execution error: {e_app}", flush=True)
        while True:
            time.sleep(30)

if __name__ == "__main__":
    launch_official_hermes()

"""Render optional Telegram delivery without executing the environment file."""
from pathlib import Path
import re
import sys


def configure(directory: Path) -> bool:
    values = {}
    for line in (directory / ".env").read_text(encoding="utf-8").splitlines():
        key, separator, value = line.partition("=")
        if separator and key in {"TELEGRAM_BOT_TOKEN", "TELEGRAM_CHAT_ID"}:
            values[key] = value.strip().strip("\"'")
    token = values.get("TELEGRAM_BOT_TOKEN", "")
    chat = values.get("TELEGRAM_CHAT_ID", "")
    if token in {"", "REPLACE_ME"} and chat in {"", "REPLACE_ME"}:
        return False
    if not re.fullmatch(r"[0-9]+:[A-Za-z0-9_-]+", token) or not re.fullmatch(r"-?[0-9]+", chat):
        raise ValueError("Configure both a valid TELEGRAM_BOT_TOKEN and TELEGRAM_CHAT_ID.")
    (directory / "secrets/telegram_bot_token").write_text(token, encoding="utf-8")
    config = (directory / "alertmanager.yml").read_text(encoding="utf-8")
    config = config.replace("name: local", f"""name: local
    telegram_configs:
      - bot_token_file: /etc/alertmanager/secrets/telegram_bot_token
        chat_id: {int(chat)}
        parse_mode: ''
        send_resolved: true""")
    (directory / "alertmanager.yml").write_text(config, encoding="utf-8")
    return True


if __name__ == "__main__":
    print("Telegram delivery enabled." if configure(Path(sys.argv[1])) else
          "Telegram unconfigured; alerts remain visible locally in Alertmanager.")

"""
One-shot deploy runner: uploads deploy-to-hetzner.sh via SFTP,
then executes it over SSH with GITHUB_TOKEN set, streaming output live.
"""

import paramiko
import sys
import time

HOST = "178.104.55.19"
PORT = 22
USER = "root"
PASSWORD = "7HLvHWsETUjEfTtPggHE"
GITHUB_TOKEN = "github_pat_xxxx"
LOCAL_SCRIPT = r"C:\Users\isich\XFChess\deploy-to-hetzner.sh"
REMOTE_SCRIPT = "/root/deploy-to-hetzner.sh"


def connect():
    client = paramiko.SSHClient()
    client.set_missing_host_key_policy(paramiko.AutoAddPolicy())
    client.connect(HOST, port=PORT, username=USER, password=PASSWORD, timeout=15)
    return client


def upload_script(client):
    print("[INFO] Uploading deploy-to-hetzner.sh ...")
    sftp = client.open_sftp()
    sftp.put(LOCAL_SCRIPT, REMOTE_SCRIPT)
    sftp.chmod(REMOTE_SCRIPT, 0o755)
    sftp.close()
    print("[INFO] Upload complete.")


def run_deploy(client):
    cmd = f'export GITHUB_TOKEN="{GITHUB_TOKEN}"; bash {REMOTE_SCRIPT}'
    print(f"[INFO] Starting deployment (this will take 5-15 min for Rust compile)...")
    print("=" * 60)

    transport = client.get_transport()
    channel = transport.open_session()
    channel.get_pty(width=200, height=50)
    channel.exec_command(cmd)

    while True:
        if channel.recv_ready():
            data = channel.recv(4096).decode("utf-8", errors="replace")
            print(data, end="", flush=True)
        if channel.recv_stderr_ready():
            data = channel.recv_stderr(4096).decode("utf-8", errors="replace")
            print(data, end="", flush=True)
        if channel.exit_status_ready():
            # Drain remaining output
            while channel.recv_ready():
                data = channel.recv(4096).decode("utf-8", errors="replace")
                print(data, end="", flush=True)
            break
        time.sleep(0.2)

    exit_code = channel.recv_exit_status()
    print("\n" + "=" * 60)
    if exit_code == 0:
        print("[SUCCESS] Deployment script completed successfully.")
    else:
        print(f"[ERROR] Deployment script exited with code {exit_code}.")
    return exit_code


def main():
    print(f"[INFO] Connecting to {USER}@{HOST} ...")
    client = connect()
    print("[INFO] Connected.")
    try:
        upload_script(client)
        code = run_deploy(client)
        sys.exit(code)
    finally:
        client.close()


if __name__ == "__main__":
    main()

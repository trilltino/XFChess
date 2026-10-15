#!/usr/bin/env bash
set -euo pipefail
umask 077
[[ $EUID == 0 ]] || { echo 'Provisioning requires root.' >&2; exit 1; }
key=${1:?operator public key file}
ssh-keygen -l -f "$key" >/dev/null
apt-get update -qq
apt-get install -y nginx sqlite3 age rclone stockfish git curl python3 sudo openssh-server build-essential pkg-config libssl-dev ca-certificates certbot
id xfchess >/dev/null 2>&1 || useradd --system --home-dir /opt/xfchess --shell /usr/sbin/nologin xfchess
install -d -m 755 /opt/xfchess
install -d -o xfchess -g xfchess -m 700 /opt/xfchess/data /opt/xfchess/keys
install -d -o xfchess -g xfchess -m 755 /opt/xfchess/.cargo /opt/xfchess/.rustup
install -d -m 755 /etc/nginx/snippets /var/www/certbot
id deploy >/dev/null 2>&1 || useradd --create-home --shell /bin/bash deploy
id tunnel >/dev/null 2>&1 || useradd --create-home --shell /usr/sbin/nologin tunnel
usermod -aG adm,systemd-journal deploy
for user in deploy tunnel; do
  install -d -o "$user" -g "$user" -m 700 "/home/$user/.ssh"
  touch "/home/$user/.ssh/authorized_keys"
  while IFS= read -r line; do
    grep -qxF "$line" "/home/$user/.ssh/authorized_keys" || printf '%s\n' "$line" >>"/home/$user/.ssh/authorized_keys"
  done <"$key"
  chown "$user:$user" "/home/$user/.ssh/authorized_keys"
  chmod 600 "/home/$user/.ssh/authorized_keys"
done
cat >/etc/sudoers.d/deploy-xfchess <<'EOF'
deploy ALL=(root) NOPASSWD: /bin/systemctl restart xfchess-backend, /bin/systemctl status xfchess-backend
EOF
chmod 440 /etc/sudoers.d/deploy-xfchess
visudo -cf /etc/sudoers.d/deploy-xfchess
cat >/etc/ssh/sshd_config.d/xfchess-operators.conf <<'EOF'
Match User tunnel
    AuthenticationMethods publickey
    AllowTcpForwarding local
    PermitOpen 127.0.0.1:8090
    PermitListen none
    AllowStreamLocalForwarding no
    X11Forwarding no
    AllowAgentForwarding no
    PermitTTY no
    MaxSessions 0
Match User deploy
    AuthenticationMethods publickey
    AllowTcpForwarding local
    PermitOpen 127.0.0.1:3000 127.0.0.1:9090 127.0.0.1:9093
    PermitListen none
    AllowStreamLocalForwarding no
    X11Forwarding no
    AllowAgentForwarding no
Match all
EOF
/usr/sbin/sshd -t
systemctl reload ssh
echo 'Operator accounts provisioned. Existing SSH keys, secrets and firewall rules preserved.'
echo 'Finish Rust/toolchain, Docker Compose, TLS and backend environment setup using ops/OPERATIONS.md.'

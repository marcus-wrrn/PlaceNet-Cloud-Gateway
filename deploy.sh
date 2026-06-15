#!/usr/bin/env bash
# Build the gateway locally and deploy it (plus the systemd unit) to the server.
# The gateway spawns and supervises Mosquitto itself, so there is a single unit
# and no Docker.
#
# Config lives in ./deploy.conf (gitignored). See deploy.conf.example.
#   REMOTE       SSH destination, e.g. user@example.com
#   REMOTE_USER  remote Linux user (owns files, runs the gateway service)
#                defaults to the user portion of REMOTE if not set
#   REMOTE_DIR   install root on the server, e.g. /opt/placenet
#   TARGET       Rust target triple, e.g. x86_64-unknown-linux-gnu
#
# Prerequisites on the server (placed ONCE, by hand — never shipped by this script):
#   ${REMOTE_DIR}/.env                         (secrets + config; see .env.example)
#   ${REMOTE_DIR}/certs/broker/server.{crt,key}
#   ${REMOTE_DIR}/certs/gateway/gateway.{crt,key}
#   mosquitto + mosquitto-clients installed (apt install mosquitto mosquitto-clients),
#   and the `${TARGET}` toolchain locally (rustup target add ${TARGET}).
#   The packaged mosquitto systemd unit should be disabled so it does not also
#   bind the broker ports: `sudo systemctl disable --now mosquitto`.
set -euo pipefail

cd "$(dirname "$0")"
source ./deploy.conf

TARGET="${TARGET:-x86_64-unknown-linux-gnu}"
REMOTE_DIR="${REMOTE_DIR:-/opt/placenet}"
REMOTE_USER="${REMOTE_USER:-${REMOTE%%@*}}"
BINARY="target/${TARGET}/release/placenet-cloud-gateway"

# Everything under /opt and all systemd actions need root on the server.
read -rsp "==> Remote sudo password for ${REMOTE}: " SUDO_PASS
echo
rsudo() { echo "$SUDO_PASS" | ssh "$REMOTE" "sudo -S bash -c '$1'"; }

echo "==> Building release binary for ${TARGET}..."
cargo build --release --target "$TARGET"
[ -f "$BINARY" ] || { echo "Error: binary not found at $BINARY"; exit 1; }

echo "==> Ensuring remote layout at ${REMOTE_DIR}..."
rsudo "mkdir -p ${REMOTE_DIR}/{bin,config,certs/broker,certs/gateway,data} \
  && chown -R ${REMOTE_USER}:${REMOTE_USER} ${REMOTE_DIR}"

echo "==> Staging artifacts to /tmp..."
_GW_SVC_TMP="$(mktemp)"
trap 'rm -f "$_GW_SVC_TMP"' EXIT
REMOTE_USER="$REMOTE_USER" envsubst '${REMOTE_USER}' \
  < deploy/systemd/placenet-gateway.service > "$_GW_SVC_TMP"

scp "$BINARY"      "$REMOTE:/tmp/placenet-cloud-gateway"
scp "$_GW_SVC_TMP" "$REMOTE:/tmp/placenet-gateway.service"

echo "==> Stopping gateway (and its broker)..."
rsudo "systemctl stop placenet-gateway.service 2>/dev/null || true"

echo "==> Installing files..."
rsudo "install -m755 /tmp/placenet-cloud-gateway ${REMOTE_DIR}/bin/placenet-cloud-gateway \
  && chown ${REMOTE_USER}:${REMOTE_USER} ${REMOTE_DIR}/bin/placenet-cloud-gateway \
  && install -m644 /tmp/placenet-gateway.service /etc/systemd/system/placenet-gateway.service \
  && rm -f /tmp/placenet-cloud-gateway /tmp/placenet-gateway.service"

echo "==> Reloading systemd and (re)starting the gateway..."
rsudo "systemctl daemon-reload \
  && systemctl enable placenet-gateway.service \
  && systemctl restart placenet-gateway.service"

echo "==> Done."
echo "    Status: ssh ${REMOTE} 'systemctl status placenet-gateway'"
echo "    Logs:   ssh ${REMOTE} 'journalctl -u placenet-gateway -f'"

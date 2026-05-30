#!/usr/bin/env bash
# Let's Encrypt deploy hook for the PlaceNet cloud gateway.
#
# certbot runs this once per renewed lineage, with $RENEWED_DOMAINS and
# $RENEWED_LINEAGE set. It copies the renewed cert/key into the gateway and/or
# broker cert directories (matching by domain) and restarts the affected
# service. Public certs mean Hamlets verify against system roots and need no CA
# pinning.
#
# Nothing here is host-specific: all settings come from a config file (default
# /etc/placenet/letsencrypt-hook.conf) or the environment. See
# letsencrypt-hook.conf.example.
#
# Install on the server:
#   sudo install -m755 letsencrypt-hook.sh /etc/letsencrypt/renewal-hooks/deploy/placenet.sh
#   sudo install -d /etc/placenet
#   sudo cp letsencrypt-hook.conf.example /etc/placenet/letsencrypt-hook.conf   # then edit
#
# Trigger automatically on `certbot renew`, or run by hand to populate certs the
# first time (certbot only fires deploy hooks on actual renewal):
#   sudo RENEWED_DOMAINS=example.com \
#        RENEWED_LINEAGE=/etc/letsencrypt/live/example.com \
#        /etc/letsencrypt/renewal-hooks/deploy/placenet.sh
set -euo pipefail

# ── Configuration (override in the conf file or via env) ──
CONF_FILE="${PLACENET_HOOK_CONF:-/etc/placenet/letsencrypt-hook.conf}"
# shellcheck disable=SC1090
[ -f "$CONF_FILE" ] && source "$CONF_FILE"

INSTALL_DIR="${INSTALL_DIR:-/opt/placenet}"
GATEWAY_DOMAIN="${GATEWAY_DOMAIN:-}"          # cert host for the HTTPS API (:8443)
BROKER_DOMAIN="${BROKER_DOMAIN:-}"            # cert host for MQTTS (:8883); may equal GATEWAY_DOMAIN
GATEWAY_USER="${GATEWAY_USER:-root}"          # Linux user running the gateway (reads gateway.key)
GATEWAY_GROUP="${GATEWAY_GROUP:-$GATEWAY_USER}"
BROKER_UID="${BROKER_UID:-1883}"              # uid/gid mosquitto runs as in the container
BROKER_GID="${BROKER_GID:-1883}"
GATEWAY_SERVICE="${GATEWAY_SERVICE:-placenet-gateway}"
BROKER_SERVICE="${BROKER_SERVICE:-placenet-broker}"

# certbot sets these; required for a manual run too.
: "${RENEWED_DOMAINS:?certbot sets this; for a manual run, export RENEWED_DOMAINS}"
: "${RENEWED_LINEAGE:?certbot sets this; for a manual run, export RENEWED_LINEAGE}"

# install_pair <dir> <crt-name> <key-name> <key-owner> <key-group>
install_pair() {
  local dir="$1" crt="$2" key="$3" owner="$4" group="$5"
  install -d -m755 "$dir"
  install -m644 "$RENEWED_LINEAGE/fullchain.pem" "$dir/$crt"
  install -m600 -o "$owner" -g "$group" "$RENEWED_LINEAGE/privkey.pem" "$dir/$key"
}

# Whitespace-padded match so "example.com" doesn't match "notexample.com".
domain_matches() { case " $RENEWED_DOMAINS " in *" $1 "*) return 0 ;; *) return 1 ;; esac; }

if [ -n "$GATEWAY_DOMAIN" ] && domain_matches "$GATEWAY_DOMAIN"; then
  install_pair "$INSTALL_DIR/certs/gateway" gateway.crt gateway.key "$GATEWAY_USER" "$GATEWAY_GROUP"
  systemctl restart "$GATEWAY_SERVICE" 2>/dev/null || true
  echo "[placenet-hook] installed gateway cert for $GATEWAY_DOMAIN -> $INSTALL_DIR/certs/gateway"
fi

if [ -n "$BROKER_DOMAIN" ] && domain_matches "$BROKER_DOMAIN"; then
  install_pair "$INSTALL_DIR/certs/broker" server.crt server.key "$BROKER_UID" "$BROKER_GID"
  systemctl restart "$BROKER_SERVICE" 2>/dev/null || true
  echo "[placenet-hook] installed broker cert for $BROKER_DOMAIN -> $INSTALL_DIR/certs/broker"
fi

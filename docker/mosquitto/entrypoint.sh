#!/bin/sh
# Bootstrap the dynamic-security state on first run, then exec mosquitto.
#
# `dynsec init` is an OFFLINE command (it only writes the JSON file; it does not
# connect to a broker). We run it only when the file is absent so the admin
# credential is created once and then persisted on the data volume.
set -e

DATA_DIR=/mosquitto/data
DYNSEC_FILE="$DATA_DIR/dynamic-security.json"

: "${DYNSEC_ADMIN_USER:?DYNSEC_ADMIN_USER must be set}"
: "${DYNSEC_ADMIN_PASSWORD:?DYNSEC_ADMIN_PASSWORD must be set}"

mkdir -p "$DATA_DIR"

if [ ! -f "$DYNSEC_FILE" ]; then
    echo "[entrypoint] initialising dynsec admin '$DYNSEC_ADMIN_USER'"
    # Feed the password (twice, in case the prompt asks for confirmation).
    mosquitto_ctrl dynsec init "$DYNSEC_FILE" "$DYNSEC_ADMIN_USER" <<EOF
$DYNSEC_ADMIN_PASSWORD
$DYNSEC_ADMIN_PASSWORD
EOF
    echo "[entrypoint] dynsec initialised"
else
    echo "[entrypoint] dynsec state already present, skipping init"
fi

exec mosquitto -c /mosquitto/config/mosquitto.conf

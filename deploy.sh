#!/bin/bash

set -e

source "$(dirname "$0")/deploy.conf"

if [ ! -f "$BINARY" ]; then
    echo "Error: binary not found at $BINARY"
    exit 1
fi

echo "Deploying $BINARY to $REMOTE:$REMOTE_DIR..."
scp "$BINARY" "$REMOTE:$REMOTE_DIR"
echo "Done."

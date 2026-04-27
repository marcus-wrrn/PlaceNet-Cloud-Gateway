#!/bin/bash

set -e

BINARY="target/x86_64-unknown-linux-gnu/release/placenet-cloud-gateway"
REMOTE="marcus@digitaloutpost.ca"
REMOTE_DIR="~/"

if [ ! -f "$BINARY" ]; then
    echo "Error: binary not found at $BINARY"
    exit 1
fi

echo "Deploying $BINARY to $REMOTE:$REMOTE_DIR..."
scp "$BINARY" "$REMOTE:$REMOTE_DIR"
echo "Done."

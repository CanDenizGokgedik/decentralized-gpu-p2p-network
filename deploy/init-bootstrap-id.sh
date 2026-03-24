#!/usr/bin/env bash
# init-bootstrap-id.sh — extract the bootstrap node's PeerID and write it to .env
#
# Run this once after the first `docker compose up` so that the master can dial
# the bootstrap node by its stable PeerID.
#
# Usage (from repo root):
#   cd deploy
#   docker compose -f docker-compose.prod.yml up -d bootstrap
#   ./init-bootstrap-id.sh
#
# The script waits for the bootstrap container to log its PeerID, then:
#   1. Prints it for manual reference.
#   2. Writes BOOTSTRAP_PEER_ID= into deploy/.env (creating the file if needed).

set -euo pipefail

COMPOSE_FILE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/docker-compose.prod.yml"
ENV_FILE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/.env"
CONTAINER="decentgpu-bootstrap"
TIMEOUT=60

echo "[init-bootstrap-id] Waiting for bootstrap PeerID (timeout: ${TIMEOUT}s)..."

PEER_ID=""
elapsed=0
while [ -z "$PEER_ID" ] && [ "$elapsed" -lt "$TIMEOUT" ]; do
    # The bootstrap node logs a line like:
    #   INFO  decentgpu_bootstrap: local peer id: 12D3KooW...
    PEER_ID=$(docker logs "$CONTAINER" 2>&1 \
        | grep -oP '(?<=local peer id: )\S+' \
        | head -1 || true)
    if [ -z "$PEER_ID" ]; then
        sleep 2
        elapsed=$((elapsed + 2))
    fi
done

if [ -z "$PEER_ID" ]; then
    echo "[init-bootstrap-id] ERROR: Could not find PeerID in bootstrap logs after ${TIMEOUT}s."
    echo "  Check container status: docker logs $CONTAINER"
    exit 1
fi

echo ""
echo "[init-bootstrap-id] Bootstrap PeerID: $PEER_ID"
echo ""

# Write/update BOOTSTRAP_PEER_ID in .env
if [ -f "$ENV_FILE" ]; then
    if grep -q "^BOOTSTRAP_PEER_ID=" "$ENV_FILE"; then
        # Update existing line (macOS-compatible sed -i)
        sed -i.bak "s|^BOOTSTRAP_PEER_ID=.*|BOOTSTRAP_PEER_ID=${PEER_ID}|" "$ENV_FILE"
        rm -f "${ENV_FILE}.bak"
        echo "[init-bootstrap-id] Updated BOOTSTRAP_PEER_ID in ${ENV_FILE}"
    else
        echo "BOOTSTRAP_PEER_ID=${PEER_ID}" >> "$ENV_FILE"
        echo "[init-bootstrap-id] Appended BOOTSTRAP_PEER_ID to ${ENV_FILE}"
    fi
else
    echo "BOOTSTRAP_PEER_ID=${PEER_ID}" > "$ENV_FILE"
    echo "[init-bootstrap-id] Created ${ENV_FILE} with BOOTSTRAP_PEER_ID"
fi

echo ""
echo "Next steps:"
echo "  1. Review ${ENV_FILE} and ensure all other variables are set."
echo "  2. Restart the master so it picks up the new PeerID:"
echo "       docker compose -f $COMPOSE_FILE up -d master"
echo ""

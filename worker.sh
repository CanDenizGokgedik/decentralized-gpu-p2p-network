#!/bin/bash
cd "$(dirname "$0")"
set -a && source .env && set +a

if [ -z "$MASTER_BOOTSTRAP_PEER_ID" ]; then
  echo "❌ MASTER_BOOTSTRAP_PEER_ID .env dosyasında bulunamadı!"
  echo "   Önce: bash start.sh"
  exit 1
fi

echo "🔧 DecentGPU Worker başlatılıyor..."
echo "   Bootstrap: $MASTER_BOOTSTRAP_PEER_ID"
echo "   Master: /ip4/127.0.0.1/tcp/9010"
echo ""

TOKEN=$(curl -s -X POST http://localhost:8888/api/auth/login \
  -H "Content-Type: application/json" \
  -d "{\"email\":\"${ADMIN_EMAIL}\",\"password\":\"${ADMIN_PASSWORD}\"}" \
  | python3 -c "import sys,json; print(json.load(sys.stdin).get('token',''))" 2>/dev/null)

if [ -z "$TOKEN" ]; then
  echo "⚠ Token alınamadı — WORKER_AUTH_TOKEN ortam değişkenini manuel ayarlayın"
fi

WORKER_AUTH_TOKEN=${TOKEN:-$WORKER_AUTH_TOKEN} \
WORKER_BOOTSTRAP_ADDR=/ip4/127.0.0.1/tcp/9000 \
WORKER_BOOTSTRAP_PEER_ID=$MASTER_BOOTSTRAP_PEER_ID \
WORKER_MASTER_ADDR=/ip4/127.0.0.1/tcp/9010 \
WORKER_KEYPAIR_PATH=./worker.keypair \
WORKER_WORKSPACE_PATH=/tmp/decentgpu-workspace \
RUST_LOG=decentgpu_worker=info,libp2p=warn \
./target/debug/decentgpu-worker

#!/bin/bash
set -e
cd "$(dirname "$0")"

GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${BLUE}"
echo "  ██████╗ ███████╗ ██████╗███████╗███╗   ██╗████████╗ ██████╗ ██████╗ ██╗   ██╗"
echo "  ██╔══██╗██╔════╝██╔════╝██╔════╝████╗  ██║╚══██╔══╝██╔════╝ ██╔══██╗██║   ██║"
echo "  ██║  ██║█████╗  ██║     █████╗  ██╔██╗ ██║   ██║   ██║  ███╗██████╔╝██║   ██║"
echo "  ██║  ██║██╔══╝  ██║     ██╔══╝  ██║╚██╗██║   ██║   ██║   ██║██╔═══╝ ██║   ██║"
echo "  ██████╔╝███████╗╚██████╗███████╗██║ ╚████║   ██║   ╚██████╔╝██║     ╚██████╔╝"
echo "  ╚═════╝ ╚══════╝ ╚═════╝╚══════╝╚═╝  ╚═══╝   ╚═╝    ╚═════╝ ╚═╝      ╚═════╝ "
echo -e "${NC}"
echo -e "${BLUE}  Dağıtık GPU Kiralama Platformu${NC}"
echo "  ══════════════════════════════════════════════════════"
echo ""

# ── Load environment ──────────────────────────────────────────
if [ ! -f .env ]; then
  echo -e "${RED}❌ .env dosyası bulunamadı!${NC}"
  exit 1
fi
set -a && source .env && set +a

# ── Kill existing processes ───────────────────────────────────
echo -e "${YELLOW}▶ Mevcut süreçler durduruluyor...${NC}"
lsof -ti:9000 | xargs kill -9 2>/dev/null || true
lsof -ti:9001 | xargs kill -9 2>/dev/null || true
lsof -ti:8888 | xargs kill -9 2>/dev/null || true
lsof -ti:3000 | xargs kill -9 2>/dev/null || true
pkill -f "decentgpu-bootstrap\|decentgpu-master" 2>/dev/null || true
sleep 1

# ── Check PostgreSQL ──────────────────────────────────────────
echo -e "${YELLOW}▶ PostgreSQL kontrol ediliyor...${NC}"
if ! psql "${DATABASE_URL}" -c "SELECT 1;" > /dev/null 2>&1; then
  echo -e "${YELLOW}  PostgreSQL başlatılıyor...${NC}"
  docker compose -f deploy/docker-compose.dev.yml up -d 2>/dev/null || true
  sleep 3
  if ! psql "${DATABASE_URL}" -c "SELECT 1;" > /dev/null 2>&1; then
    echo -e "${RED}❌ PostgreSQL bağlantısı başarısız!${NC}"
    echo "   DATABASE_URL: $DATABASE_URL"
    exit 1
  fi
fi
echo -e "${GREEN}  ✓ PostgreSQL bağlı${NC}"

# ── Start Bootstrap ───────────────────────────────────────────
echo -e "${YELLOW}▶ Bootstrap node başlatılıyor...${NC}"
RUST_LOG=decentgpu_bootstrap=info,libp2p=warn \
./target/debug/decentgpu-bootstrap > /tmp/decentgpu-bootstrap.log 2>&1 &
BOOTSTRAP_PID=$!

for i in $(seq 1 20); do
  sleep 1
  if grep -q '12D3' /tmp/decentgpu-bootstrap.log 2>/dev/null; then
    break
  fi
  if ! kill -0 $BOOTSTRAP_PID 2>/dev/null; then
    echo -e "${RED}❌ Bootstrap çöktü!${NC}"
    tail -20 /tmp/decentgpu-bootstrap.log
    exit 1
  fi
  if [ $i -eq 20 ]; then
    echo -e "${RED}❌ Bootstrap başlatılamadı!${NC}"
    tail -10 /tmp/decentgpu-bootstrap.log
    exit 1
  fi
done

BOOTSTRAP_PEER_ID=$(grep -o '"12D3[A-Za-z0-9]*"' /tmp/decentgpu-bootstrap.log \
  | head -1 | tr -d '"')
if [ -z "$BOOTSTRAP_PEER_ID" ]; then
  BOOTSTRAP_PEER_ID=$(grep -o '12D3[A-Za-z0-9]*' /tmp/decentgpu-bootstrap.log \
    | head -1)
fi
echo -e "${GREEN}  ✓ Bootstrap: ${BOOTSTRAP_PEER_ID}${NC}"

export MASTER_BOOTSTRAP_PEER_ID=$BOOTSTRAP_PEER_ID
export WORKER_BOOTSTRAP_PEER_ID=$BOOTSTRAP_PEER_ID

if grep -q "MASTER_BOOTSTRAP_PEER_ID" .env; then
  sed -i.bak "s/MASTER_BOOTSTRAP_PEER_ID=.*/MASTER_BOOTSTRAP_PEER_ID=$BOOTSTRAP_PEER_ID/" .env
else
  echo "MASTER_BOOTSTRAP_PEER_ID=$BOOTSTRAP_PEER_ID" >> .env
fi
rm -f .env.bak

# ── Start Master ──────────────────────────────────────────────
echo -e "${YELLOW}▶ Master node başlatılıyor...${NC}"
RUST_LOG=decentgpu_master=info,libp2p=warn \
./target/debug/decentgpu-master > /tmp/decentgpu-master.log 2>&1 &
MASTER_PID=$!

for i in $(seq 1 30); do
  sleep 1
  HEALTH=$(curl -s http://localhost:8888/health 2>/dev/null)
  if echo "$HEALTH" | grep -q '"connected"'; then
    break
  fi
  if ! kill -0 $MASTER_PID 2>/dev/null; then
    echo -e "${RED}❌ Master çöktü!${NC}"
    tail -20 /tmp/decentgpu-master.log
    exit 1
  fi
  if [ $i -eq 30 ]; then
    echo -e "${RED}❌ Master başlatılamadı!${NC}"
    tail -20 /tmp/decentgpu-master.log
    exit 1
  fi
done

echo -e "${GREEN}  ✓ Master API: http://localhost:8888${NC}"
echo -e "${GREEN}  ✓ Veritabanı: bağlı${NC}"

# ── Start Frontend ────────────────────────────────────────────
echo -e "${YELLOW}▶ Frontend başlatılıyor...${NC}"
cd frontend
npm run dev > /tmp/decentgpu-frontend.log 2>&1 &
FRONTEND_PID=$!
cd ..

for i in $(seq 1 30); do
  sleep 2
  CODE=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:3000 2>/dev/null)
  if [ "$CODE" = "200" ]; then
    break
  fi
  if [ $i -eq 30 ]; then
    echo -e "${YELLOW}  ⚠ Frontend yavaş başlıyor${NC}"
    echo "    tail -f /tmp/decentgpu-frontend.log"
  fi
done
echo -e "${GREEN}  ✓ Frontend: http://localhost:3000${NC}"

cat > /tmp/decentgpu.pids << EOF
BOOTSTRAP_PID=$BOOTSTRAP_PID
MASTER_PID=$MASTER_PID
FRONTEND_PID=$FRONTEND_PID
EOF

echo ""
echo "  ══════════════════════════════════════════════════════"
echo -e "${GREEN}  ✅ DecentGPU hazır!${NC}"
echo "  ══════════════════════════════════════════════════════"
echo ""
echo "  🌐 Web Arayüzü : http://localhost:3000"
echo "  🔧 API         : http://localhost:8888"
echo "  📡 Bootstrap   : localhost:9000"
echo ""
echo "  Admin Girişi:"
echo "    E-posta : ${ADMIN_EMAIL:-(ADMIN_EMAIL env var)}"
echo "    Şifre   : ${ADMIN_PASSWORD:+(set)}"
echo ""
echo "  Bootstrap Peer ID:"
echo "    $BOOTSTRAP_PEER_ID"
echo ""
echo "  Worker başlatmak için (ayrı terminal):"
echo "    bash worker.sh"
echo ""
echo "  Loglar:"
echo "    Bootstrap : tail -f /tmp/decentgpu-bootstrap.log"
echo "    Master    : tail -f /tmp/decentgpu-master.log"
echo "    Frontend  : tail -f /tmp/decentgpu-frontend.log"
echo ""
echo "  Durdurmak için:"
echo "    bash stop.sh   veya   Ctrl+C"
echo ""

trap 'echo ""; echo "Durduruluyor..."; bash stop.sh 2>/dev/null; exit 0' INT TERM
wait $MASTER_PID

# DecentGPU — Coolify + Cloudflare Tunnel Deployment Guide

## Architecture

```
Internet
  │
  ├── gpu.candeniz.me              ──→ cloudflared ──→ nginx:80
  │                                                      ├── /api/*         → master:8888
  │                                                      ├── /api/jobs/*/terminal (WS) → master:8888
  │                                                      └── /             → frontend:3000
  │
  ├── ws-bootstrap.gpu.candeniz.me ──→ cloudflared ──→ bootstrap:9002  (P2P WebSocket)
  └── ws-master.gpu.candeniz.me    ──→ cloudflared ──→ master:9012     (P2P WebSocket)

VPS firewall: only port 22 (SSH) needs to be open.
```

---

## Step 1 — Create new resource in Coolify

- **New Resource → Docker Compose**
- Source: GitHub repository
- Repo: `https://github.com/CanDenizGokgedik/decentralized-gpu-p2p-network`
- Branch: `main`
- Docker Compose file: `docker-compose.yml` (repo root)

---

## Step 2 — Set environment variables in Coolify UI

Copy from `.env.example` and fill in real values:

```env
DATABASE_URL=postgres://decentgpu:STRONG_PASS@postgres:5432/decentgpu
POSTGRES_PASSWORD=STRONG_PASS
JWT_SECRET=<openssl rand -hex 32>
ADMIN_EMAIL=admin@gpu.candeniz.me
ADMIN_PASSWORD=<strong password>
CLOUDFLARE_TUNNEL_TOKEN=eyJhIjoiNTU1...   # your full token from Zero Trust
NEXT_PUBLIC_API_URL=https://gpu.candeniz.me
NEXT_PUBLIC_WS_URL=wss://gpu.candeniz.me
BOOTSTRAP_PEER_ID=                          # leave empty on first deploy
```

---

## Step 3 — First deploy

Click **Deploy** in Coolify. First build takes ~5–10 minutes (Rust compile).

Watch logs — when you see `health endpoint listening` from bootstrap, move to Step 4.

---

## Step 4 — Get Bootstrap Peer ID

```bash
# On your VPS via SSH:
docker exec decentgpu-bootstrap \
  curl -s http://localhost:9001/health \
  | python3 -c "import sys,json; print(json.load(sys.stdin)['peer_id'])"
```

Or use Coolify's built-in terminal for the bootstrap container.

Copy the printed PeerID (starts with `12D3KooW...`).

---

## Step 5 — Update BOOTSTRAP_PEER_ID and redeploy

1. In Coolify → Environment Variables, set:
   ```
   BOOTSTRAP_PEER_ID=12D3KooW...   (the value from Step 4)
   ```
2. Click **Redeploy** — master will now connect to bootstrap at startup.

---

## Step 6 — Configure Cloudflare Tunnel Public Hostnames

In **Cloudflare Zero Trust → Networks → Tunnels → your tunnel → Configure → Public Hostname**:

Add **three** routes:

| Subdomain        | Domain       | Service                  | Notes                        |
|------------------|--------------|--------------------------|------------------------------|
| `gpu`            | `candeniz.me`| `http://nginx:80`        | Frontend + API               |
| `ws-bootstrap`   | `gpu.candeniz.me` | `http://bootstrap:9002` | P2P WebSocket for workers |
| `ws-master`      | `gpu.candeniz.me` | `http://master:9012`    | P2P WebSocket for workers |

> **Why separate hostnames for P2P?**
> libp2p's WebSocket transport always connects to path `/` — it cannot use
> paths like `/p2p/bootstrap`. Path-based nginx routing for P2P is not possible.
> Hostname-based Cloudflare routing solves this without changing libp2p internals.

---

## Step 7 — Verify deployment

```bash
# Health check
curl https://gpu.candeniz.me/health
# Expected: {"status":"ok","db":"connected",...}

# Frontend
open https://gpu.candeniz.me
```

---

## Step 8 — Connect a worker (production)

Workers connect via WebSocket so they work from any network:

```bash
export WORKER_BOOTSTRAP_ADDR="/dns4/ws-bootstrap.gpu.candeniz.me/tcp/443/wss"
export WORKER_MASTER_ADDR="/dns4/ws-master.gpu.candeniz.me/tcp/443/wss"
export WORKER_BOOTSTRAP_PEER_ID="12D3KooW..."   # from Step 4
export WORKER_AUTH_TOKEN="<token from UI dashboard>"
./decentgpu-worker
```

Or download the personalised setup script from the dashboard:
`https://gpu.candeniz.me` → Worker Panel → Download Setup Script

---

## Troubleshooting

| Symptom | Check |
|---------|-------|
| `master` won't start | `BOOTSTRAP_PEER_ID` set and bootstrap healthy? |
| Workers can't connect | ws-bootstrap / ws-master Cloudflare routes added? |
| Frontend shows 502 | Is master running? Check `docker logs decentgpu-master` |
| Jobs stuck in queue | Is a worker online? Check Worker Panel in UI |

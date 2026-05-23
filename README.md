# DecentGPU

> Decentralized GPU Rental Platform · Merkeziyetsiz GPU Kiralama Platformu

---


<div align="center">
  <img src="https://github.com/user-attachments/assets/b7eb4dfe-f433-4ea7-99ae-412de5b9e086" width="250" alt="TÜBİTAK Logo" />
  
  <p>
    Bu proje TÜBİTAK 2209-A Üniversite Öğrencileri Araştırma Projeleri Desteği kapsamında desteklenmektedir.<br>
    <em>This project is supported by TÜBİTAK 2209-A Undergraduate Research Projects Support Programme.</em>
  </p>
</div>

**🇬🇧 [English](#english)** · **🇹🇷 [Türkçe](#türkçe)**

---

## English

### Table of Contents
- [What is it?](#what-is-it)
- [How it Works](#how-it-works)
- [Tech Stack](#tech-stack)
- [Setup & Running](#setup--running)
- [Project Structure](#project-structure)
- [Compute Units (CU)](#compute-units-cu)
- [API Overview](#api-overview)

---


### What is it?

DecentGPU is a peer-to-peer decentralized platform that connects **workers** who want to rent out their GPUs with **hirers** who need GPU compute power.

- **Worker**: Executes Python jobs inside isolated Docker containers and earns Compute Units.
- **Hirer**: Uploads Python code, assigns jobs to available workers, and pays with CU.
- **Master**: Manages the scheduler, REST API, and database; maintains P2P connections with workers.
- **Bootstrap**: Acts as a rendezvous/relay server so workers and the master can discover each other behind NAT.

---

### How it Works

```
Hirer → [Web UI / API] → Master Node → Scheduler → Worker Node
                                ↕                        ↕
                          PostgreSQL              Docker Container
                                ↕                        ↕
                       P2P (libp2p)   ←────────   Results + Logs
```

1. The hirer uploads Python code and an optional `requirements.txt`.
2. The master stores the job as `pending` in PostgreSQL.
3. The scheduler picks a suitable worker based on GPU backend requirements.
4. The job is sent to the worker over an encrypted P2P connection.
5. The worker builds an isolated Docker image on-the-fly, runs the code, and streams logs back to the master in real time.
6. On completion, the result is stored, the hirer's CU balance is debited, and the worker is paid.

---

### Tech Stack

| Layer | Technology |
|-------|------------|
| Backend | Rust (Tokio, Axum, SQLx) |
| P2P Network | libp2p 0.54 (TCP, QUIC, WebSocket, GossipSub, Relay v2) |
| Database | PostgreSQL 16 |
| Serialization | Protobuf (Prost) + Serde |
| Auth | JWT + Argon2 |
| Containers | Bollard (Docker) |
| Frontend | Next.js 16, React 19, Tailwind CSS v4 |
| Real-time | xterm.js, Socket.IO, WebSocket |
| Deployment | Docker Compose, Nginx, Cloudflare Tunnel, Coolify |

---

### Setup & Running

#### Prerequisites

- Rust (stable)
- Node.js 20+
- Docker
- PostgreSQL 16

#### Development

```bash
# Clone the repo
git clone https://github.com/CanDenizGokgedik/decentralized-gpu-p2p-network
cd decentgpu

# Set up environment variables
cp .env.example .env

# Start PostgreSQL only
docker compose -f deploy/docker-compose.dev.yml up -d

# Start Bootstrap → Master → Frontend in order
./start.sh
```

#### Starting a Worker

```bash
./worker.sh
```

#### Production (Docker Compose)

```bash
docker compose -f docker-compose.yml up -d
```

---

### Project Structure

```
decentgpu/
├── crates/
│   ├── decentgpu-bootstrap/   # Rendezvous / relay node
│   ├── decentgpu-master/      # Coordinator node + REST API
│   │   ├── api/               # Auth, jobs, credits, admin, logs, terminal
│   │   ├── p2p/               # libp2p behaviours
│   │   ├── scheduler/         # Job scheduler & worker matching
│   │   ├── credits/           # CU ledger
│   │   └── db/                # SQLx queries + migrations
│   ├── decentgpu-worker/      # Worker node
│   │   ├── gpu/               # GPU detection (CUDA, Metal, ROCm)
│   │   ├── docker/            # Image build & management
│   │   └── executor/          # Job runner & log streaming
│   └── decentgpu-proto/       # Protobuf definitions
├── frontend/                  # Next.js app
│   └── src/
│       ├── app/               # Pages (auth, dashboard, jobs, admin)
│       ├── components/        # UI components
│       └── lib/               # API client, auth helpers
├── deploy/                    # Dockerfiles, Nginx, Coolify guide
├── binaries/                  # Pre-built binaries (macOS ARM64)
├── start.sh                   # Dev startup script
└── worker.sh                  # Worker startup script
```

---

### Compute Units (CU)

The platform's internal credit system. Job cost is calculated as:

```
Cost = Duration (hours) × 10 CU × GPU Factor
```

| GPU Backend | Factor |
|-------------|--------|
| CPU | 1× |
| Metal (Apple) | 3× |
| ROCm (AMD) | 4× |
| CUDA (NVIDIA) | 5× |

- Workers earn **85%** of the job cost.
- **15%** is retained as a platform fee.
- Users can request additional CUs; an admin approves or rejects them.

---

### API Overview

| Endpoint | Description |
|----------|-------------|
| `POST /api/auth/register` | Register a new account |
| `POST /api/auth/login` | Login (returns JWT) |
| `GET /api/jobs` | List jobs |
| `POST /api/jobs` | Create a new job |
| `GET /api/jobs/:id` | Job details |
| `GET /api/workers` | List available workers |
| `GET /api/credits/balance` | CU balance |
| `GET /api/admin/stats` | Admin statistics |

---

## Türkçe

### İçindekiler
- [Nedir?](#nedir)
- [Nasıl Çalışır?](#nasıl-çalışır)
- [Teknoloji Yığını](#teknoloji-yığını)
- [Kurulum ve Çalıştırma](#kurulum-ve-çalıştırma)
- [Proje Yapısı](#proje-yapısı)
- [Compute Units (CU)](#compute-units-cu-1)
- [API Özeti](#api-özeti)

---

### Nedir?

DecentGPU, GPU'larını kiraya vermek isteyen **worker**'lar ile GPU hesaplama gücüne ihtiyaç duyan **hirer**'ları bir araya getiren, eşten eşe (P2P) merkeziyetsiz bir platformdur.

- **Worker**: Python işlerini Docker container içinde çalıştırır, Compute Unit kazanır.
- **Hirer**: Python kodu yükler, uygun worker'a iş atar, CU harcayarak sonucu alır.
- **Master**: Zamanlayıcı, REST API ve veritabanını yönetir; worker'larla P2P bağlantı kurar.
- **Bootstrap**: Worker ve master'ların birbirini NAT arkasında bulabilmesi için rendezvous/relay sunucusu görevini üstlenir.

---

### Nasıl Çalışır?

```
Hirer → [Web UI / API] → Master Node → Scheduler → Worker Node
                                ↕                        ↕
                          PostgreSQL              Docker Container
                                ↕                        ↕
                       P2P (libp2p)   ←────────   İş Sonuçları + Loglar
```

1. Hirer, Python kodu ve isteğe bağlı `requirements.txt` yükler.
2. Master işi `pending` olarak kayıt eder.
3. Zamanlayıcı, GPU gereksinimlerine göre uygun bir worker seçer.
4. İş, şifreli P2P bağlantısı üzerinden worker'a gönderilir.
5. Worker, izole bir Docker image oluşturur, kodu çalıştırır; logları gerçek zamanlı olarak master'a iletir.
6. İş tamamlandığında sonuç kaydedilir, hirer'ın CU bakiyesi düşülür, worker'a ödeme yapılır.

---

### Teknoloji Yığını

| Katman | Teknoloji |
|--------|-----------|
| Backend | Rust (Tokio, Axum, SQLx) |
| P2P Ağ | libp2p 0.54 (TCP, QUIC, WebSocket, GossipSub, Relay v2) |
| Veritabanı | PostgreSQL 16 |
| Serialization | Protobuf (Prost) + Serde |
| Auth | JWT + Argon2 |
| Container | Bollard (Docker) |
| Frontend | Next.js 16, React 19, Tailwind CSS v4 |
| Gerçek Zamanlı | xterm.js, Socket.IO, WebSocket |
| Deployment | Docker Compose, Nginx, Cloudflare Tunnel, Coolify |

---

### Kurulum ve Çalıştırma

#### Gereksinimler

- Rust (stable)
- Node.js 20+
- Docker
- PostgreSQL 16

#### Geliştirme Ortamı

```bash
# Repoyu klonla
git clone https://github.com/CanDenizGokgedik/decentralized-gpu-p2p-network
cd decentgpu

# .env dosyasını oluştur
cp .env.example .env

# PostgreSQL'i başlat (sadece)
docker compose -f deploy/docker-compose.dev.yml up -d

# Bootstrap → Master → Frontend sırasıyla başlat
./start.sh
```

#### Worker Başlatma

```bash
./worker.sh
```

#### Production (Docker Compose)

```bash
docker compose -f docker-compose.yml up -d
```

---

### Proje Yapısı

```
decentgpu/
├── crates/
│   ├── decentgpu-bootstrap/   # Rendezvous / relay node
│   ├── decentgpu-master/      # Koordinatör node + REST API
│   │   ├── api/               # Auth, jobs, credits, admin, logs, terminal
│   │   ├── p2p/               # libp2p davranışları
│   │   ├── scheduler/         # İş zamanlayıcı ve worker eşleştirme
│   │   ├── credits/           # CU muhasebesi
│   │   └── db/                # SQLx sorguları + migration'lar
│   ├── decentgpu-worker/      # Worker node
│   │   ├── gpu/               # GPU algılama (CUDA, Metal, ROCm)
│   │   ├── docker/            # Image oluşturma ve yönetim
│   │   └── executor/          # İş çalıştırma ve log akışı
│   └── decentgpu-proto/       # Protobuf tanımları
├── frontend/                  # Next.js uygulaması
│   └── src/
│       ├── app/               # Sayfalar (auth, dashboard, jobs, admin)
│       ├── components/        # UI bileşenleri
│       └── lib/               # API client, auth yardımcıları
├── deploy/                    # Dockerfile'lar, Nginx, Coolify kılavuzu
├── binaries/                  # Hazır binary'ler (macOS ARM64)
├── start.sh                   # Dev başlatma betiği
└── worker.sh                  # Worker başlatma betiği
```

---

### Compute Units (CU)

Platformun iç kredi sistemi. İş maliyeti şu şekilde hesaplanır:

```
Maliyet = Süre (saat) × 10 CU × GPU Faktörü
```

| GPU Backend | Faktör |
|-------------|--------|
| CPU | 1× |
| Metal (Apple) | 3× |
| ROCm (AMD) | 4× |
| CUDA (NVIDIA) | 5× |

- Worker, iş maliyetinin **%85**'ini kazanır.
- **%15** platform komisyonu olarak kesilir.
- Kullanıcılar ek CU talep edebilir; admin onaylar/reddeder.

---

### API Özeti

| Endpoint | Açıklama |
|----------|----------|
| `POST /api/auth/register` | Kayıt |
| `POST /api/auth/login` | Giriş (JWT döner) |
| `GET /api/jobs` | İş listesi |
| `POST /api/jobs` | Yeni iş oluştur |
| `GET /api/jobs/:id` | İş detayı |
| `GET /api/workers` | Worker listesi |
| `GET /api/credits/balance` | CU bakiyesi |
| `GET /api/admin/stats` | Admin istatistikleri |

---

<p align="center">
  Built with Rust + Next.js · gpu.candeniz.me
</p>

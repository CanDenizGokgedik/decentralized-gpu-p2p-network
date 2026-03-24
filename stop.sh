#!/bin/bash
echo "DecentGPU durduruluyor..."

if [ -f /tmp/decentgpu.pids ]; then
  source /tmp/decentgpu.pids
  kill $BOOTSTRAP_PID $MASTER_PID $FRONTEND_PID 2>/dev/null || true
  rm -f /tmp/decentgpu.pids
fi

lsof -ti:9000 | xargs kill -9 2>/dev/null || true
lsof -ti:9001 | xargs kill -9 2>/dev/null || true
lsof -ti:8888 | xargs kill -9 2>/dev/null || true
lsof -ti:3000 | xargs kill -9 2>/dev/null || true
pkill -f "decentgpu-bootstrap\|decentgpu-master\|decentgpu-worker" 2>/dev/null || true

echo "✓ Tüm süreçler durduruldu."

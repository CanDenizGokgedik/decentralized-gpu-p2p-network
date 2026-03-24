'use client'

import { useState, useEffect } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useAuth, getToken } from '@/lib/auth'
import { workersApi, downloadsApi } from '@/lib/api'
import {
  Download,
  CheckCircle,
  Wifi,
  Apple,
  Monitor,
  Terminal,
  RefreshCw,
  ChevronRight,
  Cpu,
  Zap,
} from 'lucide-react'

type PlatformInfo = {
  id: string
  name: string
  arch: string
  icon: React.ElementType
  script: string
  binary: string
}

function detectPlatform(): PlatformInfo {
  if (typeof window === 'undefined') {
    return {
      id: 'linux-x86_64',
      name: 'Linux',
      arch: 'x86_64',
      icon: Terminal,
      script: 'decentgpu-setup.sh',
      binary: 'decentgpu-worker-linux-x86_64',
    }
  }
  const ua = navigator.userAgent.toLowerCase()
  const mac = ua.includes('mac')
  const win = ua.includes('win')
  if (mac) {
    return {
      id: 'macos-aarch64',
      name: 'macOS',
      arch: 'Apple Silicon',
      icon: Apple,
      script: 'decentgpu-setup.sh',
      binary: 'decentgpu-worker-macos-aarch64',
    }
  }
  if (win) {
    return {
      id: 'windows-x86_64',
      name: 'Windows',
      arch: 'x86_64',
      icon: Monitor,
      script: 'decentgpu-setup.bat',
      binary: 'decentgpu-worker-windows-x86_64.exe',
    }
  }
  return {
    id: 'linux-x86_64',
    name: 'Linux',
    arch: 'x86_64',
    icon: Terminal,
    script: 'decentgpu-setup.sh',
    binary: 'decentgpu-worker-linux-x86_64',
  }
}

function StepIndicator({
  number,
  title,
  done,
  active,
}: {
  number: number
  title: string
  done: boolean
  active: boolean
}) {
  return (
    <div className={`flex items-center gap-3 transition-all ${active ? 'opacity-100' : done ? 'opacity-70' : 'opacity-30'}`}>
      <div
        className={`w-8 h-8 rounded-full flex items-center justify-center text-sm font-bold shrink-0 transition-colors ${
          done ? 'bg-emerald-500 text-white' : active ? 'bg-indigo-500 text-white ring-4 ring-indigo-500/30' : 'bg-slate-800 text-slate-500'
        }`}
      >
        {done ? '✓' : number}
      </div>
      <span className={`text-sm font-medium ${active ? 'text-slate-100' : 'text-slate-400'}`}>{title}</span>
    </div>
  )
}

export default function WorkerClientPage() {
  const { user } = useAuth()
  const [platform, setPlatform] = useState<PlatformInfo | null>(null)
  const [step, setStep] = useState<0 | 1 | 2 | 3>(0)
  const [altPlatform, setAltPlatform] = useState(false)

  useEffect(() => {
    setPlatform(detectPlatform())
  }, [])

  const { data: workerStatus, refetch: refetchWorker } = useQuery({
    queryKey: ['my-worker-status'],
    queryFn: () => workersApi.me().then((r) => r.data).catch(() => null),
    refetchInterval: step === 2 ? 5000 : false,
    retry: false,
    enabled: !!user,
  })

  const { data: downloadInfo } = useQuery({
    queryKey: ['download-info'],
    queryFn: () => downloadsApi.info().then((r) => r.data),
  })

  useEffect(() => {
    if (workerStatus?.is_online && step === 2) {
      setStep(3)
    }
  }, [workerStatus, step])

  if (!platform) return null

  const platformInfo = downloadInfo?.platforms?.[platform.id]
  const isAvailable = platformInfo?.available ?? false
  const Icon = platform.icon

  const handleDownloadBinary = () => {
    window.location.href = `/api/downloads/worker/${platform.id}`
    setTimeout(() => setStep(1), 1000)
  }

  const handleDownloadScript = async () => {
    try {
      const token = getToken()
      if (!token) {
        alert('Oturum açmanız gerekiyor.')
        return
      }
      const response = await fetch(`/api/downloads/setup-script/${platform.id}`, {
        headers: {
          Authorization: `Bearer ${token}`,
        },
      })
      if (!response.ok) {
        const err = await response.json().catch(() => ({}))
        alert(`Hata: ${(err as { error?: string }).error ?? response.statusText}`)
        return
      }
      const disposition = response.headers.get('content-disposition') ?? ''
      const filenameMatch = disposition.match(/filename="?([^";\n]+)"?/)
      const filename =
        filenameMatch?.[1] ??
        (platform.id.includes('windows') ? 'decentgpu-setup.bat' : 'decentgpu-setup.sh')

      const blob = await response.blob()
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = filename
      document.body.appendChild(a)
      a.click()
      document.body.removeChild(a)
      URL.revokeObjectURL(url)
      setTimeout(() => setStep(2), 500)
    } catch (e) {
      console.error('Script download failed:', e)
      alert('Script indirilemedi. Lütfen tekrar deneyin.')
    }
  }

  const ALT_PLATFORMS = [
    { id: 'macos-aarch64', label: 'macOS (Apple Silicon)' },
    { id: 'macos-x86_64', label: 'macOS (Intel)' },
    { id: 'linux-x86_64', label: 'Linux (x86_64)' },
    { id: 'windows-x86_64', label: 'Windows' },
  ]

  return (
    <div className="max-w-2xl mx-auto">
      <div className="mb-8">
        <h1 className="text-2xl font-bold text-slate-100">Worker Ol</h1>
        <p className="text-slate-400 mt-1">GPU&apos;nunu platforma sun, model eğitimlerine katkı sağla</p>
      </div>

      {/* Already connected banner */}
      {workerStatus?.is_online && (
        <div className="bg-emerald-500/10 border border-emerald-500/30 rounded-2xl p-5 mb-6 flex items-center gap-4">
          <div className="w-12 h-12 bg-emerald-500/20 rounded-full flex items-center justify-center shrink-0">
            <Wifi className="w-6 h-6 text-emerald-400" />
          </div>
          <div className="flex-1">
            <p className="font-semibold text-emerald-300">Worker olarak bağlısınız!</p>
            <p className="text-sm text-emerald-400/80 mt-0.5">Sisteme dahilsiniz ve iş almaya hazırsınız.</p>
          </div>
          <div className="text-right">
            <p className="text-xs text-emerald-500/70 font-mono">{workerStatus.peer_id?.slice(0, 16)}...</p>
          </div>
        </div>
      )}

      {/* Step tracker */}
      <div className="bg-slate-900 border border-slate-800 rounded-2xl p-6 mb-6">
        <div className="flex flex-col sm:flex-row gap-4 sm:items-center sm:justify-between">
          <StepIndicator number={1} title="Programı İndir" done={step >= 1} active={step === 0} />
          <ChevronRight className="w-4 h-4 text-slate-700 hidden sm:block shrink-0" />
          <StepIndicator number={2} title="Scripti Çalıştır" done={step >= 2} active={step === 1} />
          <ChevronRight className="w-4 h-4 text-slate-700 hidden sm:block shrink-0" />
          <StepIndicator number={3} title="Bağlantı Bekleniyor" done={step >= 3} active={step === 2} />
          <ChevronRight className="w-4 h-4 text-slate-700 hidden sm:block shrink-0" />
          <StepIndicator number={4} title="Hazır!" done={step >= 3} active={step === 3} />
        </div>
      </div>

      {/* STEP 0 — Download binary */}
      {step === 0 && (
        <div className="bg-slate-900 border border-slate-800 rounded-2xl overflow-hidden">
          <div className="p-6 border-b border-slate-800">
            <div className="flex items-center gap-3 mb-1">
              <Icon className="w-5 h-5 text-slate-300" />
              <h2 className="font-semibold text-slate-100">Adım 1 — Worker Programını İndir</h2>
            </div>
            <p className="text-sm text-slate-400 ml-8">
              Sisteminiz için uygun program otomatik seçildi:{' '}
              <span className="text-indigo-400 font-medium">
                {platform.name} {platform.arch}
              </span>
            </p>
          </div>
          <div className="p-6">
            <div className="bg-slate-800/50 rounded-xl p-4 mb-6 border border-slate-700/50">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <div className="w-10 h-10 bg-indigo-500/20 rounded-lg flex items-center justify-center">
                    <Icon className="w-5 h-5 text-indigo-400" />
                  </div>
                  <div>
                    <p className="font-mono text-sm text-slate-200">{platform.binary}</p>
                    <p className="text-xs text-slate-500 mt-0.5">
                      {isAvailable ? `${platformInfo?.size_mb?.toFixed(1) ?? '~'} MB` : 'Hazırlanıyor...'}
                    </p>
                  </div>
                </div>
                {isAvailable ? (
                  <CheckCircle className="w-5 h-5 text-emerald-400" />
                ) : (
                  <div className="w-5 h-5 border-2 border-slate-600 rounded-full animate-spin border-t-indigo-400" />
                )}
              </div>
            </div>

            <button
              onClick={handleDownloadBinary}
              disabled={!isAvailable}
              className="w-full py-3.5 bg-indigo-600 hover:bg-indigo-500 disabled:bg-slate-700 disabled:text-slate-500 text-white rounded-xl font-semibold transition-all flex items-center justify-center gap-2 shadow-lg shadow-indigo-500/20"
            >
              <Download className="w-5 h-5" />
              {isAvailable ? 'Worker Programını İndir' : 'Hazırlanıyor...'}
            </button>

            <button onClick={() => setStep(1)} className="w-full mt-3 py-2.5 text-slate-400 hover:text-slate-200 text-sm transition-colors">
              Zaten indirdim →
            </button>

            <div className="mt-6 pt-4 border-t border-slate-800">
              <button onClick={() => setAltPlatform((v) => !v)} className="text-xs text-slate-500 hover:text-slate-400 transition-colors">
                Farklı platform için indir ↓
              </button>
              {altPlatform && (
                <div className="mt-3 grid grid-cols-2 gap-2">
                  {ALT_PLATFORMS.map((p) => {
                    const info = downloadInfo?.platforms?.[p.id]
                    return (
                      <a
                        key={p.id}
                        href={info?.available ? `/api/downloads/worker/${p.id}` : undefined}
                        className={`text-xs px-3 py-2 rounded-lg border text-center transition-colors ${
                          info?.available
                            ? 'border-slate-700 text-slate-300 hover:border-indigo-500 hover:text-indigo-400'
                            : 'border-slate-800 text-slate-600 cursor-not-allowed'
                        }`}
                      >
                        {p.label}
                        {!info?.available && ' (yakında)'}
                      </a>
                    )
                  })}
                </div>
              )}
            </div>
          </div>
        </div>
      )}

      {/* STEP 1 — Download and run setup script */}
      {step === 1 && (
        <div className="bg-slate-900 border border-slate-800 rounded-2xl overflow-hidden">
          <div className="p-6 border-b border-slate-800">
            <h2 className="font-semibold text-slate-100">Adım 2 — Kurulum Scriptini İndir ve Çalıştır</h2>
            <p className="text-sm text-slate-400 mt-1">
              Bu script hesabınıza özel olarak hazırlanmıştır. Tek tıkla her şeyi otomatik yapar.
            </p>
          </div>
          <div className="p-6">
            <div className="space-y-2 mb-6">
              {[
                'macOS güvenlik engelini otomatik kaldırır',
                'Gerekli izinleri otomatik ayarlar',
                'Hesabınızla otomatik ilişkilendirir',
                'Worker programını başlatır',
              ].map((item) => (
                <div key={item} className="flex items-center gap-2 text-sm text-slate-300">
                  <CheckCircle className="w-4 h-4 text-emerald-400 shrink-0" />
                  {item}
                </div>
              ))}
            </div>

            <button
              onClick={handleDownloadScript}
              className="w-full py-3.5 bg-emerald-600 hover:bg-emerald-500 text-white rounded-xl font-semibold transition-all flex items-center justify-center gap-2 shadow-lg shadow-emerald-500/20"
            >
              <Download className="w-5 h-5" />
              Kurulum Scriptini İndir
            </button>

            <div className="mt-6 bg-slate-800/50 rounded-xl p-4 border border-slate-700/50">
              <p className="text-sm font-medium text-slate-300 mb-3">Script nasıl çalıştırılır?</p>
              {platform.id.includes('windows') ? (
                <div className="space-y-3 text-sm text-slate-400">
                  <p>1. İndirilen <code className="text-indigo-300 bg-slate-900 px-1 rounded">decentgpu-setup.bat</code> ve <code className="text-indigo-300 bg-slate-900 px-1 rounded">{platform.binary}</code> dosyalarını aynı klasöre koyun</p>
                  <p>2. <code className="text-indigo-300 bg-slate-900 px-1 rounded">decentgpu-setup.bat</code> dosyasına çift tıklayın</p>
                </div>
              ) : (
                <div className="space-y-3">
                  <p className="text-sm text-slate-400">1. İndirilen <code className="text-indigo-300 bg-slate-900 px-1 rounded">decentgpu-setup.sh</code> ve <code className="text-indigo-300 bg-slate-900 px-1 rounded">{platform.binary}</code> dosyalarını aynı klasöre koyun</p>
                  <p className="text-sm text-slate-400">2. Terminal açın ve çalıştırın:</p>
                  <div className="bg-slate-950 rounded-lg p-3 font-mono text-xs text-emerald-300 border border-slate-700">
                    cd ~/Downloads && bash decentgpu-setup.sh
                  </div>
                </div>
              )}
            </div>

            <button onClick={() => setStep(2)} className="w-full mt-4 py-2.5 text-slate-400 hover:text-slate-200 text-sm transition-colors">
              Scripti çalıştırdım, bağlantı bekliyorum →
            </button>
          </div>
        </div>
      )}

      {/* STEP 2 — Waiting for connection */}
      {step === 2 && (
        <div className="bg-slate-900 border border-slate-800 rounded-2xl p-8 text-center">
          <div className="w-16 h-16 bg-indigo-500/10 rounded-full flex items-center justify-center mx-auto mb-4">
            <Wifi className="w-8 h-8 text-indigo-400 animate-pulse" />
          </div>
          <h2 className="text-lg font-semibold text-slate-100 mb-2">Bağlantı Bekleniyor...</h2>
          <p className="text-sm text-slate-400 mb-6">
            Worker programı çalışıyor mu? Terminalde loglar akıyorsa birkaç saniye içinde bağlantı kurulacak.
          </p>
          <div className="flex items-center justify-center gap-2 text-xs text-slate-500 mb-6">
            <RefreshCw className="w-3 h-3 animate-spin" />
            Her 5 saniyede kontrol ediliyor...
          </div>
          <button
            onClick={() => refetchWorker()}
            className="px-6 py-2.5 bg-slate-800 hover:bg-slate-700 text-slate-300 rounded-lg text-sm transition-colors flex items-center gap-2 mx-auto"
          >
            <RefreshCw className="w-4 h-4" />
            Şimdi Kontrol Et
          </button>
          <button onClick={() => setStep(1)} className="block mx-auto mt-3 text-xs text-slate-600 hover:text-slate-400 transition-colors">
            ← Geri dön
          </button>
        </div>
      )}

      {/* STEP 3 — Success */}
      {step === 3 && (
        <div className="bg-slate-900 border border-emerald-500/30 rounded-2xl overflow-hidden">
          <div className="bg-emerald-500/10 p-6 border-b border-emerald-500/20 text-center">
            <div className="text-4xl mb-2">🎉</div>
            <h2 className="text-lg font-bold text-emerald-300">Worker olarak bağlandınız!</h2>
            <p className="text-sm text-emerald-400/80 mt-1">Sisteme dahilsiniz ve iş almaya hazırsınız.</p>
          </div>
          <div className="p-6">
            {workerStatus && (
              <>
                <div className="grid grid-cols-3 gap-4 mb-6">
                  <div className="bg-slate-800/50 rounded-xl p-3 text-center">
                    <Cpu className="w-5 h-5 text-indigo-400 mx-auto mb-1" />
                    <p className="text-xs text-slate-500">GPU</p>
                    <p className="text-sm font-semibold text-slate-200 mt-0.5">
                      {(workerStatus.capabilities?.gpus?.length ?? 0) > 0
                        ? (workerStatus.capabilities.gpus[0]?.name ?? 'Algılandı')
                        : 'CPU Modu'}
                    </p>
                  </div>
                  <div className="bg-slate-800/50 rounded-xl p-3 text-center">
                    <Zap className="w-5 h-5 text-amber-400 mx-auto mb-1" />
                    <p className="text-xs text-slate-500">İş</p>
                    <p className="text-sm font-semibold text-slate-200 mt-0.5">{workerStatus.jobs_completed ?? 0}</p>
                  </div>
                  <div className="bg-slate-800/50 rounded-xl p-3 text-center">
                    <CheckCircle className="w-5 h-5 text-emerald-400 mx-auto mb-1" />
                    <p className="text-xs text-slate-500">Durum</p>
                    <p className="text-sm font-semibold text-emerald-400 mt-0.5">Aktif</p>
                  </div>
                </div>
                <div className="bg-slate-800/30 rounded-xl p-4 border border-slate-700/50">
                  <p className="text-xs text-slate-500 font-mono mb-1">Peer ID</p>
                  <p className="text-xs font-mono text-slate-400 break-all">{workerStatus.peer_id}</p>
                </div>
              </>
            )}
            <p className="text-center text-sm text-slate-400 mt-4">
              Terminali açık tutun. Program kapandığında worker olarak görünmezsiniz.
            </p>
          </div>
        </div>
      )}

      {/* Requirements */}
      <div className="mt-6 bg-slate-900/50 border border-slate-800 rounded-xl p-5">
        <h3 className="text-sm font-medium text-slate-400 mb-3">Sistem Gereksinimleri</h3>
        <div className="grid grid-cols-2 gap-2 text-xs">
          {[
            { req: 'Docker 20.10+', note: 'GPU için nvidia-docker' },
            { req: 'RAM: min 4 GB', note: 'Önerilen: 8 GB+' },
            { req: 'Disk: min 20 GB', note: 'Docker imajları için' },
            { req: 'macOS / Linux', note: 'Windows desteği yakında' },
          ].map((r) => (
            <div key={r.req} className="flex gap-2">
              <CheckCircle className="w-3.5 h-3.5 text-slate-600 shrink-0 mt-0.5" />
              <div>
                <p className="text-slate-300">{r.req}</p>
                <p className="text-slate-600">{r.note}</p>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}

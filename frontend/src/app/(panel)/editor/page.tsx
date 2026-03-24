'use client'

import { useState, useRef } from 'react'
import dynamic from 'next/dynamic'
import Link from 'next/link'
import { useQuery } from '@tanstack/react-query'
import { useAuth, getToken } from '@/lib/auth'
import {
  Play, Square, Download, Settings,
  Cpu, Clock, ChevronRight,
} from 'lucide-react'

const MonacoEditor = dynamic(
  () => import('@monaco-editor/react'),
  {
    ssr: false,
    loading: () => (
      <div className="flex-1 bg-slate-950 flex items-center justify-center">
        <div className="animate-spin w-6 h-6 border-2 border-indigo-500 border-t-transparent rounded-full" />
      </div>
    ),
  }
)

const DEFAULT_CODE = `# DecentGPU — Python Editörü
# Kodunuzu yazın ve "Çalıştır" butonuna tıklayın

import time

print("Merhaba, DecentGPU!", flush=True)

for i in range(1, 6):
    print(f"Adım {i}/5 tamamlandı", flush=True)
    time.sleep(1)

print("İş tamamlandı! 🎉", flush=True)
`

type RunStatus = 'idle' | 'submitting' | 'running' | 'completed' | 'failed'

export default function EditorPage() {
  const { user } = useAuth()
  const [code, setCode] = useState(DEFAULT_CODE)
  const [requirements, setRequirements] = useState('')
  const [output, setOutput] = useState<string[]>([])
  const [status, setStatus] = useState<RunStatus>('idle')
  const [jobId, setJobId] = useState<string | null>(null)
  const [showSettings, setShowSettings] = useState(false)
  const [downloading, setDownloading] = useState(false)
  const [settings, setSettings] = useState({
    gpu_backend: 'cpu_only',
    memory_limit_mb: 512,
    max_duration_secs: 300,
  })
  const wsRef = useRef<WebSocket | null>(null)
  const outputRef = useRef<HTMLDivElement>(null)
  const statusRef = useRef<RunStatus>('idle')

  // File management
  const [files, setFiles] = useState<Record<string, string>>({
    'main.py': DEFAULT_CODE,
  })
  const [activeFile, setActiveFile] = useState('main.py')

  const handleCodeChange = (val: string | undefined) => {
    const newCode = val ?? ''
    setCode(newCode)
    setFiles(prev => ({ ...prev, [activeFile]: newCode }))
  }

  const switchFile = (filename: string) => {
    setFiles(prev => ({ ...prev, [activeFile]: code }))
    setActiveFile(filename)
    setCode(files[filename] ?? '')
  }

  const addFile = () => {
    const name = prompt('Dosya adı (.py uzantılı):')
    if (!name) return
    const filename = name.endsWith('.py') ? name : `${name}.py`
    if (files[filename] !== undefined) {
      alert('Bu isimde dosya zaten var')
      return
    }
    setFiles(prev => ({ ...prev, [filename]: '# Yeni dosya\n' }))
    switchFile(filename)
  }

  const deleteFile = (filename: string) => {
    if (Object.keys(files).length <= 1) {
      alert('En az bir dosya olmalı')
      return
    }
    if (!confirm(`"${filename}" silinsin mi?`)) return
    const newFiles = { ...files }
    delete newFiles[filename]
    const newActive = Object.keys(newFiles)[0]
    setFiles(newFiles)
    setActiveFile(newActive)
    setCode(newFiles[newActive])
  }

  // Workers query — API returns a raw array (not {workers:[...]})
  const { data: workerCount = 0 } = useQuery({
    queryKey: ['editor-workers'],
    queryFn: async () => {
      const token = getToken()
      if (!token) return 0
      const apiBase = process.env.NEXT_PUBLIC_API_URL ?? 'http://localhost:8888'
      const res = await fetch(`${apiBase}/api/workers`, {
        headers: { Authorization: `Bearer ${token}` },
      })
      if (!res.ok) return 0
      const data = await res.json()
      // API returns a raw array of worker objects
      const list: Array<{ is_online?: boolean }> = Array.isArray(data)
        ? data
        : (data.workers ?? [])
      return list.filter(w => w.is_online).length
    },
    refetchInterval: 8_000,
  })

  const addOutput = (line: string) => {
    setOutput(prev => [...prev, line])
    setTimeout(() => {
      if (outputRef.current) {
        outputRef.current.scrollTop = outputRef.current.scrollHeight
      }
    }, 30)
  }

  const handleRun = async () => {
    if (status === 'submitting' || status === 'running') return
    if (workerCount === 0) {
      addOutput('❌ Bağlı worker yok. Lütfen bir worker başlatın.')
      return
    }
    setOutput([])
    setJobId(null)
    statusRef.current = 'submitting'
    setStatus('submitting')
    addOutput('▶ İş hazırlanıyor...')
    const token = getToken()
    if (!token) {
      addOutput('❌ Oturum bulunamadı. Lütfen yeniden giriş yapın.')
      setStatus('idle')
      return
    }
    try {
      const form = new FormData()
      const codeBlob = new Blob([code], { type: 'text/plain' })
      const codeFile = new File([codeBlob], activeFile)
      form.append('code', codeFile)
      const reqContent = requirements.trim() ? requirements : '# no additional requirements\n'
      const reqBlob = new Blob([reqContent], { type: 'text/plain' })
      const reqFile = new File([reqBlob], 'requirements.txt')
      form.append('requirements', reqFile)
      form.append('gpu_backend',        settings.gpu_backend)
      form.append('memory_limit_mb',    String(settings.memory_limit_mb))
      form.append('max_duration_secs',  String(settings.max_duration_secs))
      addOutput(`📦 Dosya: ${activeFile} (${code.length} karakter)`)
      addOutput(`⚙️  Backend: ${settings.gpu_backend} | Bellek: ${settings.memory_limit_mb}MB`)
      const apiBase = process.env.NEXT_PUBLIC_API_URL ?? 'http://localhost:8888'
      const res = await fetch(`${apiBase}/api/jobs`, {
        method:  'POST',
        headers: { Authorization: `Bearer ${token}` },
        body:    form,
      })
      const data = await res.json()
      if (!res.ok) {
        addOutput(`❌ Gönderim hatası (${res.status}): ${data.error ?? JSON.stringify(data)}`)
        setStatus('failed')
        return
      }
      const newJobId = data.job_id
      if (!newJobId) {
        addOutput(`❌ Sunucudan job_id alınamadı: ${JSON.stringify(data)}`)
        setStatus('failed')
        return
      }
      setJobId(newJobId)
      statusRef.current = 'running'
      setStatus('running')
      addOutput(`✓ İş oluşturuldu: ${newJobId.slice(0, 8)}...`)
      addOutput('⏳ Docker imajı hazırlanıyor (~10-30 saniye)...')
      const wsBase = apiBase.replace('http://', 'ws://').replace('https://', 'wss://')
      const wsUrl = `${wsBase}/api/jobs/${newJobId}/terminal?token=${token}`
      addOutput('🔌 Terminal bağlanıyor...')
      const pollInterval = setInterval(async () => {
        try {
          const statusRes = await fetch(`${apiBase}/api/jobs/${newJobId}`, {
            headers: { Authorization: `Bearer ${token}` }
          })
          const jobData = await statusRes.json()
          const s = jobData.status ?? ''
          if (s === 'assigned') {
            addOutput('✓ Worker atandı, çalışmaya başlıyor...')
          } else if (s === 'completed' || s === 'failed' || s === 'cancelled') {
            clearInterval(pollInterval)
          }
        } catch {}
      }, 3000)
      const ws = new WebSocket(wsUrl)
      wsRef.current = ws
      ws.onopen = () => addOutput('● Terminal bağlandı')
      ws.onmessage = (event) => {
        try {
          const msg = JSON.parse(event.data)
          switch (msg.type) {
            case 'connected':
              addOutput('─────────────────────────')
              break
            case 'log': {
              const line = (msg.data ?? msg.message ?? '')
                .replace(/\r\n$/, '').replace(/\n$/, '').replace(/\r$/, '')
              if (line) addOutput(line)
              break
            }
            case 'replay_complete':
              if (msg.count > 0)
                addOutput(`─── ${msg.count} satır geçmiş log yüklendi ───`)
              break
            case 'job_done': {
              clearInterval(pollInterval)
              const ok = msg.status === 'completed'
              addOutput('─────────────────────────')
              addOutput(ok ? '✓ İş başarıyla tamamlandı!' : `✗ İş başarısız oldu (${msg.status})`)
              statusRef.current = ok ? 'completed' : 'failed'
              setStatus(ok ? 'completed' : 'failed')
              ws.close()
              break
            }
            case 'ping': break
            default:
              if (event.data && !event.data.includes('"type"')) addOutput(event.data)
          }
        } catch {
          if (event.data) addOutput(event.data)
        }
      }
      ws.onerror = (e) => {
        addOutput('⚠ WebSocket bağlantı hatası')
        console.error('WS error:', e)
      }
      ws.onclose = () => {
        clearInterval(pollInterval)
        if (statusRef.current === 'running') {
          statusRef.current = 'completed'
          setStatus('completed')
        }
      }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err)
      addOutput(`❌ Beklenmeyen hata: ${msg}`)
      setStatus('failed')
      console.error('Editor submit error:', err)
    }
  }

  const handleStop = () => {
    wsRef.current?.close()
    statusRef.current = 'idle'
    setStatus('idle')
    addOutput('■ Durduruldu')
  }

  const handleDownload = async () => {
    if (!jobId) return
    const token = getToken()
    try {
      setDownloading(true)
      const res = await fetch(`/api/jobs/${jobId}/result`, {
        headers: { Authorization: `Bearer ${token}` },
      })
      if (!res.ok) {
        alert('Sonuç bulunamadı')
        return
      }
      const disposition = res.headers.get('content-disposition') ?? ''
      const match = disposition.match(/filename="([^"]+)"/)
      const filename = match?.[1] ?? `result-${jobId.slice(0, 8)}.tar.gz`
      const blob = await res.blob()
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = filename
      document.body.appendChild(a)
      a.click()
      document.body.removeChild(a)
      URL.revokeObjectURL(url)
    } catch {
      alert('İndirme başarısız')
    } finally {
      setDownloading(false)
    }
  }

  const statusColor: Record<RunStatus, string> = {
    idle: 'text-slate-400',
    submitting: 'text-amber-400',
    running: 'text-indigo-400',
    completed: 'text-emerald-400',
    failed: 'text-red-400',
  }

  const statusLabel: Record<RunStatus, string> = {
    idle: 'Hazır',
    submitting: 'Gönderiliyor...',
    running: 'Çalışıyor',
    completed: 'Tamamlandı',
    failed: 'Başarısız',
  }

  if (!user) return null

  return (
    <div className="flex flex-col h-[calc(100vh-4rem)] -m-6">
      {/* Toolbar */}
      <div className="flex items-center gap-3 px-4 py-2.5 bg-slate-900 border-b border-slate-800 shrink-0">
        {/* macOS-style dots */}
        <div className="flex items-center gap-1.5 mr-2">
          <div className="w-3 h-3 rounded-full bg-red-500" />
          <div className="w-3 h-3 rounded-full bg-amber-500" />
          <div className="w-3 h-3 rounded-full bg-emerald-500" />
          <span className="ml-2 text-sm font-medium text-slate-300">{activeFile}</span>
        </div>

        <div className="flex-1" />

        {/* Worker status */}
        <div className={`flex items-center gap-1.5 text-xs ${workerCount > 0 ? 'text-emerald-400' : 'text-red-400'}`}>
          <span className={`w-1.5 h-1.5 rounded-full ${workerCount > 0 ? 'bg-emerald-400 animate-pulse' : 'bg-red-400'}`} />
          {workerCount > 0 ? `${workerCount} worker hazır` : 'Worker bulunamadı'}
        </div>

        {/* Status */}
        <div className={`text-xs font-medium ${statusColor[status]} flex items-center gap-1.5`}>
          {status === 'running' && (
            <div className="w-2 h-2 rounded-full bg-indigo-400 animate-pulse" />
          )}
          {statusLabel[status]}
        </div>

        {/* Settings */}
        <button
          onClick={() => setShowSettings(v => !v)}
          className="p-1.5 hover:bg-slate-700 rounded text-slate-400 hover:text-slate-200 transition-colors"
        >
          <Settings className="w-4 h-4" />
        </button>

        {/* Run / Stop */}
        {status === 'running' ? (
          <button
            onClick={handleStop}
            className="flex items-center gap-2 px-4 py-1.5 bg-red-600 hover:bg-red-500 text-white rounded-lg text-sm font-medium transition-colors"
          >
            <Square className="w-3.5 h-3.5" />
            Durdur
          </button>
        ) : (
          <button
            onClick={handleRun}
            disabled={status === 'submitting' || workerCount === 0}
            className="flex items-center gap-2 px-4 py-1.5 bg-indigo-600 hover:bg-indigo-500 disabled:bg-slate-700 disabled:text-slate-500 text-white rounded-lg text-sm font-medium transition-colors"
          >
            <Play className="w-3.5 h-3.5" />
            Çalıştır
          </button>
        )}

        {/* Download (when completed) */}
        {status === 'completed' && jobId && (
          <button
            onClick={handleDownload}
            disabled={downloading}
            className="flex items-center gap-2 px-4 py-1.5 bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white rounded-lg text-sm font-medium transition-colors"
          >
            <Download className="w-3.5 h-3.5" />
            {downloading ? 'İndiriliyor...' : 'İndir'}
          </button>
        )}
      </div>

      {/* Settings panel */}
      {showSettings && (
        <div className="bg-slate-800/80 border-b border-slate-700 px-4 py-3 flex items-center gap-6 shrink-0 flex-wrap">
          <div className="flex items-center gap-2">
            <Cpu className="w-4 h-4 text-slate-400" />
            <label className="text-xs text-slate-400">Backend:</label>
            <select
              value={settings.gpu_backend}
              onChange={e => setSettings(s => ({ ...s, gpu_backend: e.target.value }))}
              className="bg-slate-700 border border-slate-600 rounded px-2 py-1 text-xs text-slate-200 focus:outline-none focus:ring-1 focus:ring-indigo-500"
            >
              <option value="cpu_only">CPU</option>
              <option value="cuda">NVIDIA CUDA</option>
              <option value="metal">Apple Metal</option>
              <option value="rocm">AMD ROCm</option>
            </select>
          </div>

          <div className="flex items-center gap-2">
            <Clock className="w-4 h-4 text-slate-400" />
            <label className="text-xs text-slate-400">Bellek:</label>
            <select
              value={settings.memory_limit_mb}
              onChange={e => setSettings(s => ({ ...s, memory_limit_mb: Number(e.target.value) }))}
              className="bg-slate-700 border border-slate-600 rounded px-2 py-1 text-xs text-slate-200 focus:outline-none focus:ring-1 focus:ring-indigo-500"
            >
              <option value={256}>256 MB</option>
              <option value={512}>512 MB</option>
              <option value={1024}>1 GB</option>
              <option value={2048}>2 GB</option>
            </select>
          </div>

          <div className="flex items-center gap-2">
            <Clock className="w-4 h-4 text-slate-400" />
            <label className="text-xs text-slate-400">Max Süre:</label>
            <select
              value={settings.max_duration_secs}
              onChange={e => setSettings(s => ({ ...s, max_duration_secs: Number(e.target.value) }))}
              className="bg-slate-700 border border-slate-600 rounded px-2 py-1 text-xs text-slate-200 focus:outline-none focus:ring-1 focus:ring-indigo-500"
            >
              <option value={60}>1 dk</option>
              <option value={300}>5 dk</option>
              <option value={600}>10 dk</option>
              <option value={1800}>30 dk</option>
              <option value={3600}>1 saat</option>
            </select>
          </div>

          <div className="flex items-center gap-2 flex-1 min-w-[200px]">
            <label className="text-xs text-slate-400 whitespace-nowrap">requirements.txt:</label>
            <input
              type="text"
              value={requirements}
              onChange={e => setRequirements(e.target.value)}
              placeholder="numpy pandas torch ..."
              className="flex-1 bg-slate-700 border border-slate-600 rounded px-2 py-1 text-xs text-slate-200 placeholder:text-slate-500 focus:outline-none focus:ring-1 focus:ring-indigo-500"
            />
          </div>
        </div>
      )}

      {/* File tabs */}
      <div className="flex items-center bg-slate-900 border-b border-slate-800 overflow-x-auto shrink-0">
        <div className="flex items-center min-w-0">
          {Object.keys(files).map(filename => (
            <div
              key={filename}
              className={`flex items-center gap-2 px-4 py-2 text-sm border-r border-slate-800 cursor-pointer select-none group whitespace-nowrap transition-colors
                ${activeFile === filename
                  ? 'bg-slate-950 text-slate-100 border-t-2 border-t-indigo-500'
                  : 'text-slate-400 hover:text-slate-200 hover:bg-slate-800'
                }`}
              onClick={() => switchFile(filename)}
            >
              <span className="text-xs">🐍</span>
              <span>{filename}</span>
              {Object.keys(files).length > 1 && (
                <button
                  onClick={(e) => { e.stopPropagation(); deleteFile(filename) }}
                  className="opacity-0 group-hover:opacity-100 text-slate-500 hover:text-red-400 transition-all ml-1 text-xs"
                >
                  ✕
                </button>
              )}
            </div>
          ))}
        </div>
        <button
          onClick={addFile}
          className="px-3 py-2 text-slate-500 hover:text-slate-200 hover:bg-slate-800 transition-colors text-sm whitespace-nowrap"
          title="Yeni dosya ekle"
        >
          + Dosya
        </button>
      </div>

      {/* Main area */}
      <div className="flex flex-1 min-h-0">
        {/* Monaco Editor */}
        <div className="flex-1 min-w-0 border-r border-slate-800">
          <MonacoEditor
            height="100%"
            language="python"
            theme="vs-dark"
            value={code}
            onChange={handleCodeChange}
            options={{
              fontSize: 14,
              fontFamily: "'JetBrains Mono', 'Fira Code', 'Consolas', monospace",
              lineNumbers: 'on',
              minimap: { enabled: false },
              scrollBeyondLastLine: false,
              automaticLayout: true,
              tabSize: 4,
              insertSpaces: true,
              wordWrap: 'on',
              renderLineHighlight: 'all',
              cursorBlinking: 'smooth',
              smoothScrolling: true,
              padding: { top: 16, bottom: 16 },
            }}
          />
        </div>

        {/* Output panel */}
        <div className="w-[400px] flex flex-col bg-slate-950 shrink-0">
          <div className="flex items-center justify-between px-4 py-2 border-b border-slate-800 shrink-0">
            <span className="text-xs font-medium text-slate-400 uppercase tracking-wider">Çıktı</span>
            <div className="flex items-center gap-2">
              <span className="text-xs text-slate-600">{output.length} satır</span>
              <button
                onClick={() => setOutput([])}
                className="text-xs text-slate-600 hover:text-slate-400 transition-colors"
              >
                Temizle
              </button>
            </div>
          </div>

          <div
            ref={outputRef}
            className="flex-1 overflow-y-auto p-4 font-mono text-xs leading-relaxed"
          >
            {output.length === 0 ? (
              <div className="flex flex-col items-center justify-center h-full text-slate-700 text-center">
                <Play className="w-8 h-8 mb-3 opacity-30" />
                <p className="text-sm">Çıktı burada görünecek</p>
                <p className="text-xs mt-1 opacity-70">
                  Kodu yazıp &quot;Çalıştır&quot;a tıklayın
                </p>
              </div>
            ) : (
              <div className="space-y-0.5">
                {output.map((line, i) => {
                  const isError = line.startsWith('❌') || line.startsWith('✗')
                  const isSuccess = line.startsWith('✓') || line.includes('tamamlandı')
                  const isMeta = line.startsWith('▶') || line.startsWith('●') ||
                    line.startsWith('⏳') || line.startsWith('─') ||
                    line.startsWith('■') || line.startsWith('⚠')
                  return (
                    <div
                      key={i}
                      className={
                        isError ? 'text-red-400' :
                        isSuccess ? 'text-emerald-400' :
                        isMeta ? 'text-slate-500' :
                        'text-slate-300'
                      }
                    >
                      {line || '\u00A0'}
                    </div>
                  )
                })}
                {status === 'running' && (
                  <div className="flex items-center gap-2 text-indigo-400 mt-1">
                    <div className="w-1.5 h-1.5 rounded-full bg-indigo-400 animate-pulse" />
                    <span>Çalışıyor...</span>
                  </div>
                )}
              </div>
            )}
          </div>

          {jobId && (
            <div className="px-4 py-2.5 border-t border-slate-800 shrink-0">
              <Link
                href={`/jobs/${jobId}`}
                className="text-xs text-indigo-400 hover:text-indigo-300 transition-colors flex items-center gap-1"
                target="_blank"
              >
                İş detayını görüntüle
                <ChevronRight className="w-3 h-3" />
              </Link>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

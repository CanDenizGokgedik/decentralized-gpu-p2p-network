'use client'

import { useState, useCallback } from 'react'
import { useRouter } from 'next/navigation'
import { useQuery } from '@tanstack/react-query'
import { Upload, FileCode, FileText, Check, AlertCircle, ChevronRight, ChevronLeft } from 'lucide-react'
import { computeUnitsApi, jobsApi } from '@/lib/api'
import { Button } from '@/components/ui/Button'
import { Card } from '@/components/ui/Card'
import { useToast } from '@/components/ui/Toast'
import { useAuth } from '@/lib/auth'
import { cn, estimateCU, formatCU, formatMB } from '@/lib/utils'
import { BACKEND_OPTIONS } from '@/lib/constants'

const STEPS = ['Kod Yükleme', 'Kaynak Seçimi', 'Maliyet Özeti']

function DropZone({ label, accept, onFile, file }: {
  label: string
  accept: string
  onFile: (f: File) => void
  file: File | null
}) {
  const [drag, setDrag] = useState(false)

  const onDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault(); setDrag(false)
    const f = e.dataTransfer.files[0]
    if (f) onFile(f)
  }, [onFile])

  return (
    <div
      onDragOver={e => { e.preventDefault(); setDrag(true) }}
      onDragLeave={() => setDrag(false)}
      onDrop={onDrop}
      className={cn(
        'relative rounded-xl border-2 border-dashed p-8 text-center transition-colors cursor-pointer',
        drag ? 'border-indigo-400 bg-indigo-950/30' : 'border-slate-600 hover:border-slate-500 bg-slate-800/50',
        file && 'border-emerald-500/50 bg-emerald-950/20'
      )}
      onClick={() => {
        const input = document.createElement('input')
        input.type = 'file'; input.accept = accept
        input.onchange = () => { if (input.files?.[0]) onFile(input.files[0]) }
        input.click()
      }}
    >
      {file ? (
        <div className="flex items-center justify-center gap-3">
          <Check className="h-5 w-5 text-emerald-400" />
          <span className="text-sm font-medium text-emerald-300">{file.name}</span>
          <span className="text-xs text-slate-500">({(file.size / 1024).toFixed(1)} KB)</span>
        </div>
      ) : (
        <>
          <Upload className="mx-auto mb-3 h-8 w-8 text-slate-500" />
          <p className="text-sm text-slate-400">{label}</p>
          <p className="mt-1 text-xs text-slate-600">Sürükleyin veya tıklayın</p>
        </>
      )}
    </div>
  )
}

export default function YeniIsPage() {
  const router      = useRouter()
  const { user }    = useAuth()
  const { toast }   = useToast()
  const [step, setStep] = useState(0)
  const [codeFile, setCodeFile]   = useState<File | null>(null)
  const [reqFile,  setReqFile]    = useState<File | null>(null)
  const [backend,  setBackend]    = useState('cpu_only')
  const [memMb,    setMemMb]      = useState(2048)
  const [durSecs,  setDurSecs]    = useState(3600)
  const [submitting, setSubmitting] = useState(false)

  const { data: balance } = useQuery({
    queryKey: ['balance'],
    queryFn: () => computeUnitsApi.balance().then(r => r.data),
    enabled: !!user,
  })

  const estimatedCU = estimateCU(backend, durSecs)
  const available   = balance?.cu_available ?? 0
  const sufficient  = available >= estimatedCU

  const canProceed = [
    !!codeFile,
    !!backend,
    true,
  ][step]

  const handleSubmit = async () => {
    if (!codeFile) return
    setSubmitting(true)
    try {
      const form = new FormData()
      form.append('code',        codeFile)
      form.append('gpu_backend', backend)
      form.append('memory_limit_mb',   String(memMb))
      form.append('max_duration_secs', String(durSecs))
      if (reqFile) form.append('requirements', reqFile)

      const res = await jobsApi.create(form)
      toast('İş başarıyla gönderildi!', 'success')
      router.push(`/jobs/${res.data.job_id}`)
    } catch (err: unknown) {
      const msg = (err as { response?: { data?: { error?: string } } })?.response?.data?.error
      toast(msg ?? 'İş gönderilemedi.', 'error')
      setSubmitting(false)
    }
  }

  return (
    <div className="max-w-2xl mx-auto space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-slate-100">Yeni İş Oluştur</h1>
        <p className="text-slate-400 mt-1">Python kodunuzu GPU'da çalıştırın</p>
      </div>

      {/* Stepper */}
      <div className="flex items-center gap-2">
        {STEPS.map((s, i) => (
          <div key={s} className="flex items-center gap-2 flex-1">
            <div className={cn(
              'flex h-7 w-7 shrink-0 items-center justify-center rounded-full text-xs font-bold',
              i < step  ? 'bg-indigo-600 text-white' :
              i === step ? 'bg-indigo-600/30 border-2 border-indigo-500 text-indigo-300' :
                           'bg-slate-800 text-slate-500'
            )}>
              {i < step ? <Check className="h-4 w-4" /> : i + 1}
            </div>
            <span className={cn('text-sm hidden sm:block', i === step ? 'text-slate-200 font-medium' : 'text-slate-500')}>
              {s}
            </span>
            {i < STEPS.length - 1 && <div className="flex-1 h-px bg-slate-700" />}
          </div>
        ))}
      </div>

      <Card>
        {/* Step 0: Code Upload */}
        {step === 0 && (
          <div className="space-y-6">
            <div>
              <div className="flex items-center gap-2 mb-3">
                <FileCode className="h-5 w-5 text-indigo-400" />
                <h2 className="font-semibold text-slate-100">Python Kodunuz</h2>
                <span className="text-xs text-red-400">*Zorunlu</span>
              </div>
              <DropZone label='Python kodunuzu (.py) buraya sürükleyin veya seçin' accept=".py" onFile={setCodeFile} file={codeFile} />
              {codeFile && (
                <div className="mt-2 rounded-lg bg-slate-900 border border-slate-700 p-3 font-mono text-xs text-slate-400 max-h-40 overflow-y-auto">
                  {/* Preview placeholder */}
                  <p className="text-slate-500 italic">Dosya yüklendi: {codeFile.name}</p>
                </div>
              )}
            </div>

            <div>
              <div className="flex items-center gap-2 mb-3">
                <FileText className="h-5 w-5 text-slate-400" />
                <h2 className="font-semibold text-slate-100">Bağımlılıklar</h2>
                <span className="text-xs text-slate-500">Opsiyonel</span>
              </div>
              <DropZone label="requirements.txt dosyanızı buraya sürükleyin (opsiyonel)" accept=".txt" onFile={setReqFile} file={reqFile} />
            </div>
          </div>
        )}

        {/* Step 1: Resource Selection */}
        {step === 1 && (
          <div className="space-y-6">
            <div>
              <h2 className="font-semibold text-slate-100 mb-4">GPU Backend Seçin</h2>
              <div className="grid gap-3 sm:grid-cols-2">
                {BACKEND_OPTIONS.map(opt => (
                  <label
                    key={opt.value}
                    className={cn(
                      'flex cursor-pointer flex-col gap-1 rounded-xl border p-4 transition-all',
                      backend === opt.value
                        ? 'border-indigo-500 bg-indigo-950/40'
                        : 'border-slate-700 bg-slate-800 hover:border-slate-600'
                    )}
                  >
                    <input
                      type="radio"
                      className="sr-only"
                      value={opt.value}
                      checked={backend === opt.value}
                      onChange={() => setBackend(opt.value)}
                    />
                    <div className="flex items-center justify-between">
                      <span className="font-semibold text-slate-100">{opt.label}</span>
                      <span className="text-xs text-indigo-300 font-medium">{opt.rate} CU/saat</span>
                    </div>
                    <span className="text-xs text-slate-400">{opt.description}</span>
                  </label>
                ))}
              </div>
            </div>

            <div>
              <div className="flex justify-between mb-2">
                <label className="text-sm font-medium text-slate-300">Bellek Limiti</label>
                <span className="text-sm font-semibold text-indigo-300">{formatMB(memMb)}</span>
              </div>
              <input
                type="range" min={256} max={32768} step={256} value={memMb}
                onChange={e => setMemMb(+e.target.value)}
                className="w-full accent-indigo-500"
              />
              <div className="flex justify-between text-xs text-slate-600 mt-1">
                <span>256 MB</span><span>32 GB</span>
              </div>
            </div>

            <div>
              <div className="flex justify-between mb-2">
                <label className="text-sm font-medium text-slate-300">Maksimum Süre</label>
                <span className="text-sm font-semibold text-indigo-300">
                  {durSecs < 3600 ? `${durSecs / 60} dk` : `${(durSecs / 3600).toFixed(1)} saat`}
                </span>
              </div>
              <input
                type="range" min={60} max={86400} step={60} value={durSecs}
                onChange={e => setDurSecs(+e.target.value)}
                className="w-full accent-indigo-500"
              />
              <div className="flex justify-between text-xs text-slate-600 mt-1">
                <span>1 dk</span><span>24 saat</span>
              </div>
            </div>
          </div>
        )}

        {/* Step 2: Cost Summary */}
        {step === 2 && (
          <div className="space-y-4">
            <h2 className="font-semibold text-slate-100">Maliyet Özeti</h2>

            <div className="rounded-xl border border-slate-700 bg-slate-900 divide-y divide-slate-700">
              <div className="flex justify-between px-4 py-3">
                <span className="text-slate-400">Tahmini Maliyet</span>
                <span className="font-bold text-indigo-300">{formatCU(estimatedCU)}</span>
              </div>
              <div className="flex justify-between px-4 py-3">
                <span className="text-slate-400">Mevcut Bakiyeniz</span>
                <span className="font-medium text-slate-200">{formatCU(available)}</span>
              </div>
              <div className="flex justify-between px-4 py-3">
                <span className="text-slate-400">İşlem Sonrası</span>
                <span className={cn('font-bold', sufficient ? 'text-emerald-400' : 'text-red-400')}>
                  {formatCU(available - estimatedCU)}
                </span>
              </div>
            </div>

            {!sufficient && (
              <div className="flex items-start gap-3 rounded-xl border border-amber-700 bg-amber-950/30 p-4">
                <AlertCircle className="h-5 w-5 shrink-0 text-amber-400 mt-0.5" />
                <p className="text-sm text-amber-300">
                  Yetersiz CU bakiyesi. Yöneticiden CU talep etmek için{' '}
                  <a href="/compute-units" className="underline font-medium">Compute Units</a>{' '}
                  sayfasını ziyaret edin.
                </p>
              </div>
            )}
          </div>
        )}
      </Card>

      {/* Navigation */}
      <div className="flex justify-between">
        <Button
          variant="secondary"
          onClick={() => setStep(s => s - 1)}
          disabled={step === 0}
        >
          <ChevronLeft className="h-4 w-4" />
          Geri
        </Button>

        {step < STEPS.length - 1 ? (
          <Button onClick={() => setStep(s => s + 1)} disabled={!canProceed}>
            İleri <ChevronRight className="h-4 w-4" />
          </Button>
        ) : (
          <Button
            onClick={handleSubmit}
            loading={submitting}
            disabled={!sufficient || submitting}
          >
            İşi Gönder
          </Button>
        )}
      </div>
    </div>
  )
}

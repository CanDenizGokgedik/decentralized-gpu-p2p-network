'use client'
import Link from 'next/link'
import { useEffect, useRef, useState } from 'react'
import {
  Cpu, Shield, Zap, Globe, ArrowRight,
  Download, CheckCircle, ChevronDown,
  Lock, Terminal,
  Code, Server, Wifi
} from 'lucide-react'

function AnimatedNumber({
  target, suffix = '', duration = 2000
}: {
  target: number; suffix?: string; duration?: number
}) {
  const [value, setValue] = useState(0)
  const [started, setStarted] = useState(false)
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const observer = new IntersectionObserver(
      ([entry]) => { if (entry.isIntersecting && !started) setStarted(true) },
      { threshold: 0.5 }
    )
    if (ref.current) observer.observe(ref.current)
    return () => observer.disconnect()
  }, [started])
  useEffect(() => {
    if (!started) return
    const steps = 60
    const step = target / steps
    const delay = duration / steps
    let current = 0
    const timer = setInterval(() => {
      current += step
      if (current >= target) { setValue(target); clearInterval(timer) }
      else setValue(Math.floor(current))
    }, delay)
    return () => clearInterval(timer)
  }, [started, target, duration])
  return <div ref={ref}>{value}{suffix}</div>
}

function Particle({ delay, x, y }: { delay: number; x: number; y: number }) {
  return (
    <div
      className="absolute w-1 h-1 rounded-full bg-indigo-400/30 animate-ping pointer-events-none"
      style={{ left: `${x}%`, top: `${y}%`, animationDelay: `${delay}s`, animationDuration: '3s' }}
    />
  )
}

const CODE_LINES = [
  { text: '# DecentGPU ile model eğit', color: 'text-slate-500' },
  { text: 'import torch', color: 'text-purple-400' },
  { text: 'import torch.nn as nn', color: 'text-purple-400' },
  { text: '', color: '' },
  { text: 'model = nn.Sequential(', color: 'text-slate-300' },
  { text: '    nn.Linear(784, 256),', color: 'text-slate-400' },
  { text: '    nn.ReLU(),', color: 'text-green-400' },
  { text: '    nn.Linear(256, 10)', color: 'text-slate-400' },
  { text: ')', color: 'text-slate-300' },
  { text: '', color: '' },
  { text: "# GPU'da eğit →", color: 'text-slate-500' },
  { text: 'model.train()', color: 'text-indigo-400' },
]

function AnimatedCodeBlock() {
  const [visibleLines, setVisibleLines] = useState(0)
  useEffect(() => {
    if (visibleLines >= CODE_LINES.length) return
    const timer = setTimeout(() => setVisibleLines(v => v + 1), 120)
    return () => clearTimeout(timer)
  }, [visibleLines])
  return (
    <div className="bg-slate-900/80 backdrop-blur border border-slate-700/50 rounded-2xl overflow-hidden shadow-2xl shadow-indigo-500/10">
      <div className="flex items-center gap-2 px-4 py-3 border-b border-slate-800 bg-slate-900">
        <div className="w-3 h-3 rounded-full bg-red-500/70" />
        <div className="w-3 h-3 rounded-full bg-amber-500/70" />
        <div className="w-3 h-3 rounded-full bg-emerald-500/70" />
        <span className="ml-2 text-xs text-slate-500 font-mono">model_train.py</span>
        <div className="flex-1" />
        <span className="text-xs text-emerald-400 flex items-center gap-1">
          <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse inline-block" />
          Çalışıyor
        </span>
      </div>
      <div className="p-5 font-mono text-sm leading-7 min-h-[280px]">
        {CODE_LINES.slice(0, visibleLines).map((line, i) => (
          <div key={i} className={line.color || 'text-slate-300'}>
            <span className="select-none text-slate-700 mr-4 text-xs">{String(i + 1).padStart(2, ' ')}</span>
            {line.text || '\u00A0'}
          </div>
        ))}
        {visibleLines < CODE_LINES.length && (
          <div className="flex items-center">
            <span className="select-none text-slate-700 mr-4 text-xs">{String(visibleLines + 1).padStart(2, ' ')}</span>
            <span className="w-2 h-4 bg-indigo-400 animate-pulse inline-block rounded-sm" />
          </div>
        )}
      </div>
      <div className="border-t border-slate-800 bg-slate-950/50 px-5 py-3">
        <div className="flex items-center gap-2 text-xs">
          <Terminal className="w-3.5 h-3.5 text-slate-500" />
          <span className="text-emerald-400 font-mono">Epoch 1/10 — loss: 0.4231 — acc: 0.8712</span>
        </div>
      </div>
    </div>
  )
}

function StepCard({ step, icon: Icon, title, desc, color }: {
  step: string; icon: React.ElementType; title: string; desc: string; color: string
}) {
  return (
    <div className="group relative bg-slate-900/60 backdrop-blur border border-slate-800 rounded-2xl p-6 hover:border-indigo-500/40 hover:bg-slate-900/80 transition-all duration-500 hover:-translate-y-1">
      <div className="absolute top-4 right-4 text-4xl font-black text-slate-800 group-hover:text-slate-700 transition-colors select-none">{step}</div>
      <div className={`w-12 h-12 rounded-xl flex items-center justify-center mb-4 ${color}`}>
        <Icon className="w-6 h-6" />
      </div>
      <h3 className="font-bold text-slate-100 mb-2 text-lg">{title}</h3>
      <p className="text-sm text-slate-400 leading-relaxed">{desc}</p>
    </div>
  )
}

function FeatureCard({ icon: Icon, title, desc, color, badge }: {
  icon: React.ElementType; title: string; desc: string; color: string; badge?: string
}) {
  return (
    <div className="group bg-slate-900/40 border border-slate-800/80 rounded-xl p-5 hover:border-slate-700 hover:bg-slate-900/70 transition-all duration-300">
      <div className="flex items-start gap-4">
        <div className={`w-10 h-10 rounded-lg flex items-center justify-center shrink-0 ${color}`}>
          <Icon className="w-5 h-5" />
        </div>
        <div className="min-w-0">
          <div className="flex items-center gap-2 mb-1">
            <h3 className="font-semibold text-slate-200 text-sm">{title}</h3>
            {badge && (
              <span className="text-[10px] bg-indigo-500/20 text-indigo-400 border border-indigo-500/20 px-1.5 py-0.5 rounded-full font-medium">{badge}</span>
            )}
          </div>
          <p className="text-xs text-slate-500 leading-relaxed">{desc}</p>
        </div>
      </div>
    </div>
  )
}

export default function LandingPage() {
  const PARTICLES = Array.from({ length: 20 }, (_, i) => ({
    id: i,
    delay: (i * 0.2) % 4,
    x: (i * 17 + 5) % 100,
    y: (i * 13 + 10) % 100,
  }))

  return (
    <div className="min-h-screen bg-slate-950 text-slate-100 overflow-x-hidden">
      {/* Navbar */}
      <nav className="fixed top-0 inset-x-0 z-50 border-b border-slate-800/60 bg-slate-950/70 backdrop-blur-xl">
        <div className="max-w-6xl mx-auto px-6 h-16 flex items-center justify-between">
          <div className="flex items-center gap-2.5">
            <div className="w-8 h-8 bg-gradient-to-br from-indigo-500 to-purple-600 rounded-lg flex items-center justify-center shadow-lg shadow-indigo-500/30">
              <Cpu className="w-4 h-4 text-white" />
            </div>
            <span className="font-bold text-lg tracking-tight">DecentGPU</span>
            <span className="hidden sm:block text-[10px] bg-indigo-500/20 text-indigo-400 border border-indigo-500/30 px-2 py-0.5 rounded-full font-medium ml-1">TÜBİTAK 2209-A</span>
          </div>
          <div className="flex items-center gap-2">
            <Link href="/login" className="text-sm text-slate-400 hover:text-slate-200 transition-colors px-4 py-2 rounded-lg hover:bg-slate-800">Giriş Yap</Link>
            <Link href="/register" className="text-sm bg-indigo-600 hover:bg-indigo-500 text-white px-4 py-2 rounded-lg transition-all font-medium shadow-lg shadow-indigo-500/20">Ücretsiz Başla →</Link>
          </div>
        </div>
      </nav>

      {/* Hero */}
      <section className="relative min-h-screen flex items-center justify-center overflow-hidden pt-16">
        <div className="absolute inset-0 pointer-events-none">
          <div className="absolute top-1/4 left-1/4 w-96 h-96 bg-indigo-600/8 rounded-full blur-3xl" />
          <div className="absolute bottom-1/4 right-1/4 w-80 h-80 bg-purple-600/8 rounded-full blur-3xl" />
          <div className="absolute inset-0 opacity-[0.015]" style={{
            backgroundImage: 'linear-gradient(rgba(99,102,241,0.5) 1px, transparent 1px), linear-gradient(90deg, rgba(99,102,241,0.5) 1px, transparent 1px)',
            backgroundSize: '60px 60px'
          }} />
          {PARTICLES.map(p => <Particle key={p.id} delay={p.delay} x={p.x} y={p.y} />)}
        </div>
        <div className="relative max-w-7xl mx-auto px-6 py-20 grid grid-cols-1 lg:grid-cols-2 gap-16 items-center">
          <div>
            <div className="inline-flex items-center gap-2 bg-gradient-to-r from-indigo-500/10 to-purple-500/10 border border-indigo-500/20 rounded-full px-4 py-2 mb-8">
              <span className="w-2 h-2 bg-emerald-400 rounded-full animate-pulse" />
              <span className="text-xs font-medium text-indigo-300">Eşler Arası GPU Kaynak Paylaşım Platformu</span>
            </div>
            <h1 className="text-5xl sm:text-6xl font-black tracking-tight leading-[1.1] mb-6">
              Dağıtık GPU Ağı<br />
              <span className="relative">
                <span className="bg-gradient-to-r from-indigo-400 via-purple-400 to-indigo-300 bg-clip-text text-transparent">ile Model Eğit</span>
                <span className="absolute -bottom-1 left-0 right-0 h-px bg-gradient-to-r from-indigo-500/0 via-indigo-500/60 to-purple-500/0" />
              </span>
            </h1>
            <p className="text-lg text-slate-400 leading-relaxed mb-8 max-w-xl">
              Merkezi sunuculara bağımlı kalmadan makine öğrenmesi modellerinizi eğitin. Bireylerin GPU ve CPU güçlerini doğrudan birbirleriyle paylaştığı, eşler arası (P2P) ve açık kaynaklı bir hesaplama ağı.
            </p>
            <div className="flex flex-col sm:flex-row gap-4 mb-12">
              <Link href="/register" className="group flex items-center justify-center gap-2 bg-gradient-to-r from-indigo-600 to-purple-600 hover:from-indigo-500 hover:to-purple-500 text-white px-8 py-4 rounded-xl font-bold text-base transition-all shadow-xl shadow-indigo-500/30 hover:shadow-indigo-500/50 hover:-translate-y-0.5">
                Hemen Başla
                <ArrowRight className="w-5 h-5 group-hover:translate-x-1 transition-transform" />
              </Link>
              <a href="#nasil-calisir" className="flex items-center justify-center gap-2 bg-slate-800/80 hover:bg-slate-700/80 border border-slate-700 text-slate-200 px-8 py-4 rounded-xl font-semibold text-base transition-all backdrop-blur">
                Nasıl Çalışır?
                <ChevronDown className="w-5 h-5" />
              </a>
            </div>
            <div className="flex items-center gap-6 text-sm text-slate-500">
              <div className="flex items-center gap-2"><CheckCircle className="w-4 h-4 text-emerald-500" />Blok zinciri gerektirmez</div>
              <div className="flex items-center gap-2"><CheckCircle className="w-4 h-4 text-emerald-500" />Docker izolasyonu</div>
              <div className="flex items-center gap-2"><CheckCircle className="w-4 h-4 text-emerald-500" />Açık kaynak</div>
            </div>
          </div>
          <div className="relative">
            <div className="absolute inset-0 bg-indigo-500/5 rounded-3xl blur-2xl scale-110" />
            <AnimatedCodeBlock />
            <div className="absolute -left-6 top-1/4 bg-slate-900/90 backdrop-blur border border-slate-700/60 rounded-xl px-4 py-3 shadow-xl hidden lg:block">
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 bg-emerald-500/20 rounded-lg flex items-center justify-center">
                  <Globe className="w-4 h-4 text-emerald-400" />
                </div>
                <div>
                  <p className="text-xs text-slate-500">Merkezi Değil</p>
                  <p className="font-bold text-emerald-400 text-sm">P2P Ağ</p>
                </div>
              </div>
            </div>
            <div className="absolute -right-4 bottom-1/4 bg-slate-900/90 backdrop-blur border border-slate-700/60 rounded-xl px-4 py-3 shadow-xl hidden lg:block">
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 bg-indigo-500/20 rounded-lg flex items-center justify-center">
                  <Wifi className="w-4 h-4 text-indigo-400" />
                </div>
                <div>
                  <p className="text-xs text-slate-500">P2P Transfer</p>
                  <p className="font-bold text-indigo-400 text-sm">Direkt bağlantı</p>
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* Stats bar */}
      <section className="border-y border-slate-800/60 bg-slate-900/30 backdrop-blur py-12">
        <div className="max-w-5xl mx-auto px-6 grid grid-cols-2 sm:grid-cols-4 gap-8">
          {/* Static P2P stat */}
          <div className="text-center">
            <div className="text-4xl font-black text-slate-100 mb-1">P2P</div>
            <p className="font-semibold text-slate-300 text-sm">Merkezi Değil</p>
            <p className="text-xs text-slate-600 mt-0.5">Her düğüm eşit</p>
          </div>
          {/* Animated stats */}
          {[
            { value: 100, suffix: '%',  label: 'Docker Güvenliği', sub: 'Konteyner izolasyonu' },
            { value: 4,   suffix: '+',  label: 'Platform Desteği', sub: 'CUDA, ROCm, Metal, CPU' },
            { value: 0,   suffix: '',   label: 'Blok Zinciri Yok', sub: 'Saf P2P mimari' },
          ].map((s, i) => (
            <div key={i} className="text-center">
              <div className="text-4xl font-black text-slate-100 mb-1">
                <AnimatedNumber target={s.value} suffix={s.suffix} />
              </div>
              <p className="font-semibold text-slate-300 text-sm">{s.label}</p>
              <p className="text-xs text-slate-600 mt-0.5">{s.sub}</p>
            </div>
          ))}
        </div>
      </section>

      {/* How it works */}
      <section id="nasil-calisir" className="py-28 px-6">
        <div className="max-w-6xl mx-auto">
          <div className="text-center mb-16">
            <p className="text-indigo-400 text-sm font-semibold uppercase tracking-widest mb-3">SÜREÇ</p>
            <h2 className="text-4xl font-black mb-4">3 Adımda Başlayın</h2>
            <p className="text-slate-400 max-w-xl mx-auto">Kayıt olmaktan model eğitimine kadar tüm süreç dakikalar içinde tamamlanır.</p>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
            <StepCard step="01" icon={Code} title="Kodu Yükleyin" desc="Python kodunuzu ve bağımlılıklarınızı tarayıcı üzerindeki Monaco editörüyle yazın ya da yükleyin. Sistem otomatik Docker imajı oluşturur." color="bg-indigo-500/10 text-indigo-400" />
            <StepCard step="02" icon={Server} title="Worker Seçin" desc="Aktif GPU/CPU düğümleri arasından kesintisizlik skoru, bellek ve backend türüne göre filtreleyerek en uygun sistemi kiralayın." color="bg-purple-500/10 text-purple-400" />
            <StepCard step="03" icon={Download} title="Sonucu Alın" desc="Eğitim ilerleyişini canlı terminal üzerinden izleyin. Tamamlandığında model dosyalarınız güvenle indirilebilir hale gelir." color="bg-emerald-500/10 text-emerald-400" />
          </div>
        </div>
      </section>

      {/* Features */}
      <section className="py-28 px-6 bg-slate-900/20">
        <div className="max-w-6xl mx-auto">
          <div className="text-center mb-16">
            <p className="text-indigo-400 text-sm font-semibold uppercase tracking-widest mb-3">ÖZELLİKLER</p>
            <h2 className="text-4xl font-black mb-4">Neden DecentGPU?</h2>
            <p className="text-slate-400 max-w-xl mx-auto">Geleneksel bulut hizmetlerinin sunmadığı özellikler.</p>
          </div>
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
            <FeatureCard icon={Shield} title="Docker İzolasyonu" desc="Her iş ayrı bir konteynerda çalışır. Kodunuz worker sistemine hiçbir zaman erişemez. İş bitince her şey otomatik silinir." color="bg-emerald-500/10 text-emerald-400" badge="Güvenli" />
            <FeatureCard icon={Globe} title="P2P Veri Transferi" desc="Docker imajları merkezi sunucu yerine doğrudan worker'a aktarılır. libp2p ve NAT traversal ile maksimum verimlilik." color="bg-blue-500/10 text-blue-400" badge="Rust + libp2p" />
            <FeatureCard icon={Terminal} title="Canlı Terminal" desc="Model eğitimi devam ederken terminal çıktısını tarayıcınızda anlık izleyin. WebSocket ile sıfır gecikme." color="bg-indigo-500/10 text-indigo-400" />
            <FeatureCard icon={Cpu} title="Geniş Donanım Desteği" desc="NVIDIA CUDA, AMD ROCm, Apple Metal ve CPU modu. GPU'suz sistemler bile platforma işçi olarak katılabilir." color="bg-purple-500/10 text-purple-400" badge="4 Platform" />
            <FeatureCard icon={Code} title="Entegre Kod Editörü" desc="Monaco editörüyle (VS Code motoru) tarayıcıda Python yazın, anında gönderin. Çoklu dosya desteğiyle organize çalışın." color="bg-amber-500/10 text-amber-400" badge="Yeni" />
            <FeatureCard icon={Lock} title="Blok Zinciri Yok" desc="Kripto para veya blok zinciri gerektirmez. Compute Unit sistemi, kaynak kullanımını ölçen basit ve şeffaf bir birimdir." color="bg-slate-500/10 text-slate-400" />
          </div>
        </div>
      </section>

      {/* CU section */}
      <section className="py-28 px-6">
        <div className="max-w-5xl mx-auto">
          <div className="relative bg-gradient-to-br from-slate-900 to-slate-900/50 border border-indigo-500/20 rounded-3xl p-10 sm:p-14 overflow-hidden">
            <div className="absolute top-0 right-0 w-64 h-64 bg-indigo-600/5 rounded-full blur-3xl pointer-events-none" />
            <div className="absolute bottom-0 left-0 w-48 h-48 bg-purple-600/5 rounded-full blur-3xl pointer-events-none" />
            <div className="relative grid grid-cols-1 md:grid-cols-2 gap-10 items-center">
              <div>
                <div className="inline-flex items-center gap-2 bg-indigo-500/10 border border-indigo-500/20 rounded-full px-3 py-1.5 mb-5">
                  <Zap className="w-3.5 h-3.5 text-indigo-400" />
                  <span className="text-xs text-indigo-300 font-medium">Compute Unit (CU)</span>
                </div>
                <h2 className="text-3xl font-black mb-4">Compute Unit Nedir?</h2>
                <p className="text-slate-400 leading-relaxed mb-4">
                  CU, DecentGPU&apos;nun dahili kaynak birimidir. Gerçek para veya kripto para <strong className="text-slate-300">değildir</strong>. Platform içinde hesaplama kaynaklarına erişimi temsil eden sayısal bir birimdir.
                </p>
                <p className="text-sm text-slate-500">Araştırmacılar CU&apos;yu sistem yöneticisinden talep eder. Satın alma işlemi yapılmaz.</p>
              </div>
              <div className="space-y-3">
                {[
                  { mode: 'CPU (Temel)',  rate: 1, pct: 20,  color: 'bg-slate-500' },
                  { mode: 'Apple Metal', rate: 3, pct: 60,  color: 'bg-purple-500' },
                  { mode: 'AMD ROCm',    rate: 4, pct: 80,  color: 'bg-amber-500' },
                  { mode: 'NVIDIA CUDA', rate: 5, pct: 100, color: 'bg-indigo-500' },
                ].map(r => (
                  <div key={r.mode} className="flex items-center gap-4 bg-slate-800/40 rounded-xl px-4 py-3">
                    <span className="text-sm text-slate-300 w-32 shrink-0">{r.mode}</span>
                    <div className="flex-1 bg-slate-700/50 rounded-full h-2">
                      <div className={`h-full rounded-full ${r.color}`} style={{ width: `${r.pct}%` }} />
                    </div>
                    <span className="text-sm font-bold text-slate-200 w-20 text-right shrink-0">{r.rate} CU/saat</span>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* Download section */}
      <section className="py-28 px-6 bg-slate-900/20">
        <div className="max-w-4xl mx-auto text-center">
          <p className="text-indigo-400 text-sm font-semibold uppercase tracking-widest mb-3">WORKER OL</p>
          <h2 className="text-4xl font-black mb-4">GPU&apos;nunu Platforma Sun</h2>
          <p className="text-slate-400 mb-10 max-w-xl mx-auto">Kullanılmayan GPU&apos;nla araştırmacılara katkı sağla, Compute Unit kazan.</p>
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-4 mb-10">
            {[
              { os: 'Linux',   arch: 'x86_64',       icon: '🐧', platform: 'linux-x86_64'   },
              { os: 'macOS',   arch: 'Apple Silicon', icon: '🍎', platform: 'macos-aarch64'  },
              { os: 'macOS',   arch: 'Intel',         icon: '🍎', platform: 'macos-x86_64'   },
              { os: 'Windows', arch: 'x86_64',        icon: '🪟', platform: 'windows-x86_64' },
            ].map(p => (
              <a key={p.platform} href={`/api/downloads/worker/${p.platform}`} className="group bg-slate-900 border border-slate-800 hover:border-indigo-500/50 rounded-xl p-4 transition-all hover:-translate-y-0.5 text-center block">
                <div className="text-3xl mb-2">{p.icon}</div>
                <p className="font-semibold text-slate-200 text-sm">{p.os}</p>
                <p className="text-xs text-slate-500 mt-0.5">{p.arch}</p>
                <div className="mt-3 flex items-center justify-center gap-1 text-xs text-indigo-400 group-hover:text-indigo-300">
                  <Download className="w-3 h-3" />İndir
                </div>
              </a>
            ))}
          </div>
          <Link href="/register?role=worker" className="inline-flex items-center gap-2 text-sm text-slate-400 hover:text-slate-200 transition-colors">
            Hesap oluştur ve kurulum sihirbazını başlat →
          </Link>
        </div>
      </section>

      {/* Final CTA */}
      <section className="py-28 px-6">
        <div className="max-w-3xl mx-auto text-center">
          <div className="relative">
            <div className="absolute inset-0 bg-gradient-to-r from-indigo-600/10 to-purple-600/10 rounded-3xl blur-2xl" />
            <div className="relative bg-slate-900/60 border border-slate-800 rounded-3xl p-14 backdrop-blur">
              <h2 className="text-4xl font-black mb-4">Hemen Başlayın</h2>
              <p className="text-slate-400 mb-8 max-w-md mx-auto">GPU&apos;nuzla katkıda bulunun veya uygun maliyetle model eğitimlerinizi gerçekleştirin.</p>
              <div className="flex flex-col sm:flex-row gap-4 justify-center">
                <Link href="/register?role=hirer" className="flex items-center justify-center gap-2 bg-gradient-to-r from-indigo-600 to-purple-600 hover:from-indigo-500 hover:to-purple-500 text-white px-8 py-4 rounded-xl font-bold transition-all shadow-xl shadow-indigo-500/25">
                  <Cpu className="w-5 h-5" />GPU Kirala
                </Link>
                <Link href="/register?role=worker" className="flex items-center justify-center gap-2 bg-slate-800 hover:bg-slate-700 border border-slate-700 text-slate-200 px-8 py-4 rounded-xl font-bold transition-all">
                  <Zap className="w-5 h-5" />Worker Ol
                </Link>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* Footer */}
      <footer className="border-t border-slate-800/60 py-12 px-6">
        <div className="max-w-5xl mx-auto">
          <div className="flex flex-col sm:flex-row items-center justify-between gap-6">
            <div className="flex items-center gap-3">
              <div className="w-7 h-7 bg-gradient-to-br from-indigo-500 to-purple-600 rounded-lg flex items-center justify-center">
                <Cpu className="w-4 h-4 text-white" />
              </div>
              <span className="font-bold text-slate-300">DecentGPU</span>
            </div>
            <p className="text-xs text-slate-600 text-center">
              TÜBİTAK 2209-A Araştırma Projesi kapsamında geliştirilmiştir.
            </p>
            <div className="flex gap-4 text-xs text-slate-600">
              <Link href="/login" className="hover:text-slate-400 transition-colors">Giriş</Link>
              <Link href="/register" className="hover:text-slate-400 transition-colors">Kayıt</Link>
            </div>
          </div>
        </div>
      </footer>
    </div>
  )
}

'use client'

import { useEffect, useRef, useState, useCallback } from 'react'
import { Terminal as XTerm } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { WebLinksAddon } from '@xterm/addon-web-links'
import '@xterm/xterm/css/xterm.css'
import { getToken } from '@/lib/auth'
import { jobsApi } from '@/lib/api'
import { Trash2 } from 'lucide-react'
import { cn } from '@/lib/utils'

interface TerminalProps {
  jobId: string
  jobStatus: string
}

export function Terminal({ jobId, jobStatus }: TerminalProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const termRef      = useRef<XTerm | null>(null)
  const fitRef       = useRef<FitAddon | null>(null)
  const wsRef        = useRef<WebSocket | null>(null)
  const [connected,  setConnected]  = useState(false)
  const [lineCount,  setLineCount]  = useState(0)

  const clear = useCallback(() => {
    termRef.current?.clear()
    setLineCount(0)
  }, [])

  useEffect(() => {
    if (!containerRef.current) return

    const term = new XTerm({
      theme: {
        background:  '#0d1117',
        foreground:  '#c9d1d9',
        cursor:      '#58a6ff',
        selectionBackground: 'rgba(56, 139, 253, 0.4)',
        black:       '#484f58',
        brightBlack: '#6e7681',
        red:         '#ff7b72',
        brightRed:   '#ffa198',
        green:       '#3fb950',
        brightGreen: '#56d364',
        yellow:      '#d29922',
        brightYellow:'#e3b341',
        blue:        '#58a6ff',
        brightBlue:  '#79c0ff',
        magenta:     '#bc8cff',
        brightMagenta:'#d2a8ff',
        cyan:        '#39c5cf',
        brightCyan:  '#56d4dd',
        white:       '#b1bac4',
        brightWhite: '#f0f6fc',
      },
      fontFamily: '"Cascadia Code", "Fira Code", "JetBrains Mono", Menlo, monospace',
      fontSize: 13,
      lineHeight: 1.5,
      cursorBlink: false,
    })

    const fit   = new FitAddon()
    const links = new WebLinksAddon()
    term.loadAddon(fit)
    term.loadAddon(links)
    term.open(containerRef.current)
    fit.fit()
    termRef.current = term
    fitRef.current  = fit

    const observer = new ResizeObserver(() => fit.fit())
    observer.observe(containerRef.current)

    term.writeln('\x1b[90m# DecentGPU Terminal — bağlanıyor…\x1b[0m')
    // ^ Kept as xterm init message; the 'connected' WS message replaces this with replay status.

    return () => {
      observer.disconnect()
      term.dispose()
      wsRef.current?.close()
    }
  }, [])

  // Connect WebSocket
  useEffect(() => {
    const term  = termRef.current
    if (!term) return

    const token = getToken() ?? ''
    const url   = jobsApi.terminalUrl(jobId, token)
    const ws    = new WebSocket(url)
    wsRef.current = ws

    ws.onopen = () => {
      setConnected(true)
      term.writeln('\x1b[32m● Bağlandı\x1b[0m')
    }

    // Track whether the server already sent a job_done banner so onclose
    // doesn't print a duplicate.
    let jobDoneSeen = false

    ws.onmessage = (ev) => {
      try {
        const msg = JSON.parse(ev.data as string)
        switch (msg.type) {
          case 'connected':
            // Server is about to replay stored logs.
            term.writeln('\x1b[90m# geçmiş loglar yükleniyor…\x1b[0m')
            break

          case 'log':
            // Replayed or live log line — data already has \r\n suffix.
            term.write(msg.data ?? '')
            setLineCount(n => n + 1)
            break

          case 'replay_complete':
            if ((msg.count ?? 0) > 0) {
              term.writeln('\x1b[90m─── geçmiş loglar yüklendi ───\x1b[0m')
            } else {
              term.writeln('\x1b[90m# henüz log yok\x1b[0m')
            }
            break

          case 'job_done':
            jobDoneSeen = true
            if (msg.status === 'completed') {
              term.writeln('\x1b[32m\r\n✓ İş Tamamlandı\x1b[0m')
            } else if (msg.status === 'failed') {
              term.writeln('\x1b[31m\r\n✗ İş Başarısız\x1b[0m')
            } else {
              term.writeln(`\x1b[33m\r\n○ İş Durumu: ${msg.status}\x1b[0m`)
            }
            break

          case 'ping':
            // Keepalive — no display.
            break

          default:
            // Fallback: display message or data field if present.
            if (msg.data) {
              term.write(String(msg.data))
              setLineCount(n => n + 1)
            } else if (msg.message) {
              term.writeln(String(msg.message))
              setLineCount(n => n + 1)
            }
        }
      } catch {
        term.writeln(String(ev.data))
        setLineCount(n => n + 1)
      }
    }

    ws.onerror = () => {
      term.writeln('\x1b[31m● Bağlantı hatası\x1b[0m')
      setConnected(false)
    }

    ws.onclose = () => {
      setConnected(false)
      // Only show a status banner if the server didn't already send job_done.
      if (!jobDoneSeen) {
        if (jobStatus === 'completed') {
          term.writeln('\x1b[32m\r\n✓ İş Tamamlandı\x1b[0m')
        } else if (jobStatus === 'failed') {
          term.writeln('\x1b[31m\r\n✗ İş Başarısız\x1b[0m')
        } else {
          term.writeln('\x1b[90m○ Bağlantı Kesildi\x1b[0m')
        }
      }
    }

    return () => { ws.close() }
  }, [jobId, jobStatus])

  return (
    <div className="flex flex-col h-full rounded-xl overflow-hidden border border-slate-700">
      {/* Terminal header */}
      <div className="flex items-center justify-between bg-slate-900 border-b border-slate-700 px-4 py-2">
        <div className="flex items-center gap-3">
          <span className="text-sm font-medium text-slate-300">Canlı Terminal Çıktısı</span>
          <span className={cn(
            'flex items-center gap-1.5 rounded-full px-2 py-0.5 text-xs',
            connected ? 'bg-emerald-900/40 text-emerald-400' : 'bg-slate-800 text-slate-500'
          )}>
            <span className={cn('h-1.5 w-1.5 rounded-full', connected ? 'bg-emerald-400 animate-pulse' : 'bg-slate-600')} />
            {connected ? 'Bağlandı' : 'Bağlantı Kesildi'}
          </span>
        </div>
        <div className="flex items-center gap-3">
          <span className="text-xs text-slate-500">{lineCount} satır</span>
          <button
            onClick={clear}
            className="flex items-center gap-1 rounded px-2 py-1 text-xs text-slate-400 hover:bg-slate-800 hover:text-slate-200 transition-colors"
          >
            <Trash2 className="h-3.5 w-3.5" />
            Temizle
          </button>
        </div>
      </div>

      {/* xterm container */}
      <div ref={containerRef} className="flex-1 bg-[#0d1117] p-2 min-h-0" />
    </div>
  )
}

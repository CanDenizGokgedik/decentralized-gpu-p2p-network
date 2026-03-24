import type { Metadata } from 'next'
import { Inter } from 'next/font/google'
import './globals.css'
import { Providers } from './providers'

const inter = Inter({ subsets: ['latin'] })

export const metadata: Metadata = {
  title: 'DecentGPU — Dağıtık GPU Kiralama Platformu',
  description:
    'Makine öğrenmesi modellerinizi eğitmek için güvenli, merkezi olmayan GPU kiralama platformu.',
}

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="tr" className="h-full">
      <body className={`min-h-full bg-slate-950 text-slate-100 antialiased ${inter.className}`}>
        <Providers>{children}</Providers>
      </body>
    </html>
  )
}

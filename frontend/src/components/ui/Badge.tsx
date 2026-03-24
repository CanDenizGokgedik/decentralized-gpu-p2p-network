import { HTMLAttributes } from 'react'
import { cn } from '@/lib/utils'

export type BadgeVariant = 'default' | 'success' | 'warning' | 'danger' | 'info' | 'muted'

const variants: Record<BadgeVariant, string> = {
  default: 'bg-slate-700 text-slate-200',
  success: 'bg-emerald-900/60 text-emerald-300 border border-emerald-700',
  warning: 'bg-amber-900/60 text-amber-300 border border-amber-700',
  danger:  'bg-red-900/60 text-red-300 border border-red-700',
  info:    'bg-indigo-900/60 text-indigo-300 border border-indigo-700',
  muted:   'bg-slate-700/60 text-slate-400',
}

interface BadgeProps extends HTMLAttributes<HTMLSpanElement> {
  variant?: BadgeVariant
}

export function Badge({ className, variant = 'default', ...props }: BadgeProps) {
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-medium',
        variants[variant],
        className
      )}
      {...props}
    />
  )
}

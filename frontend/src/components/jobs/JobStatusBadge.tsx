import { Badge } from '@/components/ui/Badge'
import type { BadgeVariant } from '@/components/ui/Badge'
import { statusLabel } from '@/lib/utils'
import type { JobStatus } from '@/types'

const statusVariant: Record<JobStatus, BadgeVariant> = {
  pending:   'warning',
  assigned:  'info',
  running:   'info',
  completed: 'success',
  failed:    'danger',
  cancelled: 'muted',
}

export function JobStatusBadge({ status }: { status: JobStatus }) {
  const variant = statusVariant[status] ?? 'default'
  const label   = statusLabel[status] ?? status
  return (
    <Badge variant={variant}>
      {status === 'running' && (
        <span className="relative flex h-2 w-2">
          <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-indigo-400 opacity-75" />
          <span className="relative inline-flex rounded-full h-2 w-2 bg-indigo-500" />
        </span>
      )}
      {label}
    </Badge>
  )
}

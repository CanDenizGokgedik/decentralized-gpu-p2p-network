'use client'

import { useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { computeUnitsApi } from '@/lib/api'
import { Modal } from '@/components/ui/Modal'
import { Button } from '@/components/ui/Button'
import { Input } from '@/components/ui/Input'
import { useToast } from '@/components/ui/Toast'
import { Coins } from 'lucide-react'

interface AllocateCUModalProps {
  userId:   string
  userEmail: string
  open:     boolean
  onClose:  () => void
}

export function AllocateCUModal({ userId, userEmail, open, onClose }: AllocateCUModalProps) {
  const { toast }      = useToast()
  const queryClient    = useQueryClient()
  const [amount, setAmount] = useState('')
  const [desc,   setDesc]   = useState('')

  const mut = useMutation({
    mutationFn: () => computeUnitsApi.allocate({ user_id: userId, amount: +amount, description: desc || undefined }),
    onSuccess: () => {
      toast(`${amount} CU başarıyla tahsis edildi.`, 'success')
      queryClient.invalidateQueries({ queryKey: ['admin-users'] })
      queryClient.invalidateQueries({ queryKey: ['balance'] })
      setAmount('')
      setDesc('')
      onClose()
    },
    onError: () => toast('CU tahsisi başarısız.', 'error'),
  })

  const valid = +amount >= 1 && +amount <= 1_000_000 && !isNaN(+amount)

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={`CU Tahsis Et — ${userEmail}`}
    >
      <div className="space-y-4">
        <div className="flex items-center gap-3 rounded-lg bg-indigo-950/30 border border-indigo-800/40 px-4 py-3">
          <Coins className="h-5 w-5 text-indigo-400 shrink-0" />
          <p className="text-sm text-indigo-300">
            Kullanıcıya 1 ile 1.000.000 arasında CU tahsis edebilirsiniz.
          </p>
        </div>

        <Input
          label="Miktar (CU)"
          type="number"
          min={1}
          max={1000000}
          value={amount}
          onChange={e => setAmount(e.target.value)}
          placeholder="örn. 10000"
          error={amount && !valid ? '1 ile 1.000.000 arasında bir değer girin' : undefined}
        />

        <Input
          label="Açıklama (opsiyonel)"
          value={desc}
          onChange={e => setDesc(e.target.value)}
          placeholder="Tahsis nedeni…"
        />

        <div className="flex justify-end gap-3 pt-2">
          <Button variant="secondary" onClick={onClose}>
            İptal
          </Button>
          <Button
            onClick={() => mut.mutate()}
            loading={mut.isPending}
            disabled={!valid || mut.isPending}
          >
            <Coins className="h-4 w-4" />
            Tahsis Et
          </Button>
        </div>
      </div>
    </Modal>
  )
}

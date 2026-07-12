import { useEffect, useState } from 'react'
import { useLayoutContext } from '@/lib/layout-context'
import { toast } from 'sonner'
import PCControl from '@/components/PCControl'
import Controller from '@/components/Controller'
import LogReplies from '@/components/LogReplies'
import { WakeDialog } from '@/pages/Dashboard/components/WakeDialog'
import { RebootDialog } from '@/pages/Dashboard/components/RebootDialog'
import { handleLatestReply } from '@/pages/Dashboard/components/reply-handlers'

type WakePhase = 'initial' | 'waiting' | 'confirm' | 'success'

export default function Dashboard() {
  const { device, state, connected, send, replies, markReplyHandled, isReplyHandled } =
    useLayoutContext()

  const [wakeOpen, setWakeOpen] = useState(false)
  const [wakePhase, setWakePhase] = useState<WakePhase>('initial')
  const [wakeToken, setWakeToken] = useState<string | null>(null)
  const [wakeExpiresAt, setWakeExpiresAt] = useState<number | null>(null)
  const [countdown, setCountdown] = useState(0)
  const [rebootOpen, setRebootOpen] = useState(false)

  const startWake = () => {
    setWakePhase('initial')
    setWakeOpen(true)
  }

  const requestWake = () => {
    setWakePhase('waiting')
    send('wake_request', { target: device.name, expires_in_s: 30 })
  }

  const confirmWake = () => {
    if (!wakeToken) return
    send('wake_confirm', { confirm_token: wakeToken, target: device.name })
    setWakePhase('waiting')
  }

  const cancelWake = () => {
    if (wakePhase === 'waiting' || wakePhase === 'confirm') {
      send('wake_cancel')
    }
    setWakeOpen(false)
    setWakePhase('initial')
    setWakeToken(null)
    setWakeExpiresAt(null)
    setCountdown(0)
  }

  const handlePing = () => send('ping')

  const startReboot = () => setRebootOpen(true)

  const confirmReboot = () => {
    send('reboot_request', { expires_in_s: 30 })
  }

  // MQTT reply listener — toast once per reply id, survives navigation
  useEffect(() => {
    const last = replies.at(-1)
    if (last && !isReplyHandled(last.id)) {
      handleLatestReply(last, {
        deviceName: device.name,
        stateTs: state?.ts ?? null,
        setWakePhase,
        setWakeToken,
        setWakeExpiresAt,
        setRebootOpen,
      })
      markReplyHandled(last.id)
    }
  }, [replies, device.name, markReplyHandled, isReplyHandled, state])

  // Auto-close wake dialog after success
  useEffect(() => {
    if (wakePhase !== 'success') return
    const t = setTimeout(() => {
      setWakeOpen(false)
      setWakePhase('initial')
      setWakeToken(null)
      setWakeExpiresAt(null)
      setCountdown(0)
    }, 2000)
    return () => clearTimeout(t)
  }, [wakePhase])

  // Expire wake confirmation via setTimeout (one shot, no polling)
  useEffect(() => {
    if (wakePhase !== 'confirm' || !wakeExpiresAt) return
    const remaining = wakeExpiresAt - Date.now()
    if (remaining <= 0) {
      setWakePhase('initial')
      setWakeToken(null)
      setWakeExpiresAt(null)
      setCountdown(0)
      toast.error('Wake confirmation expired')
      return
    }
    setCountdown(Math.ceil(remaining / 1000))
    const t = setTimeout(() => {
      setWakePhase('initial')
      setWakeToken(null)
      setWakeExpiresAt(null)
      setCountdown(0)
      toast.error('Wake confirmation expired')
    }, remaining)
    return () => clearTimeout(t)
  }, [wakePhase, wakeExpiresAt])

  // Local countdown tick during confirm phase
  useEffect(() => {
    if (wakePhase !== 'confirm' || !wakeExpiresAt) return
    const tick = setInterval(() => {
      setCountdown(Math.max(0, Math.ceil((wakeExpiresAt - Date.now()) / 1000)))
    }, 1000)
    return () => clearInterval(tick)
  }, [wakePhase, wakeExpiresAt])

  return (
    <>
      {/* Desktop: grid */}
      <div className="hidden h-full grid-cols-2 grid-rows-[auto_minmax(0,1fr)] gap-[18px] lg:grid">
        <PCControl
          device={device}
          connected={connected}
          wakePending={state?.wake_pending ?? false}
          onWake={startWake}
          onPing={handlePing}
          onReboot={startReboot}
        />
        <div className="min-w-0">
          <Controller device={device} />
        </div>
        <div className="col-span-2 h-full min-h-0">
          <LogReplies />
        </div>
      </div>
      {/* Mobile: single column */}
      <div className="flex min-h-full flex-col gap-[18px] lg:hidden">
        <PCControl
          device={device}
          connected={connected}
          wakePending={state?.wake_pending ?? false}
          onWake={startWake}
          onPing={handlePing}
          onReboot={startReboot}
        />
        <Controller device={device} />
        <div className="min-h-0 flex-1">
          <LogReplies />
        </div>
      </div>

      <WakeDialog
        open={wakeOpen}
        phase={wakePhase}
        countdown={countdown}
        deviceName={device.name}
        onCancel={cancelWake}
        onRequestWake={requestWake}
        onConfirmWake={confirmWake}
      />

      <RebootDialog open={rebootOpen} onOpenChange={setRebootOpen} onConfirm={confirmReboot} />
    </>
  )
}

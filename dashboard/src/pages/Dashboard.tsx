import { useEffect, useState } from 'react'
import { useLayoutContext } from '@/components/Layout'
import { toast } from 'sonner'
import PCControl from '@/components/PCControl'
import DeviceStatus from '@/components/DeviceStatus'
import LogReplies from '@/components/LogReplies'
import ConnectionHealth from '@/components/ConnectionHealth'
import { readText, readNumber, resolveExpiresAt } from '@/lib/helpers'
import { Button } from '@/components/ui/button'
import {
  Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter,
} from '@/components/ui/dialog'
import {
  AlertDialog, AlertDialogContent, AlertDialogHeader, AlertDialogTitle,
  AlertDialogDescription, AlertDialogFooter, AlertDialogAction, AlertDialogCancel,
} from '@/components/ui/alert-dialog'

type WakePhase = 'initial' | 'waiting' | 'confirm' | 'success'

export default function Dashboard() {
  const { device, state, connected, send, replies } = useLayoutContext()

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
    send('wake_cancel')
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

  // MQTT reply listener
  useEffect(() => {
    const last = replies.at(-1)
    if (!last) return

    if (last.cmd === 'wake_request' && last.ok) {
      const token = readText(last.confirm_token)
      setWakePhase('confirm')
      setWakeToken(token)
      setWakeExpiresAt(resolveExpiresAt(last, 30))
      toast('Confirmation ready', { description: 'The controller issued a wake token.' })
      return
    }

    if (last.cmd === 'wake_request' && !last.ok) {
      setWakePhase('initial')
      toast.error(readText(last.message) || 'Wake request failed')
      return
    }

    if (last.cmd === 'wake_confirm' && last.ok) {
      setWakePhase('success')
      toast.success('Wake packet sent to ' + device.name)
      return
    }

    if (last.cmd === 'wake_confirm' && !last.ok) {
      setWakePhase('confirm')
      toast.error(readText(last.message) || 'Wake confirm failed')
      return
    }

    if (last.cmd === 'ping') {
      const latency = readNumber(last.latencyMs) ?? readNumber(last.rtt_ms)
      if (last.ok) {
        toast.success(latency ? `ESP32 replied in ${latency} ms` : 'ESP32 replied')
      } else {
        toast.error(readText(last.message) || 'Ping failed')
      }
      return
    }

    if (last.cmd === 'reboot_request' && !last.ok) {
      toast.error(readText(last.message) || 'Reboot request failed')
      setRebootOpen(false)
      return
    }

    if (last.cmd === 'reboot_confirm' && last.ok) {
      toast.success('ESP32 rebooting')
      setRebootOpen(false)
      return
    }
  }, [replies, device.name])

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
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-[18px] items-start">
        <PCControl device={device} connected={connected} wakePending={state?.wake_pending ?? false} onWake={startWake} onPing={handlePing} onReboot={startReboot} />
        <DeviceStatus device={device} />
        <LogReplies />
        <ConnectionHealth device={device} />
      </div>

      {/* Wake Dialog */}
      <Dialog open={wakeOpen} onOpenChange={(open) => { if (!open) cancelWake() }}>
        <DialogContent className="sm:max-w-sm" showCloseButton={false}>
          <DialogHeader>
            <DialogTitle>
              {wakePhase === 'initial' && 'Wake Desktop PC?'}
              {wakePhase === 'waiting' && 'Waiting for controller'}
              {wakePhase === 'confirm' && 'Confirm wake'}
              {wakePhase === 'success' && 'Wake packet sent'}
            </DialogTitle>
            <DialogDescription>
              {wakePhase === 'initial' && 'Request confirmation from the ESP32 before sending the Wake-on-LAN packet.'}
              {wakePhase === 'waiting' && 'Requesting a temporary confirmation token from ' + device.name + '.'}
              {wakePhase === 'confirm' && `The request expires in ${countdown} seconds.`}
              {wakePhase === 'success' && device.name + ' sent the Wake-on-LAN packet.'}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            {wakePhase === 'initial' && (
              <>
                <Button variant="outline" onClick={cancelWake}>Cancel</Button>
                <Button variant="default" onClick={requestWake}>Continue</Button>
              </>
            )}
            {wakePhase === 'waiting' && (
              <Button variant="outline" disabled>Waiting…</Button>
            )}
            {wakePhase === 'confirm' && (
              <>
                <Button variant="outline" onClick={cancelWake}>Cancel</Button>
                <Button variant="default" onClick={confirmWake}>Wake PC</Button>
              </>
            )}
            {wakePhase === 'success' && (
              <span className="text-xs text-muted-foreground">Closing…</span>
            )}
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Reboot AlertDialog */}
      <AlertDialog open={rebootOpen} onOpenChange={setRebootOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Reboot ESP32?</AlertDialogTitle>
            <AlertDialogDescription>
              The controller will disconnect briefly and reconnect automatically.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction onClick={confirmReboot}>Reboot</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  )
}

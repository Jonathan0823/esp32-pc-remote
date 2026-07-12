import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog'

type WakePhase = 'initial' | 'waiting' | 'confirm' | 'success'

export function WakeDialog({
  open,
  phase,
  countdown,
  deviceName,
  onCancel,
  onRequestWake,
  onConfirmWake,
}: Readonly<{
  open: boolean
  phase: WakePhase
  countdown: number
  deviceName: string
  onCancel: () => void
  onRequestWake: () => void
  onConfirmWake: () => void
}>) {
  return (
    <Dialog open={open} onOpenChange={(open) => { if (!open) onCancel() }}>
      <DialogContent className="sm:max-w-sm" showCloseButton={false}>
        <DialogHeader>
          <DialogTitle>
            {phase === 'initial' && 'Wake Desktop PC?'}
            {phase === 'waiting' && 'Waiting for controller'}
            {phase === 'confirm' && 'Confirm wake'}
            {phase === 'success' && 'Wake packet sent'}
          </DialogTitle>
          <DialogDescription>
            {phase === 'initial' &&
              'Request confirmation from the ESP32 before sending the Wake-on-LAN packet.'}
            {phase === 'waiting' &&
              'Requesting a temporary confirmation token from ' + deviceName + '.'}
            {phase === 'confirm' && `The request expires in ${countdown} seconds.`}
            {phase === 'success' && deviceName + ' sent the Wake-on-LAN packet.'}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          {phase === 'initial' && (
            <>
              <Button variant="outline" onClick={onCancel}>
                Cancel
              </Button>
              <Button variant="default" onClick={onRequestWake}>
                Continue
              </Button>
            </>
          )}
          {phase === 'waiting' && (
            <Button variant="outline" disabled>
              Waiting…
            </Button>
          )}
          {phase === 'confirm' && (
            <>
              <Button variant="outline" onClick={onCancel}>
                Cancel
              </Button>
              <Button variant="default" onClick={onConfirmWake}>
                Wake PC
              </Button>
            </>
          )}
          {phase === 'success' && (
            <span className="text-muted-foreground text-xs">Closing…</span>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

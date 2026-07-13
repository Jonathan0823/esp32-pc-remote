import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import type { DeviceData } from '@/lib/types'
import LiveTime from '@/components/LiveTime'
import {
  MonitorIcon,
  ClockIcon,
  CheckCircleIcon,
  PowerIcon,
  PulseIcon,
  ArrowClockwiseIcon,
} from '@phosphor-icons/react'

type PCControlProps = Readonly<{
  device: DeviceData
  connected: boolean
  wakePending: boolean
  onWake: () => void
  onPing: () => void
  onReboot: () => void
}>

export default function PCControl({
  device,
  connected,
  wakePending,
  onWake,
  onPing,
  onReboot,
}: PCControlProps) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>PC Control</CardTitle>
      </CardHeader>
      <CardContent className="grid gap-3">
        <div className="grid grid-cols-1 items-center gap-4 sm:grid-cols-[auto_1fr]">
          <div className="flex items-center justify-center justify-self-center sm:justify-self-auto">
            <MonitorIcon
              className="text-muted-foreground/40 size-[80px] sm:size-[100px]"
              weight="thin"
            />
          </div>
          <div className="grid gap-2">
            <div className="flex items-center gap-2">
              <span className="text-base font-semibold">Desktop PC</span>
              <Badge
                variant="outline"
                className={`text-[10px] ${
                  device.ready
                    ? 'border-emerald-500/30 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
                    : 'border-amber-500/30 bg-amber-500/10 text-amber-600 dark:text-amber-400'
                }`}
              >
                {device.ready ? 'Ready' : 'Unavailable'}
              </Badge>
            </div>
            <div className="grid grid-cols-[auto_auto_1fr] gap-x-2 gap-y-1.5 text-xs">
              <ClockIcon className="text-muted-foreground size-3.5 self-center" />
              <span className="text-foreground self-center font-medium">Last Wake</span>
              <span className="text-muted-foreground self-center">
                :{' '}
                {device.lastWakeAbsMs ? (
                  <LiveTime refTimestamp={device.lastWakeAbsMs} />
                ) : (
                  device.lastWake
                )}
              </span>
              <CheckCircleIcon className="text-muted-foreground size-3.5 self-center" />
              <span className="text-foreground self-center font-medium">Last Wake Status</span>
              <span
                className={`self-center ${
                  device.lastWakeStatus.toLowerCase().includes('sent')
                    ? 'text-emerald-500'
                    : 'text-muted-foreground'
                }`}
              >
                : {device.lastWakeStatus}
              </span>
            </div>
          </div>
        </div>

        <Button
          variant="default"
          size="lg"
          className="h-14 w-full gap-2 text-sm font-bold"
          disabled={!connected || wakePending}
          onClick={onWake}
        >
          <PowerIcon className="size-5" />
          WAKE PC
        </Button>

        <div className="grid grid-cols-2 gap-3">
          <Button
            variant="outline"
            className="h-[50px] gap-2 text-xs font-semibold"
            disabled={!connected}
            onClick={onPing}
          >
            <PulseIcon className="size-4" />
            PING ESP32
          </Button>
          <Button
            variant="destructive"
            className="h-[50px] gap-2 text-xs font-semibold"
            disabled={!connected}
            onClick={onReboot}
          >
            <ArrowClockwiseIcon className="size-4" />
            REBOOT ESP32
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}

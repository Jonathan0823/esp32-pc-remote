import { Card, CardHeader, CardTitle, CardContent, CardAction } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { DropdownMenu, DropdownMenuTrigger, DropdownMenuContent, DropdownMenuItem } from '@/components/ui/dropdown-menu'
import type { DeviceData } from '@/lib/types'
import { MonitorIcon, ClockIcon, CheckCircleIcon, PowerIcon, PulseIcon, ArrowClockwiseIcon, DotsThreeIcon } from '@phosphor-icons/react'

interface PCControlProps {
  device: DeviceData
  connected: boolean
  wakePending: boolean
  onWake: () => void
  onPing: () => void
  onReboot: () => void
}

export default function PCControl({ device, connected, wakePending, onWake, onPing, onReboot }: PCControlProps) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>PC Control</CardTitle>
        <CardAction>
          <DropdownMenu>
            <DropdownMenuTrigger>
              <Button variant="ghost" size="icon-sm">
                <DotsThreeIcon className="size-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem>Refresh</DropdownMenuItem>
              <DropdownMenuItem>Settings</DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </CardAction>
      </CardHeader>
      <CardContent className="grid gap-5">
        <div className="grid grid-cols-1 sm:grid-cols-[auto_1fr] gap-4 items-center justify-items-center sm:justify-items-start">
          <div className="flex items-center justify-center">
            <MonitorIcon className="size-[80px] sm:size-[132px] text-muted-foreground/40" weight="thin" />
          </div>
          <div className="grid gap-2">
            <div className="flex items-center gap-2">
              <span className="text-base font-semibold">Desktop PC</span>
              <Badge variant="secondary" className="text-[10px]">{device.ready ? 'Ready' : 'Unavailable'}</Badge>
            </div>
            <div className="grid gap-1.5">
              <div className="flex items-center gap-2 text-xs text-muted-foreground">
                <ClockIcon className="size-3.5" />
                <span className="font-medium text-foreground">Last Wake:</span>
                <span>{device.lastWake}</span>
              </div>
              <div className="flex items-center gap-2 text-xs text-muted-foreground">
                <CheckCircleIcon className="size-3.5" />
                <span className="font-medium text-foreground">Last Wake Status:</span>
                <span>{device.lastWakeStatus}</span>
              </div>
            </div>
          </div>
        </div>

        <Button variant="default" size="lg" className="w-full h-14 text-sm font-bold gap-2" disabled={!connected || wakePending} onClick={onWake}>
          <PowerIcon className="size-5" />
          WAKE PC
        </Button>

        <div className="grid grid-cols-2 gap-3">
          <Button variant="outline" className="h-[50px] text-xs font-semibold gap-2" disabled={!connected} onClick={onPing}>
            <PulseIcon className="size-4" />
            PING ESP32
          </Button>
          <Button variant="outline" className="h-[50px] text-xs font-semibold gap-2" disabled={!connected} onClick={onReboot}>
            <ArrowClockwiseIcon className="size-4" />
            REBOOT ESP32
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}

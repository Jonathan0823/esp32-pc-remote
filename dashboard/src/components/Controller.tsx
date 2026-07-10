import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import type { DeviceData } from '@/lib/types'
import {
  WifiHighIcon,
  ChartBarIcon,
  ClockIcon,
  MemoryIcon,
  MonitorIcon,
} from '@phosphor-icons/react'

interface ControllerProps {
  device: DeviceData
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between py-2 text-xs">
      <span className="text-muted-foreground">{label}</span>
      <span className="text-foreground font-medium">{value}</span>
    </div>
  )
}

export default function Controller({ device }: ControllerProps) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Controller</CardTitle>
      </CardHeader>
      <CardContent className="grid gap-4">
        <div className="divide-border grid grid-cols-4 divide-x rounded-lg border">
          <div className="flex flex-col items-center gap-1 p-3 text-center">
            <WifiHighIcon className="text-muted-foreground size-5" />
            <span className="text-muted-foreground text-[10px] tracking-wider uppercase">RSSI</span>
            <span className="text-foreground text-sm font-semibold">{device.rssi} dBm</span>
          </div>
          <div className="flex flex-col items-center gap-1 p-3 text-center">
            <ChartBarIcon className="text-muted-foreground size-5" />
            <span className="text-muted-foreground text-[10px] tracking-wider uppercase">
              Signal
            </span>
            <span className="text-foreground text-sm font-semibold">{device.signalQuality}%</span>
          </div>
          <div className="flex flex-col items-center gap-1 p-3 text-center">
            <ClockIcon className="text-muted-foreground size-5" />
            <span className="text-muted-foreground text-[10px] tracking-wider uppercase">
              Uptime
            </span>
            <span className="text-foreground text-sm font-semibold">{device.uptime}</span>
          </div>
          <div className="flex flex-col items-center gap-1 p-3 text-center">
            <MemoryIcon className="text-muted-foreground size-5" />
            <span className="text-muted-foreground text-[10px] tracking-wider uppercase">Heap</span>
            <span className="text-foreground text-sm font-semibold">{device.freeHeap}</span>
          </div>
        </div>

        <div>
          <div className="border-border/30 flex items-center gap-2 border-b py-2 text-xs">
            <MonitorIcon className="text-muted-foreground size-4" />
            <span className="text-foreground font-medium">{device.controllerLabel}</span>
          </div>
          <InfoRow label="Status" value={device.online ? 'Online' : 'Offline'} />
          <InfoRow label="IP Address" value={device.ipAddress} />
          <InfoRow label="Last Update" value={device.lastUpdateAgo} />
          <InfoRow label="MQTT" value={device.mqttStatus} />
          <InfoRow label="Broker" value={device.broker} />
        </div>
      </CardContent>
    </Card>
  )
}

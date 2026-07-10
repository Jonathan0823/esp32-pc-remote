import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import type { DeviceData } from '@/lib/types'
import {
  WifiHighIcon,
  ChartBarIcon,
  ClockIcon,
  MemoryIcon,
  ComputerTowerIcon,
  GlobeSimpleIcon,
  MonitorIcon,
} from '@phosphor-icons/react'

interface ControllerProps {
  device: DeviceData
}

function InfoRow({
  icon: Icon,
  label,
  value,
}: {
  icon: React.ComponentType<{ className?: string }>
  label: string
  value: string
}) {
  return (
    <div className="flex items-center gap-3 text-xs min-w-0">
      <Icon className="text-muted-foreground size-4 shrink-0" />
      <span className="text-muted-foreground shrink-0 whitespace-nowrap">{label}</span>
      <span className="text-foreground min-w-0 flex-1 truncate text-right font-medium">{value}</span>
    </div>
  )
}

export default function Controller({ device }: ControllerProps) {
  return (
    <Card className="overflow-hidden">
      <CardHeader>
        <CardTitle>Controller</CardTitle>
      </CardHeader>
      <CardContent className="grid gap-2">
        <span className="text-muted-foreground font-medium">{device.controllerLabel}</span>

        <div className="divide-border grid grid-cols-4 divide-x border-t border-b">
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

        <div className="space-y-3.5">
          <InfoRow
            icon={ComputerTowerIcon}
            label="Status"
            value={device.online ? 'Online' : 'Offline'}
          />
          <InfoRow icon={GlobeSimpleIcon} label="IP Address" value={device.ipAddress} />
          <InfoRow icon={ClockIcon} label="Last Update" value={device.lastUpdateAgo} />
          <InfoRow icon={WifiHighIcon} label="MQTT" value={device.mqttStatus} />
          <InfoRow icon={MonitorIcon} label="Broker" value={device.broker} />
        </div>
      </CardContent>
    </Card>
  )
}

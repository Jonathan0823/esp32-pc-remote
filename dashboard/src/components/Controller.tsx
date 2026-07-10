import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import type { DeviceData } from '@/lib/types'
import {
  WifiHighIcon,
  ChartBarIcon,
  ClockIcon,
  MemoryIcon,
} from '@phosphor-icons/react'

interface ControllerProps {
  device: DeviceData
}

function StatBox({
  icon: Icon,
  label,
  value,
}: {
  icon: React.ComponentType<{ className?: string }>
  label: string
  value: string
}) {
  return (
    <div className="flex flex-col items-center gap-1 rounded-lg border p-3 text-center">
      <Icon className="text-muted-foreground size-5" />
      <span className="text-muted-foreground text-[10px] tracking-wider uppercase">{label}</span>
      <span className="text-foreground text-sm font-semibold">{value}</span>
    </div>
  )
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
        <div className="grid grid-cols-4 gap-2">
          <StatBox icon={WifiHighIcon} label="RSSI" value={`${device.rssi} dBm`} />
          <StatBox icon={ChartBarIcon} label="Signal" value={`${device.signalQuality}%`} />
          <StatBox icon={ClockIcon} label="Update" value={device.lastUpdateAgo} />
          <StatBox icon={MemoryIcon} label="Heap" value={device.freeHeap} />
        </div>
        <div>
          <InfoRow label="Status" value={device.online ? 'Online' : 'Offline'} />
          <InfoRow label="IP Address" value={device.ipAddress} />
          <InfoRow label="MQTT Connection" value={device.mqttStatus} />
          <InfoRow label="Broker" value={device.broker} />
          <InfoRow label="Uptime" value={device.uptime} />
        </div>
      </CardContent>
    </Card>
  )
}

import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import { Separator } from '@/components/ui/separator'
import type { DeviceData } from '@/lib/types'

interface ConnectionHealthProps {
  device: DeviceData
}

function HealthRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid grid-cols-[auto_1fr] items-center gap-x-2 py-[12.5px] text-xs">
      <span className="text-muted-foreground">{label}</span>
      <span className="text-foreground text-right font-medium break-words">{value}</span>
    </div>
  )
}

export default function ConnectionHealth({ device }: ConnectionHealthProps) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Connection Health</CardTitle>
      </CardHeader>
      <CardContent className="grid">
        <HealthRow label="MQTT Connection" value={device.mqttStatus} />
        <Separator />
        <HealthRow label="Broker" value={device.broker} />
        <Separator />
        <HealthRow label="Last Update" value={device.lastUpdateAgo} />
      </CardContent>
    </Card>
  )
}

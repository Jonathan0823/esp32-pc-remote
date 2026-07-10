import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import { Separator } from '@/components/ui/separator'
import type { DeviceData } from '@/lib/types'

interface ConnectionHealthProps {
  device: DeviceData
}

function HealthRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid grid-cols-[auto_minmax(0,1fr)] items-start gap-x-2 py-[12.5px] text-xs">
      <span className="text-muted-foreground">{label}</span>
      <div className="min-w-0 overflow-hidden">
        <span className="text-foreground block text-right font-medium break-words">{value}</span>
      </div>
    </div>
  )
}

export default function ConnectionHealth({ device }: ConnectionHealthProps) {
  return (
    <Card className="w-full">
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

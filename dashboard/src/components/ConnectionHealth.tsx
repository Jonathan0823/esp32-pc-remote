import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import { Separator } from '@/components/ui/separator'
import { Progress } from '@/components/ui/progress'
import type { DeviceData } from '@/lib/types'

interface ConnectionHealthProps {
  device: DeviceData
}

function HealthRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between py-[12.5px] text-xs">
      <span className="text-muted-foreground">{label}</span>
      <span className="font-medium text-foreground">{value}</span>
    </div>
  )
}

export default function ConnectionHealth({ device }: ConnectionHealthProps) {
  return (
    <Card className="[grid-area:connectionHealth]">
      <CardHeader>
        <CardTitle>Connection Health</CardTitle>
      </CardHeader>
      <CardContent className="grid">
        <HealthRow label="MQTT Connection" value={device.mqttStatus} />
        <Separator />
        <HealthRow label="Grafana Integration" value={device.grafanaStatus} />
        <Separator />
        <HealthRow label="Broker" value={device.broker} />
        <Separator />
        <HealthRow label="Uptime (Device)" value={device.uptime} />
        <Separator />
        <div className="flex items-center justify-between py-[12.5px] text-xs">
          <span className="text-muted-foreground">Signal Quality</span>
          <div className="flex items-center gap-3 min-w-0 flex-1 ml-4">
            <Progress value={device.signalQuality} className="h-2 flex-1" />
            <span className="text-xs font-medium tabular-nums shrink-0">{device.signalQuality}%</span>
          </div>
        </div>
      </CardContent>
    </Card>
  )
}

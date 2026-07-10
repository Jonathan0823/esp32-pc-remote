import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import { Separator } from '@/components/ui/separator'
import type { DeviceData } from '@/lib/types'
import { ComputerTowerIcon, WifiHighIcon, GlobeSimpleIcon, ClockIcon, MemoryIcon, CloudIcon, CheckCircleIcon, XCircleIcon } from '@phosphor-icons/react'

interface DeviceStatusProps {
  device: DeviceData
}

function StatusRow({ icon: Icon, label, value }: { icon: React.ComponentType<{ className?: string }>; label: string; value: string }) {
  return (
    <div className="flex items-center gap-3 py-[12.5px] text-xs">
      <Icon className="size-4 text-muted-foreground shrink-0" />
      <span className="text-muted-foreground">{label}</span>
      <span className="ml-auto font-medium text-foreground">{value}</span>
    </div>
  )
}

export default function DeviceStatus({ device }: DeviceStatusProps) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Device Status</CardTitle>
      </CardHeader>
      <CardContent className="grid">
        <StatusRow icon={ComputerTowerIcon} label="Status" value={device.online ? 'Online' : 'Offline'} />
        <Separator />
        <StatusRow icon={WifiHighIcon} label="Wi-Fi RSSI" value={`${device.rssi} dBm`} />
        <Separator />
        <StatusRow icon={GlobeSimpleIcon} label="IP Address" value={device.ipAddress} />
        <Separator />
        <StatusRow icon={ClockIcon} label="Uptime" value={device.uptime} />
        <Separator />
        <StatusRow icon={MemoryIcon} label="Free Heap" value={device.freeHeap} />
        <Separator />
        <StatusRow icon={device.pcOnline ? CheckCircleIcon : XCircleIcon} label="PC Online" value={device.pcOnline ? 'Online' : 'Offline'} />
        <Separator />
        <StatusRow icon={CloudIcon} label="MQTT" value={device.mqttStatus} />
      </CardContent>
    </Card>
  )
}

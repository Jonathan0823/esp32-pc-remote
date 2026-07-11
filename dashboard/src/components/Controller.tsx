import { type ReactNode } from 'react'
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import LiveTime from '@/components/LiveTime'
import type { DeviceData } from '@/lib/types'
import { cn } from '@/lib/utils'
import {
  WifiHighIcon,
  ChartBarIcon,
  ClockIcon,
  MemoryIcon,
  ComputerTowerIcon,
  GlobeSimpleIcon,
  MonitorIcon,
} from '@phosphor-icons/react'

type ControllerProps = Readonly<{
  device: DeviceData
}>

function InfoRow({
  icon: Icon,
  label,
  value,
  iconClassName,
  valueClassName,
}: Readonly<{
  icon: React.ComponentType<{ className?: string }>
  label: string
  value: ReactNode
  iconClassName?: string
  valueClassName?: string
}>) {
  return (
    <div className="flex min-w-0 items-center gap-3 text-xs">
      <Icon className={cn('size-4 shrink-0', iconClassName ?? 'text-muted-foreground')} />
      <span className="text-muted-foreground shrink-0 whitespace-nowrap">{label}</span>
      <span
        className={cn(
          'min-w-0 flex-1 truncate text-right font-medium',
          valueClassName ?? 'text-foreground',
        )}
      >
        {value}
      </span>
    </div>
  )
}

export default function Controller({ device }: ControllerProps) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Controller</CardTitle>
      </CardHeader>
      <CardContent>
        <span className="text-muted-foreground font-medium">{device.controllerLabel}</span>

        <div className="divide-border my-2 grid grid-cols-4 divide-x border-t border-b">
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
            <span className="text-foreground text-sm font-semibold">
              <LiveTime
                refTimestamp={device.stateRefreshedAt}
                uptimeBaseSeconds={device.uptimeSeconds}
              />
            </span>
          </div>
          <div className="flex flex-col items-center gap-1 p-3 text-center">
            <MemoryIcon className="text-muted-foreground size-5" />
            <span className="text-muted-foreground text-[10px] tracking-wider uppercase">Heap</span>
            <span className="text-foreground text-sm font-semibold">{device.freeHeap}</span>
          </div>
        </div>

        <div className="space-y-3">
          <InfoRow
            icon={ComputerTowerIcon}
            label="Status"
            value={device.online ? 'Online' : 'Offline'}
            valueClassName={device.online ? 'text-emerald-500' : 'text-rose-500'}
          />
          <InfoRow icon={GlobeSimpleIcon} label="IP Address" value={device.ipAddress} />
          <InfoRow
            icon={ClockIcon}
            label="Last Update"
            value={<LiveTime refTimestamp={device.stateRefreshedAt} />}
          />
          <InfoRow
            icon={WifiHighIcon}
            label="MQTT"
            value={device.mqttStatus}
            valueClassName={device.mqttStatus === 'Connected' ? 'text-sky-500' : 'text-rose-500'}
          />
          <InfoRow icon={MonitorIcon} label="Broker" value={device.broker} />
        </div>
      </CardContent>
    </Card>
  )
}

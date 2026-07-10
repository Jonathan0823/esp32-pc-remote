import { Card, CardHeader, CardTitle, CardContent, CardAction } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import { ScrollArea, ScrollBar } from '@/components/ui/scroll-area'
import { PaperPlaneRight, Pulse, Heart, WifiHigh, Cloud, Bell } from '@phosphor-icons/react'

interface EventItem {
  time: string
  title: string
  type: string
  icon: string
}

const EVENTS: EventItem[] = [
  { time: '15:44:21', title: 'WOL packet sent to Desktop PC', type: 'WAKE', icon: 'Send' },
  { time: '15:43:58', title: 'Ping replied (230 ms)', type: 'PING', icon: 'Activity' },
  { time: '15:41:12', title: 'Heartbeat updated', type: 'SYSTEM', icon: 'Heart' },
  { time: '15:39:47', title: 'ESP32 connected to Wi-Fi', type: 'SYSTEM', icon: 'Wifi' },
  { time: '15:39:45', title: 'ESP32 connected to MQTT Broker', type: 'SYSTEM', icon: 'Cloud' },
]

function EventIcon({ name }: { name: string }) {
  switch (name) {
    case 'Send': return <PaperPlaneRight className="size-4 text-muted-foreground shrink-0" />
    case 'Activity': return <Pulse className="size-4 text-muted-foreground shrink-0" />
    case 'Heart': return <Heart className="size-4 text-muted-foreground shrink-0" />
    case 'Wifi': return <WifiHigh className="size-4 text-muted-foreground shrink-0" />
    case 'Cloud': return <Cloud className="size-4 text-muted-foreground shrink-0" />
    default: return <Bell className="size-4 text-muted-foreground shrink-0" />
  }
}

export default function RecentEvents() {
  return (
    <Card className="[grid-area:recentEvents]">
      <CardHeader>
        <CardTitle>Recent Events</CardTitle>
        <CardAction>
          <Button variant="outline" size="sm">View all</Button>
        </CardAction>
      </CardHeader>
      <CardContent className="p-0">
        <ScrollArea className="h-full">
          <div className="grid px-(--card-spacing)">
            {EVENTS.map((event, i) => (
              <div key={i}>
                {i > 0 && <Separator />}
                <div className="flex items-center gap-3 py-[13px] text-xs">
                  <EventIcon name={event.icon} />
                  <span className="text-muted-foreground tabular-nums shrink-0">{event.time}</span>
                  <span className="flex-1 truncate text-foreground">{event.title}</span>
                  <Badge variant="secondary" className="text-[10px] shrink-0">{event.type}</Badge>
                </div>
              </div>
            ))}
          </div>
          <ScrollBar orientation="vertical" />
        </ScrollArea>
      </CardContent>
    </Card>
  )
}

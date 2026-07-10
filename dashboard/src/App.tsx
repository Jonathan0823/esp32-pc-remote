import { useEffect, useMemo, useState } from 'react'
import { useMqtt } from './mqtt/useMqtt'
import { toast } from 'sonner'
import './App.css'

import { SidebarProvider, Sidebar, SidebarHeader, SidebarContent, SidebarFooter, SidebarMenu, SidebarMenuItem, SidebarMenuButton, SidebarTrigger } from '@/components/ui/sidebar'
import { Card, CardHeader, CardTitle, CardContent, CardAction } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Separator } from '@/components/ui/separator'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '@/components/ui/dialog'
import { AlertDialog, AlertDialogContent, AlertDialogHeader, AlertDialogTitle, AlertDialogDescription, AlertDialogFooter, AlertDialogAction, AlertDialogCancel } from '@/components/ui/alert-dialog'
import { ScrollArea, ScrollBar } from '@/components/ui/scroll-area'
import { TooltipProvider } from '@/components/ui/tooltip'
import { DropdownMenu, DropdownMenuTrigger, DropdownMenuContent, DropdownMenuItem } from '@/components/ui/dropdown-menu'
import { ThemeProvider, useTheme } from '@/components/ui/theme-provider'
import { Toaster } from '@/components/ui/sonner'

import {
  Monitor, Layout, Bell, FileText, Gear, Question,
  WifiHigh, ChartBar, Power, Pulse, ArrowClockwise,
  ComputerTower, GlobeSimple, Memory, Thermometer, Cloud,
  Clock, CheckCircle, Heart, PaperPlaneRight,
  DotsThree, Sun, Moon,
} from '@phosphor-icons/react'

const PC_NAME = import.meta.env.VITE_PC_NAME || 'Desktop-01'
const BROKER_URL = import.meta.env.VITE_MQTT_BROKER_URL as string | undefined

type WakePhase = 'initial' | 'waiting' | 'confirm' | 'success'

interface DeviceData {
  name: string
  controllerLabel: string
  online: boolean
  ready: boolean
  lastUpdateAgo: string
  rssi: number
  ipAddress: string
  uptime: string
  freeHeap: string
  cpuTemp: string
  mqttStatus: string
  grafanaStatus: string
  broker: string
  signalQuality: number
  lastWake: string
  lastWakeStatus: string
}

const STATIC_DEVICE: DeviceData = {
  name: 'Desktop-01',
  controllerLabel: 'ESP32 Controller',
  online: true,
  ready: true,
  lastUpdateAgo: '5 sec ago',
  rssi: -49,
  ipAddress: '192.168.100.17',
  uptime: '10m 08s',
  freeHeap: '204 KB',
  cpuTemp: '34.2 °C',
  mqttStatus: 'Connected',
  grafanaStatus: 'Last sent: 1m ago',
  broker: 'HiveMQ Cloud',
  signalQuality: 82,
  lastWake: 'Today 15:44',
  lastWakeStatus: 'WOL packet sent',
}

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

function AppWrapper() {
  return (
    <ThemeProvider defaultTheme="dark" storageKey="pc-remote-theme">
      <App />
    </ThemeProvider>
  )
}

function App() {
  const { connection, state, replies, send } = useMqtt()
  const [now, setNow] = useState(() => Date.now())

  // Wake dialog state
  const [wakeOpen, setWakeOpen] = useState(false)
  const [wakePhase, setWakePhase] = useState<WakePhase>('initial')
  const [wakeToken, setWakeToken] = useState<string | null>(null)
  const [wakeExpiresAt, setWakeExpiresAt] = useState<number | null>(null)

  // Reboot dialog state
  const [rebootOpen, setRebootOpen] = useState(false)

  // Clock tick
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 1000)
    return () => clearInterval(t)
  }, [])

  // Listen for MQTT replies
  useEffect(() => {
    const last = replies.at(-1)
    if (!last) return

    if (last.cmd === 'wake_request' && last.ok) {
      const token = readText(last.confirm_token)
      setWakePhase('confirm')
      setWakeToken(token)
      setWakeExpiresAt(resolveExpiresAt(last, 30))
      toast('Confirmation ready', { description: 'The controller issued a wake token.' })
      return
    }

    if (last.cmd === 'wake_request' && !last.ok) {
      setWakePhase('initial')
      toast.error(readText(last.message) || 'Wake request failed')
      return
    }

    if (last.cmd === 'wake_confirm' && last.ok) {
      setWakePhase('success')
      toast.success('Wake packet sent to Desktop-01')
      return
    }

    if (last.cmd === 'wake_confirm' && !last.ok) {
      setWakePhase('confirm')
      toast.error(readText(last.message) || 'Wake confirm failed')
      return
    }

    if (last.cmd === 'ping') {
      const latency = readNumber(last.latencyMs) ?? readNumber(last.rtt_ms)
      if (last.ok) {
        toast.success(latency ? `ESP32 replied in ${latency} ms` : 'ESP32 replied')
      } else {
        toast.error(readText(last.message) || 'Ping failed')
      }
      return
    }

    if (last.cmd === 'reboot_request' && !last.ok) {
      toast.error(readText(last.message) || 'Reboot request failed')
      setRebootOpen(false)
      return
    }

    if (last.cmd === 'reboot_confirm' && last.ok) {
      toast.success('ESP32 rebooting')
      setRebootOpen(false)
      return
    }
  }, [replies])

  // Auto-close wake dialog after success
  useEffect(() => {
    if (wakePhase !== 'success') return
    const t = setTimeout(() => {
      setWakeOpen(false)
      setWakePhase('initial')
      setWakeToken(null)
      setWakeExpiresAt(null)
    }, 2000)
    return () => clearTimeout(t)
  }, [wakePhase])

  // Expire wake confirmation
  useEffect(() => {
    if (wakePhase !== 'confirm' || !wakeExpiresAt) return
    if (now < wakeExpiresAt) return
    setWakePhase('initial')
    setWakeToken(null)
    setWakeExpiresAt(null)
    toast.error('Wake confirmation expired')
  }, [now, wakePhase, wakeExpiresAt])

  // Merge MQTT state with static defaults
  const device: DeviceData = useMemo(() => {
    const s = state
    return {
      name: PC_NAME,
      controllerLabel: 'ESP32 Controller',
      online: s?.online ?? STATIC_DEVICE.online,
      ready: Boolean(s?.online && s?.mqtt_connected),
      lastUpdateAgo: s?.ts ? formatAgo(s.ts) : STATIC_DEVICE.lastUpdateAgo,
      rssi: typeof s?.rssi === 'number' ? s.rssi : STATIC_DEVICE.rssi,
      ipAddress: s?.ip || STATIC_DEVICE.ipAddress,
      uptime: typeof s?.uptime_s === 'number' ? formatDuration(s.uptime_s) : STATIC_DEVICE.uptime,
      freeHeap: typeof s?.heap === 'number' ? `${Math.round(s.heap / 1024)} KB` : STATIC_DEVICE.freeHeap,
      cpuTemp: STATIC_DEVICE.cpuTemp,
      mqttStatus: s?.mqtt_connected ? 'Connected' : 'Disconnected',
      grafanaStatus: STATIC_DEVICE.grafanaStatus,
      broker: BROKER_URL ? formatBroker(BROKER_URL) : STATIC_DEVICE.broker,
      signalQuality: typeof s?.rssi === 'number' ? signalQuality(s.rssi) : STATIC_DEVICE.signalQuality,
      lastWake: s?.last_wake_at ? formatAgo(s.last_wake_at) : STATIC_DEVICE.lastWake,
      lastWakeStatus: s?.last_wake_result || STATIC_DEVICE.lastWakeStatus,
    }
  }, [state])

  const connected = connection.connected

  const startWake = () => {
    setWakePhase('initial')
    setWakeOpen(true)
  }

  const requestWake = () => {
    setWakePhase('waiting')
    send('wake_request', { target: PC_NAME, expires_in_s: 30 })
  }

  const confirmWake = () => {
    if (!wakeToken) return
    send('wake_confirm', { confirm_token: wakeToken, target: PC_NAME })
    setWakePhase('waiting')
  }

  const cancelWake = () => {
    send('wake_cancel')
    setWakeOpen(false)
    setWakePhase('initial')
    setWakeToken(null)
    setWakeExpiresAt(null)
  }

  const startReboot = () => {
    setRebootOpen(true)
  }

  const confirmReboot = () => {
    send('reboot_request', { expires_in_s: 30 })
  }

  const handlePing = () => {
    send('ping')
  }

  const secsLeft = wakeExpiresAt ? Math.max(0, Math.ceil((wakeExpiresAt - now) / 1000)) : 0

  return (
    <TooltipProvider>
      <SidebarProvider defaultOpen style={{ '--sidebar-width': '244px' } as React.CSSProperties}>
        <Sidebar collapsible="icon" className="border-r border-sidebar-border">
          <SidebarHeader className="p-4 pt-6">
            <div className="flex items-center gap-3">
              <Monitor className="size-10 text-foreground" weight="light" />
              <div className="grid gap-0.5">
                <span className="text-sm font-semibold leading-none text-sidebar-foreground">PC Remote</span>
                <span className="text-xs text-sidebar-foreground/60">Control Panel</span>
              </div>
            </div>
          </SidebarHeader>

          <SidebarContent>
            <SidebarMenu>
              <NavItem icon={Layout} label="Dashboard" active />
              <NavItem icon={Bell} label="Events" />
              <NavItem icon={FileText} label="Logs" />
              <NavItem icon={Gear} label="Settings" />
              <NavItem icon={Question} label="About" />
            </SidebarMenu>
          </SidebarContent>

          <SidebarFooter className="p-0 pb-4">
            <div className="sidebar-footer-card">
              <Card size="sm">
                <CardHeader className="flex flex-row items-center gap-2">
                  <WifiHigh className="size-4 text-sidebar-foreground/70" />
                  <CardTitle className="text-xs text-sidebar-foreground/70">Connection</CardTitle>
                </CardHeader>
                <CardContent className="grid gap-1.5">
                  <div className="flex items-center justify-between text-xs">
                    <span className="text-sidebar-foreground/60">Online</span>
                    <Badge variant="secondary" className="text-[10px] leading-none">
                      {connection.connected ? 'Connected' : 'Offline'}
                    </Badge>
                  </div>
                  <div className="flex items-center justify-between text-xs">
                    <span className="text-sidebar-foreground/60">MQTT Broker</span>
                    <span className="text-sidebar-foreground font-medium">{device.broker}</span>
                  </div>
                  <div className="flex items-center justify-between text-xs">
                    <span className="text-sidebar-foreground/60">Last Update</span>
                    <span className="text-sidebar-foreground font-medium">{device.lastUpdateAgo}</span>
                  </div>
                </CardContent>
              </Card>
            </div>
          </SidebarFooter>
        </Sidebar>

        {/* Main content */}
        <div className="flex flex-1 flex-col h-dvh overflow-hidden">
          {/* Desktop & Mobile header */}
          <header className="flex items-center justify-between px-5 py-4 shrink-0 min-h-[86px] border-b border-border">
            <div className="flex items-center gap-4">
              <SidebarTrigger className="md:hidden" />
              <div className="grid gap-0.5">
                <div className="flex items-center gap-2">
                  <h1 className="text-lg font-semibold leading-none text-foreground">{device.name}</h1>
                  <Badge variant="secondary" className="text-[10px]">
                    {device.online ? 'Online' : 'Offline'}
                  </Badge>
                </div>
                <p className="text-xs text-muted-foreground">
                  {device.controllerLabel} &middot; Last update: {device.lastUpdateAgo}
                </p>
              </div>
            </div>

            <div className="hidden md:flex items-center gap-4">
              <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
                <WifiHigh className="size-4" />
                <span>{device.rssi} dBm</span>
              </div>
              <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
                <ChartBar className="size-4" />
                <span>{device.signalQuality}%</span>
              </div>
              <div className="flex items-center gap-1.5 text-xs text-muted-foreground w-[116px] justify-end">
                <Clock className="size-4" />
                <span>{new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit' }).format(now)}</span>
              </div>
              <ThemeToggle />
            </div>
          </header>

          {/* Dashboard grid */}
          <div className="flex-1 overflow-auto p-5">
            <div className="grid grid-cols-1 lg:grid-cols-dashboard gap-[18px] auto-rows-fr h-full">
              {/* PC Control */}
              <Card className="[grid-area:pcControl]">
                <CardHeader>
                  <CardTitle>PC Control</CardTitle>
                  <CardAction>
                    <DropdownMenu>
                      <DropdownMenuTrigger>
                        <Button variant="ghost" size="icon-sm">
                          <DotsThree className="size-4" />
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
                  {/* Top section: icon + info */}
                  <div className="flex items-center gap-7" style={{ display: 'grid', gridTemplateColumns: '180px minmax(0, 1fr)', gap: 28, alignItems: 'center' }}>
                    <div className="flex items-center justify-center">
                      <Monitor className="size-[132px] text-muted-foreground/40" weight="thin" />
                    </div>
                    <div className="grid gap-2">
                      <div className="flex items-center gap-2">
                        <span className="text-base font-semibold">Desktop PC</span>
                        <Badge variant="secondary" className="text-[10px]">{device.ready ? 'Ready' : 'Unavailable'}</Badge>
                      </div>
                      <div className="grid gap-1.5">
                        <div className="flex items-center gap-2 text-xs text-muted-foreground">
                          <Clock className="size-3.5" />
                          <span className="font-medium text-foreground">Last Wake:</span>
                          <span>{device.lastWake}</span>
                        </div>
                        <div className="flex items-center gap-2 text-xs text-muted-foreground">
                          <CheckCircle className="size-3.5" />
                          <span className="font-medium text-foreground">Last Wake Status:</span>
                          <span>{device.lastWakeStatus}</span>
                        </div>
                      </div>
                    </div>
                  </div>

                  {/* Actions */}
                  <Button variant="default" size="lg" className="w-full h-14 text-sm font-bold gap-2" disabled={!connected} onClick={startWake}>
                    <Power className="size-5" />
                    WAKE PC
                  </Button>

                  <div className="grid grid-cols-2 gap-3">
                    <Button variant="outline" className="h-[50px] text-xs font-semibold gap-2" disabled={!connected} onClick={handlePing}>
                      <Pulse className="size-4" />
                      PING ESP32
                    </Button>
                    <Button variant="outline" className="h-[50px] text-xs font-semibold gap-2" disabled={!connected} onClick={startReboot}>
                      <ArrowClockwise className="size-4" />
                      REBOOT ESP32
                    </Button>
                  </div>
                </CardContent>
              </Card>

              {/* Device Status */}
              <Card className="[grid-area:deviceStatus]">
                <CardHeader>
                  <CardTitle>Device Status</CardTitle>
                </CardHeader>
                <CardContent className="grid">
                  <StatusRow icon={ComputerTower} label="Status" value={device.online ? 'Online' : 'Offline'} />
                  <Separator />
                  <StatusRow icon={WifiHigh} label="Wi-Fi RSSI" value={`${device.rssi} dBm`} />
                  <Separator />
                  <StatusRow icon={GlobeSimple} label="IP Address" value={device.ipAddress} />
                  <Separator />
                  <StatusRow icon={Clock} label="Uptime" value={device.uptime} />
                  <Separator />
                  <StatusRow icon={Memory} label="Free Heap" value={device.freeHeap} />
                  <Separator />
                  <StatusRow icon={Thermometer} label="CPU Temp" value={device.cpuTemp} />
                  <Separator />
                  <StatusRow icon={Cloud} label="MQTT" value={device.mqttStatus} />
                </CardContent>
              </Card>

              {/* Recent Events */}
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

              {/* Connection Health */}
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
            </div>
          </div>
        </div>

        {/* Mobile bottom nav */}
        <nav className="fixed bottom-0 left-0 right-0 z-40 md:hidden border-t border-border bg-background/90 backdrop-blur-sm">
          <div className="flex items-center justify-around h-[68px] px-4">
            <MobileNavItem icon={Layout} label="Dashboard" active />
            <MobileNavItem icon={Bell} label="Events" />
            <MobileNavItem icon={FileText} label="Logs" />
            <MobileNavItem icon={Gear} label="Settings" />
            <MobileThemeToggle />
          </div>
        </nav>
      </SidebarProvider>

      {/* Wake Dialog */}
      <Dialog open={wakeOpen} onOpenChange={(open) => { if (!open) cancelWake() }}>
        <DialogContent className="sm:max-w-sm" showCloseButton={false}>
          <DialogHeader>
            <DialogTitle>
              {wakePhase === 'initial' && 'Wake Desktop PC?'}
              {wakePhase === 'waiting' && 'Waiting for controller'}
              {wakePhase === 'confirm' && 'Confirm wake'}
              {wakePhase === 'success' && 'Wake packet sent'}
            </DialogTitle>
            <DialogDescription>
              {wakePhase === 'initial' && 'Request confirmation from the ESP32 before sending the Wake-on-LAN packet.'}
              {wakePhase === 'waiting' && 'Requesting a temporary confirmation token from Desktop-01.'}
              {wakePhase === 'confirm' && `The request expires in ${secsLeft} seconds.`}
              {wakePhase === 'success' && 'Desktop-01 sent the Wake-on-LAN packet.'}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            {wakePhase === 'initial' && (
              <>
                <Button variant="outline" onClick={cancelWake}>Cancel</Button>
                <Button variant="default" onClick={requestWake}>Continue</Button>
              </>
            )}
            {wakePhase === 'waiting' && (
              <Button variant="outline" disabled>Waiting…</Button>
            )}
            {wakePhase === 'confirm' && (
              <>
                <Button variant="outline" onClick={cancelWake}>Cancel</Button>
                <Button variant="default" onClick={confirmWake}>Wake PC</Button>
              </>
            )}
            {wakePhase === 'success' && (
              <span className="text-xs text-muted-foreground">Closing…</span>
            )}
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Reboot AlertDialog */}
      <AlertDialog open={rebootOpen} onOpenChange={setRebootOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Reboot ESP32?</AlertDialogTitle>
            <AlertDialogDescription>
              The controller will disconnect briefly and reconnect automatically.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction onClick={confirmReboot}>Reboot</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <Toaster position="bottom-right" />
    </TooltipProvider>
  )
}

/* ─── Sub-components ─── */

function ThemeToggle() {
  const { theme, setTheme } = useTheme()
  const next = theme === 'dark' ? 'light' : 'dark'
  return (
    <button
      onClick={() => setTheme(next)}
      className="flex items-center justify-center size-8 rounded-none text-muted-foreground hover:text-foreground transition-colors"
      aria-label={`Switch to ${next} theme`}
    >
      {theme === 'dark' ? <Sun className="size-4" /> : <Moon className="size-4" />}
    </button>
  )
}

function MobileThemeToggle() {
  const { theme, setTheme } = useTheme()
  const next = theme === 'dark' ? 'light' : 'dark'
  return (
    <button
      onClick={() => setTheme(next)}
      className="flex flex-col items-center gap-1 py-2 px-3 text-[10px] font-medium text-muted-foreground transition-colors"
      aria-label={`Switch to ${next} theme`}
    >
      {theme === 'dark' ? <Sun className="size-5" /> : <Moon className="size-5" />}
    </button>
  )
}

function NavItem({ icon: Icon, label, active }: { icon: React.ComponentType<{ className?: string }>; label: string; active?: boolean }) {
  return (
    <SidebarMenuItem>
      <SidebarMenuButton isActive={active} tooltip={label}>
        <Icon className="size-4" />
        <span>{label}</span>
      </SidebarMenuButton>
    </SidebarMenuItem>
  )
}

function MobileNavItem({ icon: Icon, label, active, onClick }: { icon: React.ComponentType<{ className?: string }>; label: string; active?: boolean; onClick?: () => void }) {
  return (
    <button className={`flex flex-col items-center gap-1 py-2 px-3 text-[10px] font-medium transition-colors ${active ? 'text-foreground' : 'text-muted-foreground'}`} onClick={onClick}>
      <Icon className={`size-5 ${active ? 'text-foreground' : 'text-muted-foreground'}`} />
      {label && <span>{label}</span>}
    </button>
  )
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

function HealthRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between py-[12.5px] text-xs">
      <span className="text-muted-foreground">{label}</span>
      <span className="font-medium text-foreground">{value}</span>
    </div>
  )
}

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

/* ─── Helpers ─── */

function readText(value: unknown): string | null {
  return typeof value === 'string' && value.trim() ? value : null
}

function readNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null
}

function toMillis(value?: number | null): number | null {
  if (typeof value !== 'number' || !Number.isFinite(value)) return null
  return value > 1_000_000_000_000 ? value : value * 1000
}

function resolveExpiresAt(reply: Record<string, unknown>, fallbackSeconds: number): number {
  const absolute = toMillis(reply.expires_at as number | undefined) ?? toMillis(reply.wake_expires_at as number | undefined)
  if (absolute) return absolute
  if (typeof reply.expires_in_s === 'number' && Number.isFinite(reply.expires_in_s)) {
    return Date.now() + reply.expires_in_s * 1000
  }
  return Date.now() + fallbackSeconds * 1000
}

function formatDuration(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds))
  const hours = Math.floor(total / 3600)
  const minutes = Math.floor((total % 3600) / 60)
  const secs = total % 60
  const parts = [
    hours ? `${hours}h` : null,
    hours || minutes ? `${minutes}m` : null,
    `${secs}s`,
  ].filter(Boolean)
  return parts.join(' ')
}

function formatAgo(value: number): string {
  const ms = toMillis(value)
  if (!ms) return '—'
  const diff = Date.now() - ms
  const abs = Math.abs(diff)
  const suffix = diff >= 0 ? 'ago' : 'from now'
  if (abs < 60_000) return `${Math.max(1, Math.round(abs / 1000))}s ${suffix}`
  if (abs < 3_600_000) return `${Math.max(1, Math.round(abs / 60_000))}m ${suffix}`
  if (abs < 86_400_000) return `${Math.max(1, Math.round(abs / 3_600_000))}h ${suffix}`
  return `${Math.max(1, Math.round(abs / 86_400_000))}d ${suffix}`
}

function formatBroker(url: string): string {
  try {
    return new URL(url).host
  } catch {
    return url
  }
}

function signalQuality(rssi: number): number {
  return Math.max(0, Math.min(100, Math.round(((rssi + 90) / 50) * 100)))
}

export default AppWrapper

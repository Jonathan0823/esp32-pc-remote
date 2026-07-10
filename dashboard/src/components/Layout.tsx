import { useEffect, useState, useMemo } from 'react'
import { Outlet, useLocation, useNavigate, useOutletContext } from 'react-router-dom'
import { useMqtt } from '@/mqtt/useMqtt'
import { SidebarProvider, Sidebar, SidebarHeader, SidebarContent, SidebarFooter, SidebarMenu, SidebarMenuItem, SidebarMenuButton, SidebarTrigger } from '@/components/ui/sidebar'
import type { Theme } from '@/components/ui/theme-provider'
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { useTheme } from '@/components/ui/theme-provider'
import type { DeviceData } from '@/lib/types'
import { PC_NAME, BROKER_URL } from '@/lib/types'
import type { EspState } from '@/mqtt/types'
import { formatAgo, formatDuration, formatBroker, signalQuality } from '@/lib/helpers'
import {
  MonitorIcon, LayoutIcon, BellIcon, FileTextIcon, GearIcon, QuestionIcon,
  WifiHighIcon, ChartBarIcon, ClockIcon, SunIcon, MoonIcon,
} from '@phosphor-icons/react'

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
  mqttStatus: 'Connected',
  broker: 'HiveMQ Cloud',
  signalQuality: 82,
  lastWake: 'Today 15:44',
  lastWakeStatus: 'WOL packet sent',
}

export interface LayoutContext {
  device: DeviceData
  state: EspState | null
  connected: boolean
  connection: ReturnType<typeof useMqtt>['connection']
  send: (cmd: string, payload?: Record<string, unknown>) => void
  replies: ReturnType<typeof useMqtt>['replies']
  logs: string[]
}

export function useLayoutContext(): LayoutContext {
  const ctx = useOutletContext<LayoutContext>()
  if (!ctx) throw new Error('useLayoutContext must be used inside a Layout route')
  return ctx
}

export default function Layout() {
  const { connection, state, replies, logs, send } = useMqtt()

  const { theme, setTheme } = useTheme()
  const location = useLocation()
  const navigate = useNavigate()



  const device: DeviceData = useMemo(() => {
    const s = state
    const espOnline = s?.online === true
    return {
      name: PC_NAME,
      controllerLabel: 'ESP32 Controller',
      online: espOnline,
      ready: espOnline && s?.pc_online === true,
      lastUpdateAgo: s?.ts ? formatAgo(s.ts) : STATIC_DEVICE.lastUpdateAgo,
      rssi: typeof s?.rssi === 'number' ? s.rssi : STATIC_DEVICE.rssi,
      ipAddress: s?.ip || STATIC_DEVICE.ipAddress,
      uptime: typeof s?.uptime_s === 'number' ? formatDuration(s.uptime_s) : STATIC_DEVICE.uptime,
      freeHeap: typeof s?.heap === 'number' ? `${Math.round(s.heap / 1024)} KB` : STATIC_DEVICE.freeHeap,
      mqttStatus: espOnline ? 'Connected' : 'Disconnected',
      broker: BROKER_URL ? formatBroker(BROKER_URL) : STATIC_DEVICE.broker,
      signalQuality: typeof s?.rssi === 'number' ? signalQuality(s.rssi) : STATIC_DEVICE.signalQuality,
      lastWake: s?.last_wake_at ? formatAgo(s.last_wake_at) : STATIC_DEVICE.lastWake,
      lastWakeStatus: s?.last_wake_result || STATIC_DEVICE.lastWakeStatus,
    }
  }, [state])

  const connected = connection.connected
  const active = location.pathname

  function navTo(path: string) {
    navigate(path)
  }

  return (
    <SidebarProvider defaultOpen style={{ '--sidebar-width': '244px' } as React.CSSProperties}>
      <Sidebar collapsible="icon" className="border-r border-sidebar-border">
        <SidebarHeader className="p-4 pt-6">
          <div className="flex items-center gap-3">
            <MonitorIcon className="size-10 text-foreground" weight="light" />
            <div className="grid gap-0.5">
              <span className="text-sm font-semibold leading-none text-sidebar-foreground">PC Remote</span>
              <span className="text-xs text-sidebar-foreground/60">Control Panel</span>
            </div>
          </div>
        </SidebarHeader>

        <SidebarContent>
          <SidebarMenu>
            <NavItem icon={LayoutIcon} label="Dashboard" active={active === '/'} onClick={() => navTo('/')} />
            <NavItem icon={BellIcon} label="Events" active={active === '/events'} onClick={() => navTo('/events')} />
            <NavItem icon={FileTextIcon} label="Logs" active={active === '/logs'} onClick={() => navTo('/logs')} />
            <NavItem icon={GearIcon} label="Settings" active={active === '/settings'} onClick={() => navTo('/settings')} />
            <NavItem icon={QuestionIcon} label="About" active={active === '/about'} onClick={() => navTo('/about')} />
          </SidebarMenu>
        </SidebarContent>

        <SidebarFooter className="p-0 pb-4">
          <div className="sidebar-footer-card">
            <Card size="sm">
              <CardHeader className="flex flex-row items-center gap-2">
                <WifiHighIcon className="size-4 text-sidebar-foreground/70" />
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
              <WifiHighIcon className="size-4" />
              <span>{device.rssi} dBm</span>
            </div>
            <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
              <ChartBarIcon className="size-4" />
              <span>{device.signalQuality}%</span>
            </div>
            <div className="flex items-center gap-1.5 text-xs text-muted-foreground w-[116px] justify-end">
              <ClockIcon className="size-4" />
              <ClockTimer />
            </div>
            <ThemeToggle theme={theme} setTheme={setTheme} />
          </div>
        </header>

        {/* Page content via router outlet */}
        <div className="flex-1 overflow-auto p-5">
          <Outlet context={{ device, state, connected, connection, send, replies, logs } satisfies LayoutContext} />
        </div>
      </div>

      {/* Mobile bottom nav */}
      <nav className="fixed bottom-0 left-0 right-0 z-40 md:hidden border-t border-border bg-background/90 backdrop-blur-sm">
        <div className="flex items-center justify-around h-[68px] px-4">
          <MobileNavItem icon={LayoutIcon} label="Dashboard" active={active === '/'} onClick={() => navTo('/')} />
          <MobileNavItem icon={BellIcon} label="Events" active={active === '/events'} onClick={() => navTo('/events')} />
          <MobileNavItem icon={FileTextIcon} label="Logs" active={active === '/logs'} onClick={() => navTo('/logs')} />
          <MobileNavItem icon={GearIcon} label="Settings" active={active === '/settings'} onClick={() => navTo('/settings')} />
          <MobileThemeToggle theme={theme} setTheme={setTheme} />
        </div>
      </nav>
    </SidebarProvider>
  )
}

/* ─── Sub-components ─── */

function ClockTimer() {
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 1000)
    return () => clearInterval(t)
  }, [])
  return <span>{new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit' }).format(now)}</span>
}

function ThemeToggle({ theme, setTheme }: { theme: Theme; setTheme: (t: Theme) => void }) {
  const next = theme === 'dark' ? 'light' : 'dark'
  return (
    <button
      onClick={() => setTheme(next)}
      className="flex items-center justify-center size-8 rounded-none text-muted-foreground hover:text-foreground transition-colors"
      aria-label={`Switch to ${next} theme`}
    >
      {theme === 'dark' ? <SunIcon className="size-4" /> : <MoonIcon className="size-4" />}
    </button>
  )
}

function MobileThemeToggle({ theme, setTheme }: { theme: Theme; setTheme: (t: Theme) => void }) {
  const next = theme === 'dark' ? 'light' : 'dark'
  return (
    <button
      onClick={() => setTheme(next)}
      className="flex flex-col items-center gap-1 py-2 px-3 text-[10px] font-medium text-muted-foreground transition-colors"
      aria-label={`Switch to ${next} theme`}
    >
      {theme === 'dark' ? <SunIcon className="size-5" /> : <MoonIcon className="size-5" />}
    </button>
  )
}

function NavItem({ icon: Icon, label, active, onClick }: { icon: React.ComponentType<{ className?: string }>; label: string; active?: boolean; onClick?: () => void }) {
  return (
    <SidebarMenuItem>
      <SidebarMenuButton isActive={active} tooltip={label} onClick={onClick}>
        <Icon className="size-4" />
        <span>{label}</span>
      </SidebarMenuButton>
    </SidebarMenuItem>
  )
}

function MobileNavItem({ icon: Icon, label, active, onClick }: { icon: React.ComponentType<{ className?: string }>; label: string; active?: boolean; onClick?: () => void }) {
  return (
    <button
      className={`flex flex-col items-center gap-1 py-2 px-3 text-[10px] font-medium transition-colors ${active ? 'text-foreground' : 'text-muted-foreground'}`}
      onClick={onClick}
    >
      <Icon className={`size-5 ${active ? 'text-foreground' : 'text-muted-foreground'}`} />
      <span>{label}</span>
    </button>
  )
}

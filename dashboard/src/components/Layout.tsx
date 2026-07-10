import { useEffect, useState, useMemo, useRef } from 'react'
import { Outlet, useLocation, useNavigate, useOutletContext } from 'react-router-dom'
import { useMqtt } from '@/mqtt/useMqtt'
import {
  SidebarProvider,
  Sidebar,
  SidebarHeader,
  SidebarContent,
  SidebarFooter,
  SidebarMenu,
  SidebarMenuItem,
  SidebarMenuButton,
  SidebarTrigger,
} from '@/components/ui/sidebar'
import type { Theme } from '@/components/ui/theme-provider'
import { Badge } from '@/components/ui/badge'
import { useTheme } from '@/components/ui/theme-provider'
import type { DeviceData } from '@/lib/types'
import { PC_NAME, BROKER_URL } from '@/lib/types'
import type { EspState } from '@/mqtt/types'
import { formatAgo, formatDuration, formatBroker, signalQuality } from '@/lib/helpers'
import {
  MonitorIcon,
  LayoutIcon,
  BellIcon,
  FileTextIcon,
  GearIcon,
  QuestionIcon,
  WifiHighIcon,
  ChartBarIcon,
  ClockIcon,
  SunIcon,
  MoonIcon,
  GithubLogoIcon,
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

  // Wall clock receipt time for last update / wake
  const lastUpdateWall = useRef(Date.now())
  const bootOffsetMs = useRef<number | null>(null)
  useEffect(() => {
    if (!state) return
    lastUpdateWall.current = Date.now()
    if (bootOffsetMs.current === null && typeof state.ts === 'number') {
      bootOffsetMs.current = Date.now() - state.ts * 1000
    }
  }, [state])

  const device: DeviceData = useMemo(() => {
    const s = state
    const espOnline = s?.online === true

    // Convert firmware relative timestamps (seconds since boot) to wall clock
    let lastWakeFormatted = STATIC_DEVICE.lastWake
    if (bootOffsetMs.current !== null && s?.last_wake_at) {
      const absMs = bootOffsetMs.current + s.last_wake_at * 1000
      lastWakeFormatted = formatAgo(absMs)
    }

    return {
      name: PC_NAME,
      controllerLabel: 'ESP32 Controller',
      online: espOnline,
      ready: espOnline && s?.pc_online === true,
      lastUpdateAgo: formatAgo(lastUpdateWall.current),
      rssi: typeof s?.rssi === 'number' ? s.rssi : STATIC_DEVICE.rssi,
      ipAddress: s?.ip || STATIC_DEVICE.ipAddress,
      uptime: typeof s?.uptime_s === 'number' ? formatDuration(s.uptime_s) : STATIC_DEVICE.uptime,
      freeHeap:
        typeof s?.heap === 'number' ? `${Math.round(s.heap / 1024)} KB` : STATIC_DEVICE.freeHeap,
      mqttStatus: espOnline ? 'Connected' : 'Disconnected',
      broker: BROKER_URL ? formatBroker(BROKER_URL) : STATIC_DEVICE.broker,
      signalQuality:
        typeof s?.rssi === 'number' ? signalQuality(s.rssi) : STATIC_DEVICE.signalQuality,
      lastWake: lastWakeFormatted,
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
      <Sidebar collapsible="icon" className="border-sidebar-border border-r">
        <SidebarHeader className="p-4 pt-6">
          <div className="flex items-center gap-3">
            <MonitorIcon className="text-foreground size-10" weight="light" />
            <div className="grid gap-0.5">
              <span className="text-sidebar-foreground text-sm leading-none font-semibold">
                PC Remote
              </span>
              <span className="text-sidebar-foreground/60 text-xs">Control Panel</span>
            </div>
          </div>
        </SidebarHeader>

        <SidebarContent>
          <SidebarMenu>
            <NavItem
              icon={LayoutIcon}
              label="Dashboard"
              active={active === '/'}
              onClick={() => navTo('/')}
            />
            <NavItem
              icon={BellIcon}
              label="Events"
              active={active === '/events'}
              onClick={() => navTo('/events')}
            />
            <NavItem
              icon={FileTextIcon}
              label="Logs"
              active={active === '/logs'}
              onClick={() => navTo('/logs')}
            />
            <NavItem
              icon={GearIcon}
              label="Settings"
              active={active === '/settings'}
              onClick={() => navTo('/settings')}
            />
            <NavItem
              icon={QuestionIcon}
              label="About"
              active={active === '/about'}
              onClick={() => navTo('/about')}
            />
          </SidebarMenu>
        </SidebarContent>

        <SidebarFooter className="p-4 pt-0">
          <a
            href="https://github.com/Jonathan0823/esp32-pc-remote"
            target="_blank"
            rel="noopener noreferrer"
            className="text-sidebar-foreground/30 hover:text-sidebar-foreground flex items-center gap-1.5 text-[10px] transition-colors"
          >
            <GithubLogoIcon className="size-3" />
            <span>esp32-pc-remote</span>
          </a>
        </SidebarFooter>
      </Sidebar>

      {/* Main content */}
      <div className="flex h-dvh flex-1 flex-col overflow-hidden">
        {/* Desktop & Mobile header */}
        <header className="border-border flex min-h-[86px] shrink-0 items-center justify-between border-b px-5 py-4">
          <div className="flex items-center gap-4">
            <SidebarTrigger className="md:hidden" />
            <div className="grid gap-0.5">
              <div className="flex items-center gap-2">
                <h1 className="text-foreground text-lg leading-none font-semibold">
                  {device.name}
                </h1>
                <Badge variant="secondary" className="text-[10px]">
                  {device.online ? 'Online' : 'Offline'}
                </Badge>
              </div>
              <p className="text-muted-foreground text-xs">
                {device.controllerLabel} &middot; Last update: {device.lastUpdateAgo}
              </p>
            </div>
          </div>

          <div className="md:hidden">
            <ThemeToggle theme={theme} setTheme={setTheme} />
          </div>

          <div className="hidden items-center gap-4 md:flex">
            <div className="text-muted-foreground flex items-center gap-1.5 text-xs">
              <WifiHighIcon className="size-4" />
              <span>{device.rssi} dBm</span>
            </div>
            <div className="text-muted-foreground flex items-center gap-1.5 text-xs">
              <ChartBarIcon className="size-4" />
              <span>{device.signalQuality}%</span>
            </div>
            <div className="text-muted-foreground flex w-[116px] items-center justify-end gap-1.5 text-xs">
              <ClockIcon className="size-4" />
              <ClockTimer />
            </div>
            <ThemeToggle theme={theme} setTheme={setTheme} />
          </div>
        </header>

        {/* Page content via router outlet */}
        <div className="flex-1 overflow-auto p-5">
          <Outlet
            context={
              {
                device,
                state,
                connected,
                connection,
                send,
                replies,
                logs,
              } satisfies LayoutContext
            }
          />
        </div>
      </div>

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
  return (
    <span>
      {new Intl.DateTimeFormat(undefined, {
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
      }).format(now)}
    </span>
  )
}

function ThemeToggle({ theme, setTheme }: { theme: Theme; setTheme: (t: Theme) => void }) {
  const next = theme === 'dark' ? 'light' : 'dark'
  return (
    <button
      onClick={() => setTheme(next)}
      className="text-muted-foreground hover:text-foreground flex size-8 items-center justify-center rounded-none transition-colors"
      aria-label={`Switch to ${next} theme`}
    >
      {theme === 'dark' ? <SunIcon className="size-4" /> : <MoonIcon className="size-4" />}
    </button>
  )
}

function NavItem({
  icon: Icon,
  label,
  active,
  onClick,
}: {
  icon: React.ComponentType<{ className?: string }>
  label: string
  active?: boolean
  onClick?: () => void
}) {
  return (
    <SidebarMenuItem>
      <SidebarMenuButton isActive={active} tooltip={label} onClick={onClick}>
        <Icon className="size-4" />
        <span>{label}</span>
      </SidebarMenuButton>
    </SidebarMenuItem>
  )
}


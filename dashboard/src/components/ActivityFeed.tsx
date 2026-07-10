import { type ReactNode, useMemo, useState } from 'react'
import { toast } from 'sonner'
import { useLayoutContext } from '@/components/Layout'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { Input } from '@/components/ui/input'
import { ScrollArea } from '@/components/ui/scroll-area'
import { cn } from '@/lib/utils'
import { readText } from '@/lib/helpers'
import type { CommandReply, EspEvent } from '@/mqtt/types'
import {
  CaretRightIcon,
  CheckCircleIcon,
  DotsThreeIcon,
  MonitorIcon,
  PulseIcon,
  TerminalIcon,
  XCircleIcon,
} from '@phosphor-icons/react'

type ActivityTab = 'replies' | 'events' | 'logs'
type ReplyStatusFilter = 'all' | 'ok' | 'fail'

export type ActivityFeedProps = {
  compact?: boolean
}

const TAB_ORDER: ActivityTab[] = ['replies', 'events', 'logs']

function formatTimeLabel(ts: number): string {
  return new Date(ts * 1000).toLocaleTimeString()
}

function stringify(value: unknown): string {
  if (value == null) return ''
  if (typeof value === 'string') return value
  if (typeof value === 'number' || typeof value === 'boolean') return String(value)
  try {
    return JSON.stringify(value)
  } catch {
    return String(value)
  }
}

function normalizeQuery(value: string): string {
  return value.trim().toLowerCase()
}

function formatStatusValue(value: unknown): string {
  const text = stringify(value)
  return text || '—'
}

function formatReplySummary(reply: CommandReply): string {
  const parts = [reply.cmd, reply.ok ? 'ok' : 'failed']
  const status = readText(reply.status)
  const target = readText(reply.target)
  const result = readText(reply.result)
  if (status) parts.push(`status: ${status}`)
  if (target) parts.push(`target: ${target}`)
  if (result) parts.push(`result: ${result}`)
  return parts.join(' · ')
}

function formatReplyCopySummary(reply: CommandReply): string {
  const lines = [
    `cmd: ${reply.cmd}`,
    `ok: ${reply.ok}`,
    `id: ${reply.id}`,
    `ts: ${reply.ts}s`,
  ]

  const fields: Array<[string, unknown]> = [
    ['status', reply.status],
    ['target', reply.target],
    ['confirm_token', reply.confirm_token],
    ['expires_at', reply.expires_at],
    ['expires_in_s', reply.expires_in_s],
    ['result', reply.result],
    ['forced', reply.forced],
    ['online', reply.online],
    ['ip', reply.ip],
    ['rssi', reply.rssi],
    ['heap', reply.heap],
    ['uptime_s', reply.uptime_s],
    ['reset_reason', reply.reset_reason],
    ['received_cmd', reply.received_cmd],
    ['latencyMs', reply.latencyMs],
    ['rtt_ms', reply.rtt_ms],
    ['message', reply.message],
  ]

  for (const [key, value] of fields) {
    const text = stringify(value)
    if (text) lines.push(`${key}: ${text}`)
  }

  return lines.join('\n')
}

function formatEventSummary(event: EspEvent): string {
  const target = readText(event.target)
  return target ? `${event.event} · target: ${target}` : event.event
}

function formatEventCopySummary(event: EspEvent): string {
  const lines = [`event: ${event.event}`, `ts: ${event.ts}s`]
  const target = readText(event.target)
  if (target) lines.push(`target: ${target}`)
  return lines.join('\n')
}

function copyText(text: string): Promise<boolean> {
  if (!navigator.clipboard?.writeText) return Promise.resolve(false)
  return navigator.clipboard.writeText(text).then(() => true, () => false)
}

function fieldEntries(reply: CommandReply): Array<[string, unknown]> {
  const core = ['id', 'cmd', 'ok', 'ts']
  const extras = Object.keys(reply).filter((key) => !core.includes(key)).sort()
  return [...core, ...extras].map((key) => [key, reply[key]])
}

function replySearchText(reply: CommandReply): string {
  return Object.entries(reply)
    .map(([key, value]) => `${key} ${stringify(value)}`)
    .join(' ')
}

function eventSearchText(event: EspEvent): string {
  return Object.entries(event)
    .map(([key, value]) => `${key} ${stringify(value)}`)
    .join(' ')
}

function logSearchText(line: string): string {
  return line
}

function matchesQuery(haystack: string, query: string): boolean {
  const needle = normalizeQuery(query)
  if (!needle) return true
  return normalizeQuery(haystack).includes(needle)
}

function TabButton({
  active,
  onClick,
  children,
}: {
  active: boolean
  onClick: () => void
  children: ReactNode
}) {
  return (
    <Button
      variant={active ? 'default' : 'ghost'}
      size="sm"
      onClick={onClick}
      className="h-7 px-2 text-[10px]"
    >
      {children}
    </Button>
  )
}

function FilterButton({
  active,
  onClick,
  children,
}: {
  active: boolean
  onClick: () => void
  children: ReactNode
}) {
  return (
    <Button
      variant={active ? 'default' : 'outline'}
      size="sm"
      onClick={onClick}
      className="h-7 px-2 text-[10px]"
    >
      {children}
    </Button>
  )
}

function CopyMenu({
  summary,
  json,
  label,
}: {
  summary: string
  json: string
  label: string
}) {
  const onCopySummary = async () => {
    const ok = await copyText(summary)
    toast[ok ? 'success' : 'error'](ok ? `${label} summary copied` : `Copy failed`)
  }

  const onCopyJson = async () => {
    const ok = await copyText(json)
    toast[ok ? 'success' : 'error'](ok ? `${label} JSON copied` : `Copy failed`)
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        aria-label={`Copy ${label}`}
        className="inline-flex size-7 items-center justify-center rounded-none border border-transparent text-foreground hover:bg-muted focus-visible:border-ring focus-visible:ring-1 focus-visible:ring-ring/50"
      >
        <DotsThreeIcon className="size-4" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-44">
        <DropdownMenuItem onClick={onCopySummary}>Copy summary</DropdownMenuItem>
        <DropdownMenuItem onClick={onCopyJson}>Copy JSON</DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

function ReplyDetails({ reply }: { reply: CommandReply }) {
  return (
    <div className="border-border/50 bg-background/40 mt-2 border p-2 dark:border-slate-800 dark:bg-slate-900/60">
      <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
        {fieldEntries(reply).map(([key, value]) => (
          <div key={key} className="border-border/40 bg-background/70 border p-2 dark:border-slate-800 dark:bg-slate-950/90">
            <div className="text-muted-foreground text-[10px] uppercase tracking-wider">{key}</div>
            <div className="text-foreground mt-1 whitespace-pre-wrap break-words text-xs">
              {key === 'ts' && typeof value === 'number' ? `${value}s` : formatStatusValue(value)}
            </div>
          </div>
        ))}
      </div>
      <div className="text-muted-foreground mt-2 text-[10px]">
        Firmware-emitted extras are shown above when present.
      </div>
    </div>
  )
}

function ReplyRow({
  reply,
  expanded,
  onToggle,
  detail,
}: {
  reply: CommandReply
  expanded?: boolean
  onToggle?: () => void
  detail: boolean
}) {
  const summary = formatReplySummary(reply)
  const json = JSON.stringify(reply, null, 2)
  const result = readText(reply.message) ?? (reply.ok ? 'ok' : 'failed')

  if (!detail) {
    return (
      <div className="border-border/30 grid grid-cols-[auto_1fr] gap-x-2 gap-y-1 border-b py-1.5 text-xs last:border-0 dark:border-slate-800">
        <div className="col-span-2 flex items-center gap-2">
          {reply.ok ? (
            <CheckCircleIcon className="size-3 shrink-0 text-green-500" />
          ) : (
            <XCircleIcon className="size-3 shrink-0 text-red-500" />
          )}
          <span className="text-foreground font-medium">{reply.cmd}</span>
          <span className="text-muted-foreground ml-auto tabular-nums">
            {formatTimeLabel(reply.ts)}
          </span>
        </div>
        <span className="text-muted-foreground text-right">Result:</span>
        <span className={reply.ok ? 'text-green-500' : 'text-red-500'}>{result}</span>
      </div>
    )
  }

  return (
    <div className="border-border/30 border-b py-2 last:border-0 dark:border-slate-800">
      <div className="flex items-start gap-2">
        {reply.ok ? (
          <CheckCircleIcon className="mt-0.5 size-3.5 shrink-0 text-green-500" />
        ) : (
          <XCircleIcon className="mt-0.5 size-3.5 shrink-0 text-red-500" />
        )}
        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 flex-wrap items-center gap-2">
            <span className="text-foreground truncate font-medium">{reply.cmd}</span>
            <Badge variant={reply.ok ? 'secondary' : 'destructive'} className="text-[10px]">
              {reply.ok ? 'ok' : 'failed'}
            </Badge>
            {readText(reply.status) ? (
              <Badge variant="outline" className="text-[10px]">
                {readText(reply.status)}
              </Badge>
            ) : null}
            <span className="text-muted-foreground ml-auto tabular-nums text-[10px]">
              {formatTimeLabel(reply.ts)}
            </span>
          </div>
          <div className="text-muted-foreground mt-1 text-[11px]">{summary}</div>
        </div>
        <div className="flex items-center gap-1">
          <CopyMenu summary={formatReplyCopySummary(reply)} json={json} label="reply" />
          {onToggle ? (
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label={expanded ? 'Collapse reply details' : 'Expand reply details'}
              onClick={onToggle}
            >
              <CaretRightIcon className={cn('size-4 transition-transform', expanded && 'rotate-90')} />
            </Button>
          ) : null}
        </div>
      </div>
      {expanded ? <ReplyDetails reply={reply} /> : null}
    </div>
  )
}

function EventRow({ event }: { event: EspEvent }) {
  const summary = formatEventSummary(event)
  const json = JSON.stringify(event, null, 2)

  return (
    <div className="border-border/30 flex items-start gap-2 border-b py-2 text-xs last:border-0 dark:border-slate-800">
      <PulseIcon className="mt-0.5 size-3.5 shrink-0 text-cyan-500" />
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-foreground font-medium">{event.event}</span>
          {readText(event.target) ? (
            <Badge variant="secondary" className="text-[10px]">
              {readText(event.target)}
            </Badge>
          ) : null}
          <span className="text-muted-foreground ml-auto tabular-nums text-[10px]">
            {formatTimeLabel(event.ts)}
          </span>
        </div>
        <div className="text-muted-foreground mt-1 text-[11px]">{summary}</div>
      </div>
      <CopyMenu summary={formatEventCopySummary(event)} json={json} label="event" />
    </div>
  )
}

function LogRow({ line }: { line: string }) {
  return (
    <div className="border-border/20 flex items-start gap-2 border-b py-1.5 text-xs last:border-0 dark:border-slate-800">
      <TerminalIcon className="mt-0.5 size-3.5 shrink-0 text-muted-foreground" />
      <div className="text-muted-foreground min-w-0 flex-1 font-mono whitespace-pre-wrap break-words dark:text-slate-300">
        {line}
      </div>
      <DropdownMenu>
        <DropdownMenuTrigger
          aria-label="Copy log line"
          className="inline-flex size-7 items-center justify-center rounded-none border border-transparent text-foreground hover:bg-muted focus-visible:border-ring focus-visible:ring-1 focus-visible:ring-ring/50"
        >
          <DotsThreeIcon className="size-4" />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-44">
          <DropdownMenuItem
            onClick={async () => {
              const ok = await copyText(line)
              toast[ok ? 'success' : 'error'](ok ? 'Log line copied' : 'Copy failed')
            }}
          >
            Copy line
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  )
}

function EmptyState({ label }: { label: string }) {
  return <p className="text-muted-foreground py-2 text-xs">No {label} yet.</p>
}

export default function ActivityFeed({ compact = false }: ActivityFeedProps) {
  const { replies, events, logs } = useLayoutContext()
  const [tab, setTab] = useState<ActivityTab>('replies')
  const [replyQuery, setReplyQuery] = useState('')
  const [replyStatus, setReplyStatus] = useState<ReplyStatusFilter>('all')
  const [eventQuery, setEventQuery] = useState('')
  const [logQuery, setLogQuery] = useState('')
  const [expandedReplies, setExpandedReplies] = useState<Record<string, boolean>>({})

  const tabs = compact ? TAB_ORDER.filter((item) => item !== 'events') : TAB_ORDER
  const activeTab = tabs.includes(tab) ? tab : tabs[0]

  const filteredReplies = useMemo(() => {
    const query = normalizeQuery(replyQuery)
    return replies
      .slice()
      .reverse()
      .filter((reply) => {
        if (replyStatus === 'ok' && !reply.ok) return false
        if (replyStatus === 'fail' && reply.ok) return false
        if (!query) return true
        return matchesQuery(replySearchText(reply), query)
      })
  }, [replies, replyQuery, replyStatus])

  const filteredEvents = useMemo(() => {
    const query = normalizeQuery(eventQuery)
    return events
      .slice()
      .reverse()
      .filter((event) => (query ? matchesQuery(eventSearchText(event), query) : true))
  }, [events, eventQuery])

  const filteredLogs = useMemo(() => {
    const query = normalizeQuery(logQuery)
    return logs
      .slice()
      .reverse()
      .filter((line) => (query ? matchesQuery(logSearchText(line), query) : true))
  }, [logs, logQuery])

  const resetActiveFilter = () => {
    if (activeTab === 'replies') {
      setReplyQuery('')
      setReplyStatus('all')
      return
    }
    if (activeTab === 'events') {
      setEventQuery('')
      return
    }
    setLogQuery('')
  }

  const renderTabContent = () => {
    if (activeTab === 'replies') {
      return filteredReplies.length === 0 ? (
        <EmptyState label="replies" />
      ) : (
        filteredReplies.map((reply) => (
          <ReplyRow
            key={reply.id}
            reply={reply}
            detail={!compact}
            expanded={Boolean(expandedReplies[reply.id])}
            onToggle={
              compact
                ? undefined
                : () =>
                    setExpandedReplies((current) => ({
                      ...current,
                      [reply.id]: !current[reply.id],
                    }))
            }
          />
        ))
      )
    }

    if (activeTab === 'events') {
      return filteredEvents.length === 0 ? (
        <EmptyState label="events" />
      ) : (
        filteredEvents.map((event, i) => <EventRow key={`${event.ts}-${event.event}-${i}`} event={event} />)
      )
    }

    return filteredLogs.length === 0 ? (
      <EmptyState label="logs" />
    ) : (
      filteredLogs.map((line, i) => <LogRow key={`${i}-${line}`} line={line} />)
    )
  }

  if (compact) {
    return (
      <Card className="flex max-h-[360px] min-h-[240px] flex-col lg:h-full lg:max-h-full lg:min-h-0">
        <CardHeader className="pb-0">
          <div className="flex items-center justify-between gap-2">
            <CardTitle>Activity</CardTitle>
            <div className="flex gap-1">
              <TabButton active={activeTab === 'replies'} onClick={() => setTab('replies')}>
                <PulseIcon className="mr-1 size-3" />
                Replies
              </TabButton>
              <TabButton active={activeTab === 'logs'} onClick={() => setTab('logs')}>
                <TerminalIcon className="mr-1 size-3" />
                Logs
              </TabButton>
            </div>
          </div>
        </CardHeader>
        <CardContent className="flex min-h-0 flex-1 flex-col overflow-hidden p-0 pt-3 bg-zinc-100 dark:bg-zinc-950">
          <ScrollArea className="min-h-0 flex-1 overflow-auto">
            <div className="px-(--card-spacing)">{renderTabContent()}</div>
          </ScrollArea>
        </CardContent>
      </Card>
    )
  }

  return (
    <Card className="flex h-full min-h-0 flex-col">
      <CardHeader className="pb-3">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div className="space-y-1">
            <CardTitle>Activity</CardTitle>
            <p className="text-muted-foreground text-xs">
              Live MQTT replies, events, and logs from the ESP32.
            </p>
          </div>
          <div className="flex flex-wrap gap-1">
            <TabButton active={activeTab === 'replies'} onClick={() => setTab('replies')}>
              <PulseIcon className="mr-1 size-3" />
              Replies
            </TabButton>
            <TabButton active={activeTab === 'events'} onClick={() => setTab('events')}>
              <MonitorIcon className="mr-1 size-3" />
              Events
            </TabButton>
            <TabButton active={activeTab === 'logs'} onClick={() => setTab('logs')}>
              <TerminalIcon className="mr-1 size-3" />
              Logs
            </TabButton>
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Input
            value={activeTab === 'replies' ? replyQuery : activeTab === 'events' ? eventQuery : logQuery}
            onChange={(event) => {
              const value = event.target.value
              if (activeTab === 'replies') setReplyQuery(value)
              else if (activeTab === 'events') setEventQuery(value)
              else setLogQuery(value)
            }}
            placeholder={`Search ${activeTab}`}
            className="h-8 min-w-[220px] flex-1"
          />
          <Button variant="ghost" size="sm" className="h-8 px-2 text-[10px]" onClick={resetActiveFilter}>
            Clear
          </Button>
        </div>
        {activeTab === 'replies' ? (
          <div className="flex flex-wrap items-center gap-1 pt-1">
            <FilterButton active={replyStatus === 'all'} onClick={() => setReplyStatus('all')}>
              All
            </FilterButton>
            <FilterButton active={replyStatus === 'ok'} onClick={() => setReplyStatus('ok')}>
              Success
            </FilterButton>
            <FilterButton active={replyStatus === 'fail'} onClick={() => setReplyStatus('fail')}>
              Failed
            </FilterButton>
          </div>
        ) : null}
      </CardHeader>

      <CardContent className="flex min-h-0 flex-1 flex-col overflow-hidden p-0 bg-zinc-100 dark:bg-zinc-950">
        <ScrollArea className="min-h-0 flex-1 overflow-auto">
          <div className="px-(--card-spacing) py-(--card-spacing)">{renderTabContent()}</div>
        </ScrollArea>
      </CardContent>
    </Card>
  )
}

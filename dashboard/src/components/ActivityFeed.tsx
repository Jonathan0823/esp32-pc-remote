import { type ReactNode, useMemo, useState } from 'react'
import { useLayoutContext } from '@/lib/layout-context'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  normalizeQuery,
  matchesQuery,
  replySearchText,
  eventSearchText,
  logSearchText,
} from '@/lib/activity-helpers'
import { ReplyRow, EventRow, LogRow, EmptyState } from '@/components/activity/Rows'
import { PulseIcon, MonitorIcon, TerminalIcon } from '@phosphor-icons/react'

type ActivityTab = 'replies' | 'events' | 'logs'
type ReplyStatusFilter = 'all' | 'ok' | 'fail'

export type ActivityFeedProps = Readonly<{
  compact?: boolean
}>

const TAB_ORDER: ActivityTab[] = ['replies', 'events', 'logs']

function TabButton({
  active,
  onClick,
  children,
}: Readonly<{
  active: boolean
  onClick: () => void
  children: ReactNode
}>) {
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
}: Readonly<{
  active: boolean
  onClick: () => void
  children: ReactNode
}>) {
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

  const toggleReplyExpanded = (replyId: string) => {
    setExpandedReplies((current) => ({
      ...current,
      [replyId]: !current[replyId],
    }))
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
            onToggle={compact ? undefined : () => toggleReplyExpanded(reply.id)}
          />
        ))
      )
    }

    if (activeTab === 'events') {
      return filteredEvents.length === 0 ? (
        <EmptyState label="events" />
      ) : (
        filteredEvents.map((event, i) => (
          <EventRow key={`${event.ts}-${event.event}-${i}`} event={event} />
        ))
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
        <CardContent className="flex min-h-0 flex-1 flex-col overflow-hidden bg-zinc-100 p-0 pt-3 dark:bg-zinc-950">
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
            value={(() => {
              if (activeTab === 'replies') return replyQuery
              if (activeTab === 'events') return eventQuery
              return logQuery
            })()}
            onChange={(event) => {
              const value = event.target.value
              if (activeTab === 'replies') setReplyQuery(value)
              else if (activeTab === 'events') setEventQuery(value)
              else setLogQuery(value)
            }}
            placeholder={`Search ${activeTab}`}
            className="h-8 min-w-[220px] flex-1"
          />
          <Button
            variant="ghost"
            size="sm"
            className="h-8 px-2 text-[10px]"
            onClick={resetActiveFilter}
          >
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

      <CardContent className="flex min-h-0 flex-1 flex-col overflow-hidden bg-zinc-100 p-0 dark:bg-zinc-950">
        <ScrollArea className="min-h-0 flex-1 overflow-auto">
          <div className="px-(--card-spacing) py-(--card-spacing)">{renderTabContent()}</div>
        </ScrollArea>
      </CardContent>
    </Card>
  )
}

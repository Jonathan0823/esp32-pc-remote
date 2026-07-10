import { useState } from 'react'
import { useLayoutContext } from '@/components/Layout'
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import type { CommandReply } from '@/mqtt/types'
import {
  PulseIcon, CheckCircleIcon, XCircleIcon, TerminalIcon,
} from '@phosphor-icons/react'

type Tab = 'logs' | 'replies'

function formatReplyTime(ts: number): string {
  const d = new Date(ts * 1000)
  return d.toLocaleTimeString()
}

function ReplyRow({ reply }: { reply: CommandReply }) {
  return (
    <div className="flex items-center gap-2 py-1.5 text-xs border-b border-border/30 last:border-0">
      {reply.ok
        ? <CheckCircleIcon className="size-3 text-green-500 shrink-0" />
        : <XCircleIcon className="size-3 text-red-500 shrink-0" />
      }
      <span className="font-medium text-foreground min-w-[60px]">{reply.cmd}</span>
      <span className="text-muted-foreground tabular-nums shrink-0">{formatReplyTime(reply.ts)}</span>
      <span className="text-muted-foreground truncate flex-1 text-right">
        {reply.ok ? 'ok' : (typeof reply.message === 'string' ? reply.message : 'failed')}
      </span>
    </div>
  )
}

export default function LogReplies() {
  const { replies, logs } = useLayoutContext()
  const [tab, setTab] = useState<Tab>('replies')

  return (
    <Card>
      <CardHeader className="pb-0">
        <div className="flex items-center justify-between">
          <CardTitle>Activity</CardTitle>
          <div className="flex gap-1">
            <Button
              variant={tab === 'replies' ? 'default' : 'ghost'}
              size="sm"
              onClick={() => setTab('replies')}
              className="text-[10px] h-7 px-2"
            >
              <PulseIcon className="size-3 mr-1" />
              Replies
            </Button>
            <Button
              variant={tab === 'logs' ? 'default' : 'ghost'}
              size="sm"
              onClick={() => setTab('logs')}
              className="text-[10px] h-7 px-2"
            >
              <TerminalIcon className="size-3 mr-1" />
              Logs
            </Button>
          </div>
        </div>
      </CardHeader>
      <CardContent className="p-0 pt-3">
        <ScrollArea>
          <div className="px-(--card-spacing)">
            {tab === 'replies' && (
              replies.length === 0
                ? <p className="text-xs text-muted-foreground py-2">No replies yet.</p>
                : replies.map((r, i) => <ReplyRow key={r.id || i} reply={r} />)
            )}
            {tab === 'logs' && (
              logs.length === 0
                ? <p className="text-xs text-muted-foreground py-2">No logs yet.</p>
                : logs.map((line, i) => (
                    <div key={i} className="text-xs text-muted-foreground py-0.5 font-mono truncate border-b border-border/20 last:border-0">
                      {line}
                    </div>
                  ))
            )}
          </div>
        </ScrollArea>
      </CardContent>
    </Card>
  )
}

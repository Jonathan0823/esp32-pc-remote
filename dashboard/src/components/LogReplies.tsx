import { useState } from 'react'
import { useLayoutContext } from '@/components/Layout'
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import type { CommandReply } from '@/mqtt/types'
import { PulseIcon, CheckCircleIcon, XCircleIcon, TerminalIcon } from '@phosphor-icons/react'

type Tab = 'logs' | 'replies'

function formatReplyTime(ts: number): string {
  const d = new Date(ts * 1000)
  return d.toLocaleTimeString()
}

function ReplyRow({ reply }: { reply: CommandReply }) {
  return (
    <div className="border-border/30 grid grid-cols-[auto_1fr] gap-x-2 gap-y-1 border-b py-1.5 text-xs last:border-0">
      <div className="col-span-2 flex items-center gap-2">
        {reply.ok ? (
          <CheckCircleIcon className="size-3 shrink-0 text-green-500" />
        ) : (
          <XCircleIcon className="size-3 shrink-0 text-red-500" />
        )}
        <span className="text-foreground font-medium">{reply.cmd}</span>
        <span className="text-muted-foreground ml-auto tabular-nums">
          {formatReplyTime(reply.ts)}
        </span>
      </div>
      <span className="text-muted-foreground text-right">Result:</span>
      <span className={reply.ok ? 'text-green-500' : 'text-red-500'}>
        {reply.ok ? 'ok' : typeof reply.message === 'string' ? reply.message : 'failed'}
      </span>
    </div>
  )
}

export default function LogReplies() {
  const { replies, logs } = useLayoutContext()
  const [tab, setTab] = useState<Tab>('replies')

  return (
    <Card className="flex flex-col min-h-[240px] max-h-[360px] lg:min-h-0 lg:max-h-[420px]">
      <CardHeader className="pb-0">
        <div className="flex items-center justify-between">
          <CardTitle>Activity</CardTitle>
          <div className="flex gap-1">
            <Button
              variant={tab === 'replies' ? 'default' : 'ghost'}
              size="sm"
              onClick={() => setTab('replies')}
              className="h-7 px-2 text-[10px]"
            >
              <PulseIcon className="mr-1 size-3" />
              Replies
            </Button>
            <Button
              variant={tab === 'logs' ? 'default' : 'ghost'}
              size="sm"
              onClick={() => setTab('logs')}
              className="h-7 px-2 text-[10px]"
            >
              <TerminalIcon className="mr-1 size-3" />
              Logs
            </Button>
          </div>
        </div>
      </CardHeader>
      <CardContent className="min-h-0 flex-1 p-0 pt-3">
        <ScrollArea className="h-full">
          <div className="px-(--card-spacing)">
            {tab === 'replies' &&
              (replies.length === 0 ? (
                <p className="text-muted-foreground py-2 text-xs">No replies yet.</p>
              ) : (
                replies.map((r, i) => <ReplyRow key={r.id || i} reply={r} />)
              ))}
            {tab === 'logs' &&
              (logs.length === 0 ? (
                <p className="text-muted-foreground py-2 text-xs">No logs yet.</p>
              ) : (
                <div className="overflow-x-auto">
                  <div className="min-w-max">
                    {logs.map((line, i) => (
                      <div
                        key={i}
                        className="text-muted-foreground border-border/20 border-b py-0.5 font-mono text-xs whitespace-pre last:border-0"
                      >
                        {line}
                      </div>
                    ))}
                  </div>
                </div>
              ))}
          </div>
        </ScrollArea>
      </CardContent>
    </Card>
  )
}

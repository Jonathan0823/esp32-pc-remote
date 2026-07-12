import {
  CheckCircleIcon,
  XCircleIcon,
  PulseIcon,
  TerminalIcon,
  CaretRightIcon,
  DotsThreeIcon,
} from '@phosphor-icons/react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import {
  formatTimeLabel,
  formatStatusValue,
  formatReplySummary,
  formatReplyCopySummary,
  formatEventSummary,
  formatEventCopySummary,
  fieldEntries,
} from '@/lib/activity-helpers'
import { CopyMenu } from '@/components/activity/CopyMenu'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import type { CommandReply, EspEvent } from '@/mqtt/types'
import { toast } from 'sonner'

function replyResult(reply: CommandReply): string {
  if (typeof reply.message === 'string' && reply.message) return reply.message
  return reply.ok ? 'ok' : 'failed'
}

function ReplyDetails({ reply }: Readonly<{ reply: CommandReply }>) {
  return (
    <div className="border-border/50 bg-background/40 mt-2 border p-2 dark:border-slate-800 dark:bg-slate-900/60">
      <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
        {fieldEntries(reply).map(([key, value]) => (
          <div
            key={key}
            className="border-border/40 bg-background/70 border p-2 dark:border-slate-800 dark:bg-slate-950/90"
          >
            <div className="text-muted-foreground text-[10px] tracking-wider uppercase">{key}</div>
            <div className="text-foreground mt-1 text-xs break-words whitespace-pre-wrap">
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

export function ReplyRow({
  reply,
  expanded,
  onToggle,
  detail,
}: Readonly<{
  reply: CommandReply
  expanded?: boolean
  onToggle?: () => void
  detail: boolean
}>) {
  const summary = formatReplySummary(reply)
  const json = JSON.stringify(reply, null, 2)
  const result = replyResult(reply)

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
            {typeof reply.status === 'string' && reply.status ? (
              <Badge variant="outline" className="text-[10px]">
                {reply.status}
              </Badge>
            ) : null}
            <span className="text-muted-foreground ml-auto text-[10px] tabular-nums">
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
              <CaretRightIcon
                className={cn('size-4 transition-transform', expanded && 'rotate-90')}
              />
            </Button>
          ) : null}
        </div>
      </div>
      {expanded ? <ReplyDetails reply={reply} /> : null}
    </div>
  )
}

export function EventRow({ event }: Readonly<{ event: EspEvent }>) {
  const summary = formatEventSummary(event)
  const json = JSON.stringify(event, null, 2)

  return (
    <div className="border-border/30 flex items-start gap-2 border-b py-2 text-xs last:border-0 dark:border-slate-800">
      <PulseIcon className="mt-0.5 size-3.5 shrink-0 text-cyan-500" />
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-foreground font-medium">{event.event}</span>
          {typeof event.target === 'string' && event.target ? (
            <Badge variant="secondary" className="text-[10px]">
              {event.target}
            </Badge>
          ) : null}
          <span className="text-muted-foreground ml-auto text-[10px] tabular-nums">
            {formatTimeLabel(event.ts)}
          </span>
        </div>
        <div className="text-muted-foreground mt-1 text-[11px]">{summary}</div>
      </div>
      <CopyMenu summary={formatEventCopySummary(event)} json={json} label="event" />
    </div>
  )
}

export function LogRow({ line }: Readonly<{ line: string }>) {
  const onCopy = async () => {
    const ok = await navigator.clipboard?.writeText(line).then(
      () => true,
      () => false,
    )
    toast[ok ? 'success' : 'error'](ok ? 'Log line copied' : 'Copy failed')
  }

  return (
    <div className="border-border/20 flex items-start gap-2 border-b py-1.5 text-xs last:border-0 dark:border-slate-800">
      <TerminalIcon className="text-muted-foreground mt-0.5 size-3.5 shrink-0" />
      <div className="text-muted-foreground min-w-0 flex-1 font-mono break-words whitespace-pre-wrap dark:text-slate-300">
        {line}
      </div>
      <DropdownMenu>
        <DropdownMenuTrigger
          aria-label="Copy log line"
          className="text-foreground hover:bg-muted focus-visible:border-ring focus-visible:ring-ring/50 inline-flex size-7 items-center justify-center rounded-none border border-transparent focus-visible:ring-1"
        >
          <DotsThreeIcon className="size-4" />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-44">
          <DropdownMenuItem onClick={onCopy}>Copy line</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  )
}

export function EmptyState({ label }: Readonly<{ label: string }>) {
  return <p className="text-muted-foreground py-2 text-xs">No {label} yet.</p>
}

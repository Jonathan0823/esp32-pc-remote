import type { CommandReply, EspEvent } from '@/mqtt/types'

export function formatTimeLabel(ts: number): string {
  return new Date(ts * 1000).toLocaleTimeString()
}

export function stringify(value: unknown): string {
  if (value == null) return ''
  if (typeof value === 'string') return value
  if (typeof value === 'number' || typeof value === 'boolean') return String(value)
  try {
    return JSON.stringify(value)
  } catch {
    return '[unserializable]'
  }
}

export function normalizeQuery(value: string): string {
  return value.trim().toLowerCase()
}

export function formatStatusValue(value: unknown): string {
  const text = stringify(value)
  return text || '—'
}

export function formatReplySummary(reply: CommandReply): string {
  const parts = [reply.cmd, reply.ok ? 'ok' : 'failed']
  const status = stringify(reply.status)
  const target = stringify(reply.target)
  const result = stringify(reply.result)
  if (status) parts.push(`status: ${status}`)
  if (target) parts.push(`target: ${target}`)
  if (result) parts.push(`result: ${result}`)
  return parts.join(' · ')
}

export function formatReplyCopySummary(reply: CommandReply): string {
  const lines = [`cmd: ${reply.cmd}`, `ok: ${reply.ok}`, `id: ${reply.id}`, `ts: ${reply.ts}s`]

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

export function formatEventSummary(event: EspEvent): string {
  const target = stringify(event.target)
  return target ? `${event.event} · target: ${target}` : event.event
}

export function formatEventCopySummary(event: EspEvent): string {
  const lines = [`event: ${event.event}`, `ts: ${event.ts}s`]
  const target = stringify(event.target)
  if (target) lines.push(`target: ${target}`)
  return lines.join('\n')
}

export async function copyText(text: string): Promise<boolean> {
  if (!navigator.clipboard?.writeText) return Promise.resolve(false)
  return navigator.clipboard.writeText(text).then(
    () => true,
    () => false,
  )
}

export function fieldEntries(reply: CommandReply): Array<[string, unknown]> {
  const core = ['id', 'cmd', 'ok', 'ts']
  const extras = Object.keys(reply)
    .filter((key) => !core.includes(key))
    .sort((a, b) => a.localeCompare(b))
  return [...core, ...extras].map((key) => [key, reply[key]])
}

export function replySearchText(reply: CommandReply): string {
  return Object.entries(reply)
    .map(([key, value]) => `${key} ${stringify(value)}`)
    .join(' ')
}

export function eventSearchText(event: EspEvent): string {
  return Object.entries(event)
    .map(([key, value]) => `${key} ${stringify(value)}`)
    .join(' ')
}

export function logSearchText(line: string): string {
  return line
}

export function matchesQuery(haystack: string, query: string): boolean {
  const needle = normalizeQuery(query)
  if (!needle) return true
  return normalizeQuery(haystack).includes(needle)
}

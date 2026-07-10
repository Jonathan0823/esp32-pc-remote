export function readText(value: unknown): string | null {
  return typeof value === 'string' && value.trim() ? value : null
}

export function readNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null
}

export function toMillis(value?: number | null): number | null {
  if (typeof value !== 'number' || !Number.isFinite(value)) return null
  return value > 1_000_000_000_000 ? value : value * 1000
}

export function resolveExpiresAt(
  reply: Record<string, unknown>,
  fallbackSeconds: number,
): number {
  const absolute =
    toMillis(reply.expires_at as number | undefined) ??
    toMillis(reply.wake_expires_at as number | undefined)
  if (absolute) return absolute
  if (typeof reply.expires_in_s === 'number' && Number.isFinite(reply.expires_in_s)) {
    return Date.now() + reply.expires_in_s * 1000
  }
  return Date.now() + fallbackSeconds * 1000
}

export function formatDuration(seconds: number): string {
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

export function formatAgo(value: number): string {
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

export function formatBroker(url: string): string {
  try {
    return new URL(url).host
  } catch {
    return url
  }
}

export function signalQuality(rssi: number): number {
  return Math.max(0, Math.min(100, Math.round(((rssi + 90) / 50) * 100)))
}

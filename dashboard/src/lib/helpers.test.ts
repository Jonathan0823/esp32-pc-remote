import { describe, expect, it, vi } from 'vitest'
import { resolveExpiresAt } from './helpers'

describe('resolveExpiresAt', () => {
  it('resolves boot-relative expires_at values against the current wall clock', () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-07-10T12:00:00.000Z'))

    const now = Date.now()
    const bootWallClockMs = now - 10_000

    expect(resolveExpiresAt({ expires_at: 42 }, 30, bootWallClockMs)).toBe(now + 32_000)

    vi.useRealTimers()
  })
})

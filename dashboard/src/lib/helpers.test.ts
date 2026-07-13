import { describe, expect, it, vi } from 'vitest'
import { formatDuration, resolveExpiresAt } from './helpers'

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

describe('formatDuration', () => {
  it('formats sub-minute durations', () => {
    expect(formatDuration(0)).toBe('0s')
    expect(formatDuration(30)).toBe('30s')
    expect(formatDuration(59)).toBe('59s')
  })

  it('formats minutes (under 1h)', () => {
    expect(formatDuration(60)).toBe('1m 0s')
    expect(formatDuration(125)).toBe('2m 5s')
    expect(formatDuration(3599)).toBe('59m 59s')
  })

  it('formats hours (under 1d)', () => {
    expect(formatDuration(3600)).toBe('1h 0m 0s')
    expect(formatDuration(3661)).toBe('1h 1m 1s')
    expect(formatDuration(86399)).toBe('23h 59m 59s')
  })

  it('formats days (1d+)', () => {
    expect(formatDuration(86400)).toBe('1d 0h 0m')
    expect(formatDuration(90061)).toBe('1d 1h 1m')
    expect(formatDuration(172800)).toBe('2d 0h 0m')
    expect(formatDuration(937839)).toBe('10d 20h 30m')
  })

  it('handles negative values as zero', () => {
    expect(formatDuration(-1)).toBe('0s')
    expect(formatDuration(-9999)).toBe('0s')
  })
})

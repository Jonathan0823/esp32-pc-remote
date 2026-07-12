import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import App from '../App'

const clipboardWriteText = vi.fn().mockResolvedValue(undefined)

vi.mock('../mqtt/useMqtt', () => ({
  useMqtt: () => ({
    connection: { connected: true, error: null, reconnecting: false },
    state: {
      online: true,
      ip: '192.168.1.10',
      rssi: -49,
      heap: 204800,
      uptime_s: 420,
      pc_online: true,
      pc_status: 'online',
      last_wake_result: 'wol_sent',
      last_wake_at: 42,
      wake_pending: false,
      ts: 420,
      reset_reason: 'poweron',
    },
    replies: [
      {
        id: 'wake-1',
        cmd: 'wake_request',
        ok: true,
        ts: 10,
        status: 'confirmation_required',
        confirm_token: 'cfm-12345678',
        expires_at: 99,
        target: 'Desktop-01',
      },
      {
        id: 'ping-1',
        cmd: 'ping',
        ok: false,
        ts: 12,
        message: 'timeout',
        rtt_ms: 123,
      },
    ],
    events: [
      { event: 'boot', ts: 1 },
      { event: 'pc_online', target: 'Desktop-01', ts: 15 },
    ],
    logs: ['[boot] start', '[mqtt] connected'],
    send: vi.fn(),
    clearFeed: vi.fn(),
    markReplyHandled: vi.fn(),
    isReplyHandled: vi.fn(() => false),
  }),
}))

beforeEach(() => {
  globalThis.history.pushState({}, '', '/activity')
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    value: { writeText: clipboardWriteText },
  })
  clipboardWriteText.mockClear()
})

describe('Activity page', () => {
  it('shows the activity tabs', async () => {
    render(<App />)

    expect(await screen.findByRole('button', { name: /replies/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /events/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /logs/i })).toBeInTheDocument()
  })

  it('filters the active tab only', async () => {
    render(<App />)

    fireEvent.change(await screen.findByPlaceholderText('Search replies'), {
      target: { value: 'ping' },
    })

    expect(screen.getAllByLabelText('Copy reply')).toHaveLength(1)

    fireEvent.click(screen.getByRole('button', { name: /events/i }))
    expect(screen.getAllByLabelText('Copy event')).toHaveLength(2)

    fireEvent.change(screen.getByPlaceholderText('Search events'), {
      target: { value: 'boot' },
    })

    expect(screen.getAllByLabelText('Copy event')).toHaveLength(1)
  })

  it('expands reply details and copies the reply summary', async () => {
    render(<App />)

    fireEvent.click((await screen.findAllByLabelText('Expand reply details'))[1])
    expect(await screen.findByText('confirm_token')).toBeInTheDocument()
    expect(screen.getByText('expires_at')).toBeInTheDocument()

    fireEvent.click(screen.getAllByLabelText('Copy reply')[1])
    fireEvent.click(await screen.findByText('Copy summary'))

    expect(clipboardWriteText).toHaveBeenCalledWith(expect.stringContaining('cmd: wake_request'))
    expect(clipboardWriteText).toHaveBeenCalledWith(
      expect.stringContaining('confirm_token: cfm-12345678'),
    )
  })
})

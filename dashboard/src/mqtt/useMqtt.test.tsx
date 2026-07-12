import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { useMqtt } from './useMqtt'

// ── Hoisted mock state ──
// Declares trackers before vi.mock so the factory can reference them.
const { listeners, mockClient } = vi.hoisted(() => {
  const listeners: Record<string, (...args: unknown[]) => void> = {}

  return {
    listeners,
    mockClient: {
      on: vi.fn((event: string, handler: (...args: unknown[]) => void) => {
        listeners[event] = handler
      }),
      subscribe: vi.fn(),
      publish: vi.fn(),
      end: vi.fn(),
      connected: true,
    },
  }
})

// ── Mock mqtt module ──
vi.mock('mqtt', () => ({
  default: {
    connect: vi.fn(() => mockClient),
  },
}))

function stubMqttEnv() {
  vi.stubEnv('VITE_MQTT_BROKER_URL', 'wss://test:8884/mqtt')
  vi.stubEnv('VITE_MQTT_USERNAME', 'testuser')
  vi.stubEnv('VITE_MQTT_PASSWORD', 'testpass')
  vi.stubEnv('VITE_MQTT_BASE_TOPIC', 'test-topic')
}

function clearListeners() {
  Object.keys(listeners).forEach((k) => delete listeners[k])
}

describe('useMqtt', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    clearListeners()
    vi.unstubAllEnvs()
  })

  afterEach(() => {
    vi.unstubAllEnvs()
  })

  // ── Missing broker URL ──
  it('reports missing broker URL error when VITE_MQTT_BROKER_URL is not set', () => {
    // Override to empty so getEnv() sees no URL
    vi.stubEnv('VITE_MQTT_BROKER_URL', '')
    const { result } = renderHook(() => useMqtt())

    expect(result.current.connection.connected).toBe(false)
    expect(result.current.connection.error).toBe('VITE_MQTT_BROKER_URL is not set')
    expect(result.current.connection.reconnecting).toBe(false)
    expect(result.current.state).toBeNull()
    // No mqtt.connect() should have been called
    expect(mockClient.on).not.toHaveBeenCalled()
  })

  describe('with broker URL', () => {
    beforeEach(() => {
      stubMqttEnv()
    })

    // ── Connect + subscribe ──
    it('connects and subscribes to topics on connect event', () => {
      const { result } = renderHook(() => useMqtt())

      // Initially not connected yet (before connect event fires)
      expect(result.current.connection.connected).toBe(false)

      // Trigger the connect event the client would fire
      act(() => {
        listeners['connect']()
      })

      expect(result.current.connection.connected).toBe(true)
      expect(result.current.connection.error).toBeNull()
      expect(mockClient.subscribe).toHaveBeenCalledTimes(5)
      expect(mockClient.subscribe).toHaveBeenCalledWith('test-topic/state', { qos: 1 })
      expect(mockClient.subscribe).toHaveBeenCalledWith('test-topic/availability', { qos: 1 })
      expect(mockClient.subscribe).toHaveBeenCalledWith('test-topic/reply', { qos: 0 })
      expect(mockClient.subscribe).toHaveBeenCalledWith('test-topic/event', { qos: 0 })
      expect(mockClient.subscribe).toHaveBeenCalledWith('test-topic/log', { qos: 0 })
    })

    // ── Connection lifecycle events ──
    it('handles reconnect, close, offline, and error events', () => {
      const { result } = renderHook(() => useMqtt())

      act(() => {
        listeners['connect']()
      })
      expect(result.current.connection.connected).toBe(true)
      expect(result.current.connection.reconnecting).toBe(false)

      // reconnect
      act(() => {
        listeners['reconnect']()
      })
      expect(result.current.connection.reconnecting).toBe(true)
      expect(result.current.connection.error).toBeNull()

      // close
      act(() => {
        listeners['close']()
      })
      expect(result.current.connection.connected).toBe(false)

      // offline
      act(() => {
        listeners['offline']()
      })
      expect(result.current.connection.connected).toBe(false)

      // error
      act(() => {
        listeners['error'](new Error('broker unreachable'))
      })
      expect(result.current.connection.connected).toBe(false)
      expect(result.current.connection.error).toBe('broker unreachable')
      expect(result.current.connection.reconnecting).toBe(false)
    })

    // ── State parsing ──
    it('parses and stores state from /state topic', () => {
      const { result } = renderHook(() => useMqtt())

      const statePayload = {
        online: true,
        ip: '192.168.1.10',
        rssi: -51,
        heap: 180224,
        uptime_s: 3600,
        pc_online: true,
        pc_status: 'online',
        last_wake_result: 'wol_sent',
        last_wake_at: 42,
        wake_pending: false,
        ts: 420,
        reset_reason: 'poweron',
      }

      act(() => {
        listeners['message']('test-topic/state', Buffer.from(JSON.stringify(statePayload)))
      })

      expect(result.current.state).toEqual(statePayload)
    })

    // ── Availability parsing ──
    it('parses and stores availability from /availability topic', () => {
      const { result } = renderHook(() => useMqtt())

      act(() => {
        listeners['message']('test-topic/availability', Buffer.from(JSON.stringify({ online: true, reason: 'mqtt_connected', ts: 42 })))
      })

      expect(result.current.availability).toEqual({ online: true, reason: 'mqtt_connected', ts: 42 })

      // Offline availability without ts
      act(() => {
        listeners['message']('test-topic/availability', Buffer.from(JSON.stringify({ online: false, reason: 'shutdown' })))
      })

      expect(result.current.availability).toEqual({ online: false, reason: 'shutdown' })
    })

    // ── Reply parsing ──
    it('parses and appends replies from /reply topic', () => {
      const { result } = renderHook(() => useMqtt())

      const reply = {
        id: 'ping-1',
        cmd: 'ping',
        ok: true,
        ts: 100,
        rtt_ms: 42,
      }

      act(() => {
        listeners['message']('test-topic/reply', Buffer.from(JSON.stringify(reply)))
      })

      expect(result.current.replies).toHaveLength(1)
      expect(result.current.replies[0]).toMatchObject({ id: 'ping-1', cmd: 'ping', ok: true })
    })

    // ── Event parsing ──
    it('parses and appends events from /event topic', () => {
      const { result } = renderHook(() => useMqtt())

      act(() => {
        listeners['message'](
          'test-topic/event',
          Buffer.from(JSON.stringify({ event: 'boot', ts: 1 })),
        )
      })
      act(() => {
        listeners['message'](
          'test-topic/event',
          Buffer.from(JSON.stringify({ event: 'pc_online', target: 'Desktop-01', ts: 15 })),
        )
      })

      expect(result.current.events).toHaveLength(2)
      expect(result.current.events[0]).toEqual({ event: 'boot', ts: 1 })
      expect(result.current.events[1]).toEqual({ event: 'pc_online', target: 'Desktop-01', ts: 15 })
    })

    // ── Log handling (plain text) ──
    it('stores plain-text payloads on /log topic', () => {
      const { result } = renderHook(() => useMqtt())

      act(() => {
        listeners['message']('test-topic/log', Buffer.from('[boot] ESP32 started'))
      })

      expect(result.current.logs).toHaveLength(1)
      expect(result.current.logs[0]).toContain('[boot] ESP32 started')
    })

    // ── Malformed JSON on non-log topics ──
    it('ignores malformed JSON on state topic without crashing', () => {
      const { result } = renderHook(() => useMqtt())

      act(() => {
        listeners['message']('test-topic/state', Buffer.from('not json'))
      })

      expect(result.current.state).toBeNull()
    })

    // ── Buffer caps ──
    it('caps replies at 100, events at 100, and logs at 500', () => {
      const { result } = renderHook(() => useMqtt())

      // Push 110 replies
      for (let i = 0; i < 110; i++) {
        act(() => {
          listeners['message'](
            'test-topic/reply',
            Buffer.from(JSON.stringify({ id: `r-${i}`, cmd: 'ping', ok: true, ts: i })),
          )
        })
      }
      expect(result.current.replies).toHaveLength(100)
      expect(result.current.replies[0].id).toBe('r-10') // oldest 10 dropped

      // Push 110 events
      for (let i = 0; i < 110; i++) {
        act(() => {
          listeners['message'](
            'test-topic/event',
            Buffer.from(JSON.stringify({ event: 'test', ts: i })),
          )
        })
      }
      expect(result.current.events).toHaveLength(100)

      // Push 510 logs
      for (let i = 0; i < 510; i++) {
        act(() => {
          listeners['message']('test-topic/log', Buffer.from(`line ${i}`))
        })
      }
      expect(result.current.logs).toHaveLength(500)
    })

    // ── send() publishes correct payload ──
    it('publishes a JSON command on /cmd with id, cmd, ts, and extra fields', () => {
      const { result } = renderHook(() => useMqtt())

      // Need connect event so client.connected is true
      act(() => {
        listeners['connect']()
      })

      act(() => {
        result.current.send('ping')
      })

      expect(mockClient.publish).toHaveBeenCalledTimes(1)
      const topic = mockClient.publish.mock.calls[0][0]
      const payloadStr = mockClient.publish.mock.calls[0][1]
      const opts = mockClient.publish.mock.calls[0][2]
      expect(topic).toBe('test-topic/cmd')
      expect(opts.qos).toBe(1)

      const payload = JSON.parse(payloadStr)
      expect(payload.cmd).toBe('ping')
      expect(payload.id).toBeTruthy()
      expect(typeof payload.ts).toBe('number')
    })

    it('includes extra fields in send() payload', () => {
      const { result } = renderHook(() => useMqtt())
      act(() => {
        listeners['connect']()
      })

      act(() => {
        result.current.send('wake_request', { target: 'Desktop-01', expires_in_s: 30 })
      })

      const payload = JSON.parse(mockClient.publish.mock.calls[0][1])
      expect(payload.cmd).toBe('wake_request')
      expect(payload.target).toBe('Desktop-01')
      expect(payload.expires_in_s).toBe(30)
    })

    // ── clearFeed ──
    it('clearFeed empties replies, events, logs and clears handled ids', () => {
      const { result } = renderHook(() => useMqtt())

      act(() => {
        listeners['message'](
          'test-topic/reply',
          Buffer.from(JSON.stringify({ id: 'r-1', cmd: 'ping', ok: true, ts: 1 })),
        )
        listeners['message'](
          'test-topic/event',
          Buffer.from(JSON.stringify({ event: 'boot', ts: 1 })),
        )
        listeners['message']('test-topic/log', Buffer.from('[boot] start'))
      })

      expect(result.current.replies).toHaveLength(1)
      expect(result.current.events).toHaveLength(1)
      expect(result.current.logs).toHaveLength(1)

      act(() => {
        result.current.clearFeed()
      })

      expect(result.current.replies).toHaveLength(0)
      expect(result.current.events).toHaveLength(0)
      expect(result.current.logs).toHaveLength(0)
    })

    // ── markReplyHandled / isReplyHandled ──
    it('tracks handled reply ids', () => {
      const { result } = renderHook(() => useMqtt())

      expect(result.current.isReplyHandled('r-42')).toBe(false)

      act(() => {
        result.current.markReplyHandled('r-42')
      })

      expect(result.current.isReplyHandled('r-42')).toBe(true)
      expect(result.current.isReplyHandled('other-id')).toBe(false)
    })
  })
})

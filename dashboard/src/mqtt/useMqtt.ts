import { useEffect, useRef, useState, useCallback } from 'react'
import mqtt from 'mqtt'
import type { EspState, CommandReply, EspEvent, MqttConnectionState } from './types'

export interface UseMqttReturn {
  connection: MqttConnectionState
  state: EspState | null
  replies: CommandReply[]
  events: EspEvent[]
  logs: string[]
  send: (cmd: string, payload?: Record<string, unknown>) => void
  clearFeed: () => void
}

function getEnv(key: string, fallback: string): string {
  return (import.meta.env[key] as string | undefined) || fallback
}

const MAX_REPLIES = 100
const MAX_EVENTS = 100
const MAX_LOGS = 500

export function useMqtt(): UseMqttReturn {
  const clientRef = useRef<mqtt.MqttClient | null>(null)
  const [connection, setConnection] = useState<MqttConnectionState>({
    connected: false,
    error: null,
    reconnecting: false,
  })
  const [state, setState] = useState<EspState | null>(null)
  const [replies, setReplies] = useState<CommandReply[]>([])
  const [events, setEvents] = useState<EspEvent[]>([])
  const [logs, setLogs] = useState<string[]>([])

  const append = <T>(arr: T[], item: T, max: number): T[] => {
    const next = [...arr, item]
    return next.length > max ? next.slice(-max) : next
  }

  useEffect(() => {
    const brokerUrl = getEnv('VITE_MQTT_BROKER_URL', '')
    const username = getEnv('VITE_MQTT_USERNAME', '')
    const password = getEnv('VITE_MQTT_PASSWORD', '')
    const baseTopic = getEnv('VITE_MQTT_BASE_TOPIC', 'esp-32-remote')

    if (!brokerUrl) {
      setConnection({
        connected: false,
        error: 'VITE_MQTT_BROKER_URL is not set',
        reconnecting: false,
      })
      return
    }

    const clientId = 'esp32-dash-' + Math.random().toString(16).slice(2, 10)

    const client = mqtt.connect(brokerUrl, {
      username,
      password,
      clientId,
      clean: true,
      reconnectPeriod: 5000,
      connectTimeout: 10000,
    })

    client.on('connect', () => {
      setConnection({ connected: true, error: null, reconnecting: false })
      client.subscribe(baseTopic + '/state', { qos: 1 })
      client.subscribe(baseTopic + '/reply', { qos: 0 })
      client.subscribe(baseTopic + '/event', { qos: 0 })
      client.subscribe(baseTopic + '/log', { qos: 0 })
    })

    client.on('reconnect', () => {
      setConnection((c) => ({ ...c, reconnecting: true, error: null }))
    })

    client.on('close', () => {
      setConnection((c) => ({ ...c, connected: false }))
    })

    client.on('offline', () => {
      setConnection((c) => ({ ...c, connected: false }))
    })

    client.on('error', (err) => {
      setConnection({ connected: false, error: err.message, reconnecting: false })
    })

    client.on('message', (topic, payload) => {
      const str = payload.toString()
      const relTopic = topic.replace(baseTopic + '/', '')

      try {
        const parsed = JSON.parse(str)

        switch (relTopic) {
          case 'state':
            setState(parsed as EspState)
            break
          case 'reply':
            setReplies((prev) => append(prev, parsed as CommandReply, MAX_REPLIES))
            break
          case 'event':
            setEvents((prev) => append(prev, parsed as EspEvent, MAX_EVENTS))
            break
          case 'log':
            setLogs((prev) => append(prev, `[${new Date().toLocaleTimeString()}] ${str}`, MAX_LOGS))
            break
        }
      } catch {
        // non-JSON payload on log topic is fine
        if (relTopic === 'log') {
          setLogs((prev) => append(prev, `[${new Date().toLocaleTimeString()}] ${str}`, MAX_LOGS))
        }
      }
    })

    clientRef.current = client

    return () => {
      client.end(true)
      clientRef.current = null
    }
  }, [])

  const send = useCallback((cmdName: string, extra: Record<string, unknown> = {}) => {
    const client = clientRef.current
    if (!client?.connected) return

    const baseTopic = getEnv('VITE_MQTT_BASE_TOPIC', 'esp-32-remote')
    const id =
      cmdName + '-' + Date.now().toString(36) + '-' + Math.random().toString(36).slice(2, 6)
    const payload = { id, cmd: cmdName, ts: Math.floor(Date.now() / 1000), ...extra }
    client.publish(baseTopic + '/cmd', JSON.stringify(payload), { qos: 1 })
  }, [])

  const clearFeed = useCallback(() => {
    setReplies([])
    setEvents([])
    setLogs([])
  }, [])

  return { connection, state, replies, events, logs, send, clearFeed }
}

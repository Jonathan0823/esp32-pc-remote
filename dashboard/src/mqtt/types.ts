// Payload shapes match src/services/mqtt_service.cpp (firmware)
export interface EspState {
  online: boolean
  ip?: string
  rssi?: number
  heap?: number
  uptime_s?: number
  pc_online: boolean
  pc_status?: string
  last_wake_result?: string
  last_wake_at?: number
  wake_pending?: boolean
  ts?: number
  reset_reason?: string
}

export interface CommandReply {
  id: string
  cmd: string
  ok: boolean
  ts: number
  [key: string]: unknown
}

export interface EspEvent {
  event: string
  target?: string
  ts: number
}

export interface AvailabilityPayload {
  online: boolean
  reason: string
  ts?: number
}

export interface MqttConnectionState {
  connected: boolean
  error: string | null
  reconnecting: boolean
}

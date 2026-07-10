export interface DeviceData {
  name: string
  controllerLabel: string
  online: boolean
  ready: boolean
  lastUpdateAgo: string
  rssi: number
  ipAddress: string
  uptime: string
  freeHeap: string
  mqttStatus: string
  grafanaStatus: string
  broker: string
  signalQuality: number
  pcOnline: boolean
  lastWake: string
  lastWakeStatus: string
}

export const PC_NAME = import.meta.env.VITE_PC_NAME || 'Desktop-01'
export const BROKER_URL = import.meta.env.VITE_MQTT_BROKER_URL as string | undefined

import { useOutletContext } from 'react-router-dom'
import type { useMqtt } from '@/mqtt/useMqtt'
import type { DeviceData } from '@/lib/types'
import type { EspState } from '@/mqtt/types'

export interface LayoutContext {
  device: DeviceData
  state: EspState | null
  connected: boolean
  connection: ReturnType<typeof useMqtt>['connection']
  send: (cmd: string, payload?: Record<string, unknown>) => void
  replies: ReturnType<typeof useMqtt>['replies']
  events: ReturnType<typeof useMqtt>['events']
  logs: string[]
  markReplyHandled: (id: string) => void
  isReplyHandled: (id: string) => boolean
}

export function useLayoutContext(): LayoutContext {
  const ctx = useOutletContext<LayoutContext>()
  if (!ctx) throw new Error('useLayoutContext must be used inside a Layout route')
  return ctx
}

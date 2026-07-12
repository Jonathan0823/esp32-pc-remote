import { toast } from 'sonner'
import { readText, readNumber, resolveExpiresAt } from '@/lib/helpers'
import type { CommandReply } from '@/mqtt/types'

type WakePhase = 'initial' | 'waiting' | 'confirm' | 'success'

export type ReplyHandlerContext = Readonly<{
  deviceName: string
  stateTs: number | null
  setWakePhase: (phase: WakePhase) => void
  setWakeToken: (token: string | null) => void
  setWakeExpiresAt: (expiresAt: number | null) => void
  setRebootOpen: (open: boolean) => void
}>

function handleWakeRequestReply(last: CommandReply, ctx: ReplyHandlerContext) {
  const token = readText(last.confirm_token)
  ctx.setWakePhase(last.ok ? 'confirm' : 'initial')
  if (last.ok) {
    ctx.setWakeToken(token)
    const bootWallClockMs =
      typeof ctx.stateTs === 'number' ? Date.now() - ctx.stateTs * 1000 : undefined
    ctx.setWakeExpiresAt(resolveExpiresAt(last, 30, bootWallClockMs))
    toast('Confirmation ready', { description: 'The controller issued a wake token.' })
  } else {
    toast.error(readText(last.message) || 'Wake request failed')
  }
}

function handleWakeConfirmReply(last: CommandReply, ctx: ReplyHandlerContext) {
  ctx.setWakePhase(last.ok ? 'success' : 'confirm')
  if (last.ok) {
    toast.success('Wake packet sent to ' + ctx.deviceName)
  } else {
    toast.error(readText(last.message) || 'Wake confirm failed')
  }
}

function handlePingReply(last: CommandReply) {
  const latency = readNumber(last.latencyMs) ?? readNumber(last.rtt_ms)
  if (last.ok) {
    toast.success(latency ? `ESP32 replied in ${latency} ms` : 'ESP32 replied')
  } else {
    toast.error(readText(last.message) || 'Ping failed')
  }
}

function handleRebootRequestReply(last: CommandReply, ctx: ReplyHandlerContext) {
  if (!last.ok) {
    toast.error(readText(last.message) || 'Reboot request failed')
    ctx.setRebootOpen(false)
  }
}

function handleRebootConfirmReply(last: CommandReply, ctx: ReplyHandlerContext) {
  if (last.ok) {
    toast.success('ESP32 rebooting')
    ctx.setRebootOpen(false)
  }
}

export function handleLatestReply(last: CommandReply, ctx: ReplyHandlerContext) {
  switch (last.cmd) {
    case 'wake_request':
      handleWakeRequestReply(last, ctx)
      break
    case 'wake_confirm':
      handleWakeConfirmReply(last, ctx)
      break
    case 'ping':
      handlePingReply(last)
      break
    case 'reboot_request':
      handleRebootRequestReply(last, ctx)
      break
    case 'reboot_confirm':
      handleRebootConfirmReply(last, ctx)
      break
  }
}

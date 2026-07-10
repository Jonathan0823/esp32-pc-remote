import { useEffect, useMemo, useRef, useState } from 'react'
import { useMqtt } from './mqtt/useMqtt'
import type { CommandReply } from './mqtt/types'
import './App.css'

const PC_NAME = import.meta.env.VITE_PC_NAME || 'PC'
const BASE_TOPIC = import.meta.env.VITE_MQTT_BASE_TOPIC || 'esp-32-remote'
const BROKER_URL = import.meta.env.VITE_MQTT_BROKER_URL as string | undefined
const THEME_KEY = 'command-deck-theme'

type Theme = 'dark' | 'light'
type NavId = 'dashboard' | 'events' | 'logs' | 'settings'
type FeedTab = 'replies' | 'events' | 'logs'
type FlowKind = 'wake' | 'reboot'
type FlowPhase = 'requesting' | 'confirm' | 'confirming' | 'success' | 'expired' | 'error'

type NoticeTone = 'info' | 'success' | 'warning' | 'danger' | 'neutral'

interface FlowState {
  kind: FlowKind
  phase: FlowPhase
  token: string | null
  expiresAt: number | null
  note: string
}

interface NoticeState {
  tone: NoticeTone
  text: string
}

function App() {
  const { connection, state, replies, events, logs, send, clearFeed } = useMqtt()
  const [theme, setTheme] = useState<Theme>(getInitialTheme)
  const [activeNav, setActiveNav] = useState<NavId>('dashboard')
  const [feedTab, setFeedTab] = useState<FeedTab>('replies')
  const [wakeFlow, setWakeFlow] = useState<FlowState | null>(null)
  const [rebootFlow, setRebootFlow] = useState<FlowState | null>(null)
  const [notice, setNotice] = useState<NoticeState | null>(null)
  const [now, setNow] = useState(() => Date.now())
  const controlRef = useRef<HTMLElement>(null)
  const activityRef = useRef<HTMLElement>(null)
  const settingsRef = useRef<HTMLElement>(null)
  const feedRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    document.documentElement.dataset.theme = theme
    document.documentElement.style.colorScheme = theme
    document.title = 'Command Deck'
    try {
      window.localStorage.setItem(THEME_KEY, theme)
    } catch {
      // ignore
    }
  }, [theme])

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1000)
    return () => window.clearInterval(timer)
  }, [])

  useEffect(() => {
    if (!notice) return
    const timer = window.setTimeout(() => setNotice(null), 3500)
    return () => window.clearTimeout(timer)
  }, [notice])

  useEffect(() => {
    const node = feedRef.current
    if (!node) return
    if (typeof node.scrollTo === 'function') {
      node.scrollTo({ top: node.scrollHeight, behavior: 'smooth' })
      return
    }
    node.scrollTop = node.scrollHeight
  }, [feedTab, replies.length, events.length, logs.length])

  useEffect(() => {
    const last = replies.at(-1)
    if (!last) return

    const cmd = last.cmd
    const ok = last.ok === true
    const status = readText(last.status)
    const result = readText(last.result)
    const message = readText(last.message)
    const latency = readNumber(last.latencyMs) ?? readNumber(last.rtt_ms)

    if (cmd === 'ping') {
      setNotice({
        tone: ok ? 'success' : 'danger',
        text: ok ? `ESP32 replied${latency ? ` in ${latency} ms` : ''}.` : message || 'Ping failed.',
      })
      return
    }

    if (cmd === 'wake_request') {
      if (ok && status === 'confirmation_required') {
        setWakeFlow((flow) =>
          flow
            ? {
                ...flow,
                phase: 'confirm',
                token: readText(last.confirm_token),
                expiresAt: resolveExpiresAt(last, 30),
                note: 'Confirmation ready.',
              }
            : flow,
        )
        setNotice({ tone: 'warning', text: 'Wake confirmation is ready.' })
      } else if (!ok) {
        setWakeFlow((flow) => (flow ? { ...flow, phase: 'error', note: message || 'Wake request failed.' } : flow))
        setNotice({ tone: 'danger', text: message || 'Wake request failed.' })
      }
      return
    }

    if (cmd === 'wake_confirm') {
      if (ok) {
        setWakeFlow((flow) => (flow ? { ...flow, phase: 'success', token: null, note: 'Wake packet sent.' } : flow))
        setNotice({ tone: 'success', text: 'Wake packet sent.' })
      } else {
        setWakeFlow((flow) => (flow ? { ...flow, phase: 'error', note: message || 'Wake confirm failed.' } : flow))
        setNotice({ tone: 'danger', text: message || 'Wake confirm failed.' })
      }
      return
    }

    if (cmd === 'reboot_request') {
      if (ok && status === 'confirmation_required') {
        setRebootFlow((flow) =>
          flow
            ? {
                ...flow,
                phase: 'confirm',
                token: readText(last.confirm_token),
                expiresAt: resolveExpiresAt(last, 30),
                note: 'Confirmation ready.',
              }
            : flow,
        )
        setNotice({ tone: 'warning', text: 'Reboot confirmation is ready.' })
      } else if (!ok) {
        setRebootFlow((flow) => (flow ? { ...flow, phase: 'error', note: message || 'Reboot request failed.' } : flow))
        setNotice({ tone: 'danger', text: message || 'Reboot request failed.' })
      }
      return
    }

    if (cmd === 'reboot_confirm') {
      if (ok) {
        setRebootFlow((flow) => (flow ? { ...flow, phase: 'success', token: null, note: 'ESP32 rebooting.' } : flow))
        setNotice({ tone: 'success', text: 'ESP32 rebooting.' })
      } else {
        setRebootFlow((flow) => (flow ? { ...flow, phase: 'error', note: message || 'Reboot confirm failed.' } : flow))
        setNotice({ tone: 'danger', text: message || 'Reboot confirm failed.' })
      }
      return
    }

    if (ok && result) {
      setNotice({ tone: 'info', text: result })
    }
  }, [replies])

  useEffect(() => {
    if (wakeFlow?.phase !== 'confirm') return
    if (!wakeFlow.expiresAt || now < wakeFlow.expiresAt) return
    setWakeFlow((flow) => (flow ? { ...flow, phase: 'expired', token: null, note: 'Confirmation expired.' } : flow))
  }, [now, wakeFlow?.expiresAt, wakeFlow?.phase])

  useEffect(() => {
    if (rebootFlow?.phase !== 'confirm') return
    if (!rebootFlow.expiresAt || now < rebootFlow.expiresAt) return
    setRebootFlow((flow) => (flow ? { ...flow, phase: 'expired', token: null, note: 'Confirmation expired.' } : flow))
  }, [now, rebootFlow?.expiresAt, rebootFlow?.phase])

  useEffect(() => {
    if (wakeFlow?.phase !== 'success') return
    const timer = window.setTimeout(() => setWakeFlow(null), 1100)
    return () => window.clearTimeout(timer)
  }, [wakeFlow?.phase])

  useEffect(() => {
    if (rebootFlow?.phase !== 'success') return
    const timer = window.setTimeout(() => setRebootFlow(null), 1100)
    return () => window.clearTimeout(timer)
  }, [rebootFlow?.phase])

  const pcReady = Boolean(state?.online && state?.mqtt_connected)
  const quality = signalQuality(state?.rssi)
  const qualityLabel = signalQualityLabel(quality)
  const uptime = formatDuration(state?.uptime_s)
  const lastUpdate = formatAgo(state?.ts)
  const lastWake = formatAgo(state?.last_wake_at)
  const heapKb = typeof state?.heap === 'number' ? Math.round(state.heap / 1024) : null
  const brokerHost = formatBroker(BROKER_URL)
  const connectionText = connection.connected
    ? 'Connected'
    : connection.reconnecting
      ? 'Reconnecting'
      : connection.error
        ? 'Needs config'
        : 'Offline'
  const connectionTone: NoticeTone = connection.connected ? 'success' : connection.reconnecting ? 'warning' : 'danger'

  const feedItems = useMemo(() => {
    if (feedTab === 'events') {
      return events.map((event) => ({
        key: `${event.ts}-${event.event}-${event.target || ''}`,
        kind: 'event' as const,
        time: formatClock(event.ts),
        title: event.event,
        body: event.target ? `→ ${event.target}` : 'Event',
        tone: 'info' as const,
      }))
    }

    if (feedTab === 'logs') {
      return logs.map((log, index) => ({
        key: `${index}-${log}`,
        kind: 'log' as const,
        time: '',
        title: 'Log',
        body: log,
        tone: 'neutral' as const,
      }))
    }

    return replies.map((reply) => {
      const body = omitReply(reply)
      return {
        key: `${reply.ts}-${reply.id}`,
        kind: 'reply' as const,
        time: formatClock(reply.ts),
        title: reply.cmd,
        body: body ? JSON.stringify(body) : reply.ok ? 'OK' : 'Error',
        tone: reply.ok ? ('success' as const) : ('danger' as const),
      }
    })
  }, [events, feedTab, logs, replies])

  const activeFlow = wakeFlow ?? rebootFlow
  const activeSectionLabel =
    activeNav === 'events' ? 'Activity' : activeNav === 'logs' ? 'Activity' : activeNav === 'settings' ? 'Settings' : 'Dashboard'

  const goTo = (nav: NavId) => {
    setActiveNav(nav)
    if (nav === 'events') {
      setFeedTab('events')
      activityRef.current?.scrollIntoView({ behavior: 'smooth', block: 'start' })
      return
    }
    if (nav === 'logs') {
      setFeedTab('logs')
      activityRef.current?.scrollIntoView({ behavior: 'smooth', block: 'start' })
      return
    }
    if (nav === 'settings') {
      settingsRef.current?.scrollIntoView({ behavior: 'smooth', block: 'start' })
      return
    }
    controlRef.current?.scrollIntoView({ behavior: 'smooth', block: 'start' })
  }

  const openFlow = (kind: FlowKind) => {
    const flow: FlowState = {
      kind,
      phase: 'requesting',
      token: null,
      expiresAt: Date.now() + 30_000,
      note: kind === 'wake' ? 'Requesting wake confirmation…' : 'Requesting reboot confirmation…',
    }

    if (kind === 'wake') {
      setWakeFlow(flow)
      send('wake_request', { target: PC_NAME, expires_in_s: 30 })
      setActiveNav('dashboard')
      return
    }

    setRebootFlow(flow)
    send('reboot_request', { expires_in_s: 30 })
    setActiveNav('dashboard')
  }

  const confirmFlow = (kind: FlowKind) => {
    const flow = kind === 'wake' ? wakeFlow : rebootFlow
    if (!flow?.token) return

    if (kind === 'wake') {
      setWakeFlow((current) => (current ? { ...current, phase: 'confirming', note: 'Sending wake confirmation…' } : current))
      send('wake_confirm', { confirm_token: flow.token, target: PC_NAME })
      return
    }

    setRebootFlow((current) => (current ? { ...current, phase: 'confirming', note: 'Sending reboot confirmation…' } : current))
    send('reboot_confirm', { confirm_token: flow.token })
  }

  const cancelFlow = (kind: FlowKind) => {
    if (kind === 'wake') {
      send('wake_cancel')
      setWakeFlow(null)
      return
    }

    send('reboot_cancel')
    setRebootFlow(null)
  }

  const retryFlow = (kind: FlowKind) => {
    if (kind === 'wake') {
      setWakeFlow(null)
      openFlow('wake')
      return
    }

    setRebootFlow(null)
    openFlow('reboot')
  }

  return (
    <div className="app-shell">
      <div className="layout">
        <aside className="sidebar" aria-label="Primary navigation">
          <div className="brand-lockup">
            <div className="brand-mark">CD</div>
            <div>
              <p className="brand-name">Command Deck</p>
              <p className="brand-note">Private control surface</p>
            </div>
          </div>

          <nav className="sidebar-nav">
            <NavButton label="Dashboard" active={activeNav === 'dashboard'} onClick={() => goTo('dashboard')} />
            <NavButton label="Events" active={activeNav === 'events'} onClick={() => goTo('events')} />
            <NavButton label="Logs" active={activeNav === 'logs'} onClick={() => goTo('logs')} />
            <NavButton label="Settings" active={activeNav === 'settings'} onClick={() => goTo('settings')} />
          </nav>

          <section className="mini-card">
            <div className={`status-pill status-pill--${connectionTone}`}>
              <span className="status-dot" />
              <span>{connectionText}</span>
            </div>
            <div className="mini-card-grid">
              <div>
                <span className="mini-label">Base topic</span>
                <span className="mono">{BASE_TOPIC}</span>
              </div>
              <div>
                <span className="mini-label">Target</span>
                <span>{PC_NAME}</span>
              </div>
            </div>
            <p className="muted-copy">{connection.error || 'Ready when the broker is.'}</p>
          </section>
        </aside>

        <main className="main">
          <header className="topbar">
            <div className="topbar-copy">
              <p className="eyebrow">Personal remote</p>
              <h1>Command Deck</h1>
              <p className="lead">Wake fast, check status, and keep the controller under control.</p>
            </div>

            <div className="topbar-actions">
              <div className="chip-row">
                <StatusChip tone={connectionTone} label={connectionText} />
                <StatusChip tone={pcReady ? 'success' : 'danger'} label={pcReady ? 'PC ready' : 'PC unavailable'} />
                <StatusChip tone={state?.online ? 'success' : 'danger'} label={state?.online ? 'ESP32 online' : 'ESP32 offline'} />
                <StatusChip tone={theme === 'dark' ? 'neutral' : 'info'} label={theme === 'dark' ? 'Dark' : 'Light'} />
              </div>
              <button className="theme-toggle" type="button" onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}>
                {theme === 'dark' ? 'Light mode' : 'Dark mode'}
              </button>
            </div>
          </header>

          <section className="top-strip" aria-label="Live status summary">
            <div className="strip-card">
              <span className="strip-label">Last update</span>
              <strong>{lastUpdate}</strong>
            </div>
            <div className="strip-card">
              <span className="strip-label">Wi-Fi RSSI</span>
              <strong>{typeof state?.rssi === 'number' ? `${state.rssi} dBm` : '—'}</strong>
            </div>
            <div className="strip-card">
              <span className="strip-label">Uptime</span>
              <strong>{uptime}</strong>
            </div>
            <div className="strip-card">
              <span className="strip-label">Signal</span>
              <strong>{qualityLabel}</strong>
            </div>
          </section>

          <section className="content-grid" ref={controlRef} id="dashboard">
            <article className="panel panel--control">
              <div className="panel-head">
                <div>
                  <p className="panel-kicker">Primary control</p>
                  <h2>{PC_NAME}</h2>
                </div>
                <StatusChip tone={pcReady ? 'success' : 'danger'} label={pcReady ? 'Ready' : 'Unavailable'} />
              </div>

              <p className="panel-copy">
                {connection.connected ? 'Connected to the broker. Send a wake, ping, or reboot with one tap.' : connection.error || 'Connect the broker to start controlling the PC.'}
              </p>

              <div className="panel-actions">
                <button className="btn btn-primary" onClick={() => openFlow('wake')} disabled={!connection.connected || wakeFlow?.phase === 'requesting' || wakeFlow?.phase === 'confirming'}>
                  Wake PC
                </button>
                <div className="btn-grid">
                  <button className="btn btn-secondary" onClick={() => send('ping')} disabled={!connection.connected}>
                    Ping ESP32
                  </button>
                  <button className="btn btn-secondary btn-warning" onClick={() => openFlow('reboot')} disabled={!connection.connected || rebootFlow?.phase === 'requesting' || rebootFlow?.phase === 'confirming'}>
                    Reboot ESP32
                  </button>
                </div>
              </div>

              {notice && <div className={`notice notice--${notice.tone}`}>{notice.text}</div>}

              <div className="flow-note">
                <span className="mini-label">Flow</span>
                <span>
                  {wakeFlow
                    ? wakeFlow.note
                    : rebootFlow
                      ? rebootFlow.note
                      : state?.wake_pending
                        ? 'Wake is pending in the controller.'
                        : 'Ready for the next command.'}
                </span>
              </div>
            </article>

            <article className="panel panel--status">
              <div className="panel-head">
                <div>
                  <p className="panel-kicker">Device status</p>
                  <h2>Desktop-01</h2>
                </div>
                <StatusChip tone={state?.online ? 'success' : 'danger'} label={state?.online ? 'Online' : 'Offline'} />
              </div>

              <dl className="status-list">
                <StatusRow label="Wi-Fi RSSI" value={typeof state?.rssi === 'number' ? `${state.rssi} dBm` : '—'} tone={signalTone(state?.rssi)} />
                <StatusRow label="IP address" value={state?.ip || '—'} tone="neutral" />
                <StatusRow label="Uptime" value={uptime} tone="neutral" />
                <StatusRow label="Free heap" value={heapKb !== null ? `${heapKb} KB` : '—'} tone="neutral" />
                <StatusRow label="MQTT" value={state?.mqtt_connected ? 'Connected' : 'Disconnected'} tone={state?.mqtt_connected ? 'success' : 'danger'} />
                <StatusRow label="Last wake" value={state?.last_wake_result || 'None'} tone={wakeTone(state?.last_wake_result)} />
                <StatusRow label="Last wake at" value={lastWake} tone="neutral" />
                <StatusRow label="Ready" value={pcReady ? 'Yes' : 'No'} tone={pcReady ? 'success' : 'danger'} />
              </dl>

              <div className="meter-block">
                <div className="meter-head">
                  <span className="mini-label">Signal quality</span>
                  <span className="mono">{quality !== null ? `${quality}%` : '—'}</span>
                </div>
                <div className="meter" aria-hidden="true">
                  <span className="meter-fill" style={{ width: `${quality ?? 0}%` }} />
                </div>
              </div>
            </article>
          </section>

          <section className="content-grid content-grid--stack" ref={activityRef} id="activity">
            <article className="panel panel--activity">
              <div className="panel-head panel-head--tight">
                <div>
                  <p className="panel-kicker">Activity</p>
                  <h2>Recent feed</h2>
                </div>
                <div className="panel-head-actions">
                  <div className="segmented" role="tablist" aria-label="Activity feed tabs">
                    <TabButton label="Replies" count={replies.length} active={feedTab === 'replies'} onClick={() => {
                      setActiveNav('dashboard')
                      setFeedTab('replies')
                    }} />
                    <TabButton label="Events" count={events.length} active={feedTab === 'events'} onClick={() => {
                      setActiveNav('events')
                      setFeedTab('events')
                    }} />
                    <TabButton label="Logs" count={logs.length} active={feedTab === 'logs'} onClick={() => {
                      setActiveNav('logs')
                      setFeedTab('logs')
                    }} />
                  </div>
                  <button className="ghost-btn" type="button" onClick={clearFeed}>
                    Clear feed
                  </button>
                </div>
              </div>

              <div className="feed-shell" ref={feedRef}>
                {feedItems.length === 0 ? (
                  <div className="empty-state">
                    <strong>No {feedTab} yet.</strong>
                    <span>{feedTab === 'replies' ? 'Wake or ping to start the control feed.' : feedTab === 'events' ? 'The controller will push events here.' : 'Logs will land here as the controller talks.'}</span>
                  </div>
                ) : (
                  <ul className="feed-list">
                    {feedItems.map((item) => (
                      <li className={`feed-item feed-item--${item.tone}`} key={item.key}>
                        <span className="feed-tag">{item.kind}</span>
                        {item.time && <span className="feed-time">{item.time}</span>}
                        <span className="feed-title">{item.title}</span>
                        <span className="feed-body">{item.body}</span>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            </article>
          </section>

          <section className="content-grid content-grid--stack" ref={settingsRef} id="settings">
            <article className="panel panel--settings">
              <div className="panel-head">
                <div>
                  <p className="panel-kicker">Settings</p>
                  <h2>Theme and connection</h2>
                </div>
                <StatusChip tone="neutral" label={activeSectionLabel} />
              </div>

              <div className="settings-grid">
                <button className="setting-row setting-row--button" type="button" onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}>
                  <span>
                    <span className="mini-label">Theme</span>
                    <strong>{theme === 'dark' ? 'Dark' : 'Light'}</strong>
                  </span>
                  <span className="setting-action">Switch</span>
                </button>

                <div className="setting-row">
                  <span>
                    <span className="mini-label">Broker</span>
                    <strong>{brokerHost}</strong>
                  </span>
                  <span className="mono">{connection.error ? 'check env' : 'live'}</span>
                </div>

                <div className="setting-row">
                  <span>
                    <span className="mini-label">Base topic</span>
                    <strong>{BASE_TOPIC}</strong>
                  </span>
                  <span className="mono">MQTT</span>
                </div>

                <div className="setting-row">
                  <span>
                    <span className="mini-label">Target</span>
                    <strong>{PC_NAME}</strong>
                  </span>
                  <span className="mono">{state?.online ? 'online' : 'idle'}</span>
                </div>
              </div>
            </article>
          </section>
        </main>
      </div>

      <nav className="bottom-nav" aria-label="Mobile navigation">
        <NavButton label="Dashboard" active={activeNav === 'dashboard'} onClick={() => goTo('dashboard')} />
        <NavButton label="Events" active={activeNav === 'events'} onClick={() => goTo('events')} />
        <NavButton label="Logs" active={activeNav === 'logs'} onClick={() => goTo('logs')} />
        <NavButton label="Settings" active={activeNav === 'settings'} onClick={() => goTo('settings')} />
      </nav>

      {activeFlow && (
        <div className="modal-backdrop" role="presentation">
          <section className="modal" role="dialog" aria-modal="true" aria-live="polite" aria-label={activeFlow.kind === 'wake' ? 'Wake PC confirmation' : 'Reboot ESP32 confirmation'}>
            <div className="modal-head">
              <div>
                <p className="panel-kicker">{activeFlow.kind === 'wake' ? 'Wake PC' : 'Reboot ESP32'}</p>
                <h2>{activeFlow.kind === 'wake' ? 'Wake Desktop-01?' : 'Reboot the controller?'}</h2>
              </div>
              <button className="ghost-btn" type="button" onClick={() => cancelFlow(activeFlow.kind)}>
                Close
              </button>
            </div>

            <p className="modal-copy">
              {activeFlow.kind === 'wake'
                ? 'This will ask the controller for a wake token, then send the wake packet when you confirm.'
                : 'This will restart the ESP32 controller after you confirm the reboot token.'}
            </p>

            <div className="modal-status">
              <span className={`status-pill status-pill--${activeFlow.kind === 'wake' ? 'warning' : 'danger'}`}>
                <span className="status-dot" />
                <span>{activeFlow.note}</span>
              </span>
              {activeFlow.phase === 'confirm' && activeFlow.expiresAt && (
                <span className="mono">{Math.max(0, Math.ceil((activeFlow.expiresAt - now) / 1000))}s left</span>
              )}
            </div>

            <div className="modal-body">
              {activeFlow.phase === 'requesting' && <p className="modal-message">Waiting for the controller to issue a token…</p>}
              {activeFlow.phase === 'confirm' && <p className="modal-message">Token received. Confirm before the timer runs out.</p>}
              {activeFlow.phase === 'confirming' && <p className="modal-message">Sending the confirmation now…</p>}
              {activeFlow.phase === 'expired' && <p className="modal-message modal-message--danger">Confirmation expired. Start again.</p>}
              {activeFlow.phase === 'error' && <p className="modal-message modal-message--danger">Something went wrong. Try again.</p>}
              {activeFlow.phase === 'success' && <p className="modal-message modal-message--success">Done.</p>}
            </div>

            <div className="modal-actions">
              {activeFlow.phase === 'confirm' && (
                <>
                  <button className="btn btn-primary" type="button" onClick={() => confirmFlow(activeFlow.kind)}>
                    {activeFlow.kind === 'wake' ? 'Confirm wake' : 'Confirm reboot'}
                  </button>
                  <button className="btn btn-secondary" type="button" onClick={() => cancelFlow(activeFlow.kind)}>
                    Cancel
                  </button>
                </>
              )}

              {activeFlow.phase === 'requesting' && (
                <button className="btn btn-secondary" type="button" onClick={() => cancelFlow(activeFlow.kind)}>
                  Cancel request
                </button>
              )}

              {activeFlow.phase === 'confirming' && <button className="btn btn-secondary" type="button" disabled>Working…</button>}

              {(activeFlow.phase === 'expired' || activeFlow.phase === 'error') && (
                <>
                  <button className="btn btn-primary" type="button" onClick={() => retryFlow(activeFlow.kind)}>
                    Try again
                  </button>
                  <button className="btn btn-secondary" type="button" onClick={() => cancelFlow(activeFlow.kind)}>
                    Close
                  </button>
                </>
              )}
            </div>
          </section>
        </div>
      )}
    </div>
  )
}

function NavButton({ label, active, onClick }: { label: string; active: boolean; onClick: () => void }) {
  return (
    <button className={`nav-button ${active ? 'is-active' : ''}`} type="button" onClick={onClick}>
      <span className="nav-dot" aria-hidden="true" />
      <span>{label}</span>
    </button>
  )
}

function StatusChip({ tone, label }: { tone: NoticeTone; label: string }) {
  return (
    <span className={`status-pill status-pill--${tone}`}>
      <span className="status-dot" />
      <span>{label}</span>
    </span>
  )
}

function TabButton({ label, count, active, onClick }: { label: string; count: number; active: boolean; onClick: () => void }) {
  return (
    <button className={`segmented-button ${active ? 'is-active' : ''}`} type="button" role="tab" aria-selected={active} onClick={onClick}>
      <span>{label}</span>
      <span className="segmented-count">{count}</span>
    </button>
  )
}

function StatusRow({ label, value, tone }: { label: string; value: string; tone: 'neutral' | NoticeTone }) {
  return (
    <div className="status-row">
      <dt>{label}</dt>
      <dd className={`status-value status-value--${tone}`}>{value}</dd>
    </div>
  )
}

function getInitialTheme(): Theme {
  if (typeof window === 'undefined') return 'dark'
  try {
    const stored = window.localStorage.getItem(THEME_KEY)
    if (stored === 'dark' || stored === 'light') return stored
    return window.matchMedia?.('(prefers-color-scheme: light)').matches ? 'light' : 'dark'
  } catch {
    return 'dark'
  }
}

function readText(value: unknown): string | null {
  return typeof value === 'string' && value.trim() ? value : null
}

function readNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null
}

function toMillis(value?: number | null): number | null {
  if (typeof value !== 'number' || !Number.isFinite(value)) return null
  return value > 1_000_000_000_000 ? value : value * 1000
}

function resolveExpiresAt(reply: CommandReply, fallbackSeconds: number): number {
  const extras = reply as CommandReply & {
    expires_at?: number
    expires_in_s?: number
    wake_expires_at?: number
  }
  const absolute = toMillis(extras.expires_at) ?? toMillis(extras.wake_expires_at)
  if (absolute) return absolute
  if (typeof extras.expires_in_s === 'number' && Number.isFinite(extras.expires_in_s)) {
    return Date.now() + extras.expires_in_s * 1000
  }
  return Date.now() + fallbackSeconds * 1000
}

function formatDuration(seconds?: number | null): string {
  if (typeof seconds !== 'number' || !Number.isFinite(seconds)) return '—'
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

function formatAgo(value?: number | null): string {
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

function formatClock(value: number): string {
  const ms = toMillis(value)
  if (!ms) return ''
  return new Intl.DateTimeFormat(undefined, {
    hour: '2-digit',
    minute: '2-digit',
  }).format(ms)
}

function formatBroker(url?: string): string {
  if (!url) return 'Not configured'
  try {
    return new URL(url).host
  } catch {
    return url
  }
}

function signalQuality(rssi?: number | null): number | null {
  if (typeof rssi !== 'number' || !Number.isFinite(rssi)) return null
  return Math.max(0, Math.min(100, Math.round(((rssi + 90) / 50) * 100)))
}

function signalQualityLabel(value: number | null): string {
  if (value === null) return 'Unknown'
  if (value >= 80) return 'Excellent'
  if (value >= 60) return 'Good'
  if (value >= 40) return 'Fair'
  return 'Weak'
}

function signalTone(rssi?: number | null): NoticeTone {
  if (typeof rssi !== 'number' || !Number.isFinite(rssi)) return 'info'
  if (rssi >= -60) return 'success'
  if (rssi >= -75) return 'warning'
  return 'danger'
}

function wakeTone(value?: string | null): NoticeTone {
  if (!value) return 'neutral'
  if (value === 'wol_sent' || value === 'success') return 'success'
  if (value.toLowerCase().includes('wait') || value.toLowerCase().includes('confirm')) return 'warning'
  return 'info'
}

function omitReply(reply: CommandReply): Record<string, unknown> | null {
  const omit = new Set(['id', 'cmd', 'ok', 'ts'])
  const out: Record<string, unknown> = {}
  for (const [key, value] of Object.entries(reply)) {
    if (omit.has(key) || value === undefined || typeof value === 'function') continue
    out[key] = value
  }
  return Object.keys(out).length ? out : null
}

export default App

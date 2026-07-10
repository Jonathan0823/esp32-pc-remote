import { useState, useRef, useEffect } from 'react'
import { useMqtt } from './mqtt/useMqtt'
import './App.css'

const PC_NAME = import.meta.env.VITE_PC_NAME || 'PC'

function App() {
  const { connection, state, replies, events, logs, send, clearFeed } = useMqtt()
  const [wakeToken, setWakeToken] = useState<string | null>(null)
  const [rebootToken, setRebootToken] = useState<string | null>(null)
  const feedRef = useRef<HTMLDivElement>(null)

  // Auto-scroll feed
  useEffect(() => {
    if (feedRef.current) {
      feedRef.current.scrollTop = feedRef.current.scrollHeight
    }
  }, [replies, events, logs])

  // ---- Commands ----
  const doPing = () => send('ping')

  const doWakeForce = () => send('wake_request', { target: PC_NAME, force: true })

  const doWakeRequest = () => send('wake_request', { target: PC_NAME, expires_in_s: 60 })

  const doWakeConfirm = () => {
    if (wakeToken) {
      send('wake_confirm', { confirm_token: wakeToken, target: PC_NAME })
      setWakeToken(null)
    }
  }

  // Watch incoming replies for wake_confirm token
  useEffect(() => {
    const last = replies[replies.length - 1]
    if (!last) return
    if (last.cmd === 'wake_request' && last.ok && last.status === 'confirmation_required') {
      setWakeToken(last.confirm_token as string)
    }
    if (last.cmd === 'wake_confirm' && last.ok) {
      setWakeToken(null)
    }
    if (last.cmd === 'wake_cancel') {
      setWakeToken(null)
    }
    if (last.cmd === 'reboot_request' && last.ok && last.status === 'confirmation_required') {
      setRebootToken(last.confirm_token as string)
    }
    if (last.cmd === 'reboot_confirm' && last.ok) {
      setRebootToken(null)
    }
    if (last.cmd === 'reboot_cancel') {
      setRebootToken(null)
    }
  }, [replies])

  const doRebootRequest = () => send('reboot_request', { expires_in_s: 60 })

  const doRebootConfirm = () => {
    if (rebootToken) {
      send('reboot_confirm', { confirm_token: rebootToken })
      setRebootToken(null)
    }
  }

  // ---- Render helpers ----
  const connClass = connection.connected ? 'conn-ok' : connection.reconnecting ? 'conn-pending' : 'conn-err'
  const connText = connection.connected
    ? 'Connected'
    : connection.reconnecting
      ? 'Reconnecting…'
      : connection.error
        ? 'Error: ' + connection.error
        : 'Disconnected'

  const pcStatusClass = state?.pc_online ? 'pc-online' : 'pc-offline'
  const pcStatusText = state?.pc_online ? 'Online' : 'Offline'

  const espOnline = state?.online ? 'Online' : 'Offline'

  return (
    <div className="app">
      {/* ---- Header ---- */}
      <header className="header">
        <h1>ESP32 PC Remote</h1>
        <div className="status-bar">
          <span className={`badge ${connClass}`}>{connText}</span>
          <span className={`badge ${pcStatusClass}`}>PC: {pcStatusText}</span>
          <span className="badge conn-ok">ESP32: {espOnline}</span>
        </div>
      </header>

      {/* ---- PC State Card ---- */}
      <section className="card state-card">
        <h2>{PC_NAME}</h2>
        <div className="state-grid">
          <div><strong>RSSI:</strong> {state?.rssi ?? '—'} dBm</div>
          <div><strong>Heap:</strong> {state?.heap ? (state.heap / 1024).toFixed(0) + ' KB' : '—'}</div>
          <div><strong>Uptime:</strong> {state?.uptime_s ? formatUptime(state.uptime_s) : '—'}</div>
          <div><strong>ESP IP:</strong> {state?.ip ?? '—'}</div>
          {state?.last_wake_result && (
            <div><strong>Last wake:</strong> {state.last_wake_result}</div>
          )}
          {state?.wake_pending && (
            <div className="wake-pending">Wake in progress…</div>
          )}
        </div>
      </section>

      {/* ---- Action Buttons ---- */}
      <section className="card actions-card">
        <h2>Actions</h2>
        <div className="btn-row">
          <button className="btn btn-ping" onClick={doPing} disabled={!connection.connected}>
            Ping
          </button>
          <button className="btn btn-wake" onClick={doWakeForce} disabled={!connection.connected}>
            Force Wake
          </button>
          <button className="btn btn-wake" onClick={doWakeRequest} disabled={!connection.connected}>
            Wake (with confirm)
          </button>
          <button className="btn btn-reboot" onClick={doRebootRequest} disabled={!connection.connected}>
            Reboot ESP32
          </button>
          <button className="btn btn-clear" onClick={clearFeed}>
            Clear Feed
          </button>
        </div>
      </section>

      {/* ---- Confirmation dialogs ---- */}
      {wakeToken && (
        <div className="overlay">
          <div className="confirm-dialog">
            <h3>Wake {PC_NAME}?</h3>
            <p>A confirmation token was received. Send the wake command?</p>
            <div className="btn-row">
              <button className="btn btn-primary" onClick={doWakeConfirm}>Confirm</button>
              <button className="btn btn-secondary" onClick={() => { send('wake_cancel'); setWakeToken(null) }}>Cancel</button>
            </div>
          </div>
        </div>
      )}
      {rebootToken && (
        <div className="overlay">
          <div className="confirm-dialog">
            <h3>Reboot ESP32?</h3>
            <p>This will restart the ESP32 device.</p>
            <div className="btn-row">
              <button className="btn btn-primary" onClick={doRebootConfirm}>Confirm</button>
              <button className="btn btn-secondary" onClick={() => { send('reboot_cancel'); setRebootToken(null) }}>Cancel</button>
            </div>
          </div>
        </div>
      )}

      {/* ---- Live Feed ---- */}
      <section className="card feed-card">
        <h2>Live Feed</h2>
        <div className="feed-tabs">
          <span className="feed-tab active">Replies ({replies.length})</span>
          <span className="feed-tab">Events ({events.length})</span>
          <span className="feed-tab">Logs ({logs.length})</span>
        </div>
        <div className="feed" ref={feedRef}>
          {replies.length === 0 && events.length === 0 && logs.length === 0 && (
            <div className="feed-empty">No data yet. Wait for updates or send a Ping.</div>
          )}
          {replies.map((r, i) => (
            <div key={i} className={`feed-item ${r.ok ? 'reply-ok' : 'reply-err'}`}>
              <span className="feed-tag">REPLY</span>
              <span className="feed-cmd">{r.cmd}</span>
              <span className={`feed-status ${r.ok ? 'ok' : 'err'}`}>{r.ok ? '✓' : '✗'}</span>
              <span className="feed-body">{JSON.stringify(omitKeys(r, 'id', 'cmd', 'ok', 'ts'))}</span>
            </div>
          ))}
          {events.map((e, i) => (
            <div key={i} className="feed-item feed-event">
              <span className="feed-tag">EVENT</span>
              <span className="feed-body">{e.event}{e.target ? ' → ' + e.target : ''}</span>
            </div>
          ))}
          {logs.map((l, i) => (
            <div key={i} className="feed-item feed-log">
              <span className="feed-tag">LOG</span>
              <code className="feed-body">{l}</code>
            </div>
          ))}
        </div>
      </section>

      <footer className="footer">
        <span className="mqtt-topic">{import.meta.env.VITE_MQTT_BASE_TOPIC || 'esp-32-remote'}</span>
      </footer>
    </div>
  )
}

function formatUptime(s: number): string {
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  const sec = s % 60
  return `${h}h ${m}m ${sec}s`
}

function omitKeys(obj: Record<string, unknown>, ...keys: string[]): Record<string, unknown> {
  const out: Record<string, unknown> = {}
  for (const k of Object.keys(obj)) {
    if (!keys.includes(k)) out[k] = obj[k]
  }
  return out
}

export default App

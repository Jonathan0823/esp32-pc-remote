import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import App from '../App'

vi.mock('../mqtt/useMqtt', () => ({
  useMqtt: () => ({
    connection: { connected: false, error: 'VITE_MQTT_BROKER_URL is not set', reconnecting: false },
    state: null,
    replies: [],
    events: [],
    logs: [],
    send: vi.fn(),
    clearFeed: vi.fn(),
  }),
}))

describe('App', () => {
  it('renders the header', () => {
    render(<App />)
    expect(screen.getByText('ESP32 PC Remote')).toBeInTheDocument()
  })

  it('shows disconnected state', () => {
    render(<App />)
    expect(screen.getByText(/Error/)).toBeInTheDocument()
  })

  it('renders action buttons', () => {
    render(<App />)
    expect(screen.getByText('Ping')).toBeInTheDocument()
    expect(screen.getByText('Force Wake')).toBeInTheDocument()
    expect(screen.getByText('Reboot ESP32')).toBeInTheDocument()
  })
})

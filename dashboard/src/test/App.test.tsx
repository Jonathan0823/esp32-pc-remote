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
  it('renders the command deck shell', () => {
    render(<App />)
    expect(screen.getByRole('heading', { name: /command deck/i })).toBeInTheDocument()
  })

  it('shows the primary controls', () => {
    render(<App />)
    expect(screen.getByRole('button', { name: /wake pc/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /ping esp32/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /reboot esp32/i })).toBeInTheDocument()
  })

  it('shows the mqtt setup error', () => {
    render(<App />)
    expect(screen.getAllByText(/VITE_MQTT_BROKER_URL is not set/i)[0]).toBeInTheDocument()
  })
})

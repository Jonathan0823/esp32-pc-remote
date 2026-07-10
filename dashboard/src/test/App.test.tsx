import { beforeAll, describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import App from '../App'

beforeAll(() => {
  Object.defineProperty(window, 'matchMedia', {
    writable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  })
})

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
  it('renders the control panel shell', () => {
    render(<App />)
    expect(screen.getByText('PC Remote')).toBeInTheDocument()
  })

  it('shows the primary controls', () => {
    render(<App />)
    expect(screen.getByRole('button', { name: /wake pc/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /ping esp32/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /reboot esp32/i })).toBeInTheDocument()
  })

  it('shows the dashboard cards', () => {
    render(<App />)
    expect(screen.getByText('PC Control')).toBeInTheDocument()
    expect(screen.getByText('Device Status')).toBeInTheDocument()
    expect(screen.getByText('Recent Events')).toBeInTheDocument()
    expect(screen.getByText('Connection Health')).toBeInTheDocument()
  })
})

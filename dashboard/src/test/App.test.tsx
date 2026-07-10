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
    // two copies: desktop + mobile layout
    expect(screen.getAllByRole('button', { name: /wake pc/i })).toHaveLength(2)
    expect(screen.getAllByRole('button', { name: /ping esp32/i })).toHaveLength(2)
    expect(screen.getAllByRole('button', { name: /reboot esp32/i })).toHaveLength(2)
  })

  it('shows the dashboard cards', () => {
    render(<App />)
    // two copies: desktop + mobile layout
    expect(screen.getAllByText('PC Control')).toHaveLength(2)
    expect(screen.getAllByText('Controller')).toHaveLength(2)
    expect(screen.getAllByText('Activity')).toHaveLength(3)
  })
})

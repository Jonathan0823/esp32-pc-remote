import type { Theme } from '@/components/ui/theme-provider'
import { SunIcon, MoonIcon } from '@phosphor-icons/react'

export function ThemeToggle({
  theme,
  setTheme,
}: Readonly<{ theme: Theme; setTheme: (t: Theme) => void }>) {
  const next = theme === 'dark' ? 'light' : 'dark'
  return (
    <button
      onClick={() => setTheme(next)}
      className="text-muted-foreground hover:text-foreground flex size-8 items-center justify-center rounded-none transition-colors"
      aria-label={`Switch to ${next} theme`}
    >
      {theme === 'dark' ? <SunIcon className="size-4" /> : <MoonIcon className="size-4" />}
    </button>
  )
}

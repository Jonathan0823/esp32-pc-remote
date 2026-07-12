import { useState, useEffect } from 'react'
import mermaid from 'mermaid'
import { useTheme } from '@/hooks/use-theme'

export function MermaidBlock({ diagram }: Readonly<{ diagram: string }>) {
  const { theme } = useTheme()
  const [svg, setSvg] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    const uid = 'mermaid-' + Math.random().toString(36).slice(2, 8)
    const mermaidTheme = theme === 'dark' ? 'dark' : 'default'
    mermaid.initialize({ startOnLoad: false, theme: mermaidTheme })
    mermaid
      .render(uid, diagram)
      .then((result) => {
        if (!cancelled) setSvg(result.svg)
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(String(err))
      })
    return () => {
      cancelled = true
    }
  }, [diagram, theme])

  if (error)
    return (
      <pre className="text-destructive my-4 overflow-x-auto rounded-lg border border-red-300 bg-red-50 p-4 text-sm dark:bg-red-950">
        Mermaid error: {error}
      </pre>
    )
  if (!svg)
    return (
      <div className="text-muted-foreground my-4 p-4 text-center text-sm">Rendering diagram…</div>
    )
  return (
    <div
      data-mermaid="true"
      className="my-4 flex justify-center"
      dangerouslySetInnerHTML={{ __html: svg }}
    />
  )
}

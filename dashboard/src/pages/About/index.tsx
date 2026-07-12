import { useState, useEffect, useRef } from 'react'
import { ArrowLeftIcon, ArrowUpIcon } from '@phosphor-icons/react'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Separator } from '@/components/ui/separator'
import { buttonVariants } from '@/components/ui/button-variants'
import { cn } from '@/lib/utils'
import { ABOUT_DOCS, ABOUT_REPO_URL } from '@/lib/about-docs'
import { DocSection } from '@/pages/About/components/about-markdown'

export default function About() {
  const [backTarget, setBackTarget] = useState<{ href: string; label: string } | null>(null)
  const scrollRef = useRef<HTMLDivElement>(null)
  const [showScrollTop, setShowScrollTop] = useState(false)

  useEffect(() => {
    const el = scrollRef.current?.closest('.overflow-auto') as HTMLElement | null
    if (!el) return
    const check = () => setShowScrollTop(el.scrollTop > 200)
    el.addEventListener('scroll', check, { passive: true })
    check()
    return () => el.removeEventListener('scroll', check)
  }, [])

  return (
    <div ref={scrollRef} id="about-top" className="mx-auto grid max-w-5xl gap-5">
      <Card className="overflow-hidden">
        <CardHeader className="space-y-4">
          <div className="flex flex-nowrap items-center gap-2 overflow-x-auto pb-1">
            <Badge variant="secondary">ESP32</Badge>
            <Badge variant="secondary">MQTT</Badge>
            <Badge variant="secondary">Telegram</Badge>
            <Badge variant="secondary">Wake-on-LAN</Badge>
            <Badge variant="secondary">Grafana</Badge>
          </div>
          <div className="grid gap-2">
            <CardTitle className="text-2xl">About this project</CardTitle>
            <p className="text-muted-foreground text-sm leading-6">
              <span className="text-foreground font-medium">esp32-wake-on-lan-remote</span> is a
              private PC control stack: an ESP32 firmware plus a React dashboard for waking,
              checking, and managing a desktop over MQTT or Telegram.
            </p>
          </div>
        </CardHeader>
        <CardContent className="grid gap-4 text-sm leading-6">
          <ul className="text-muted-foreground list-disc space-y-2 pl-6">
            <li>Wake your PC with confirmation or force mode.</li>
            <li>Check online status, ping health, and reboot the controller.</li>
            <li>Use MQTT over WebSocket for the dashboard, or Telegram for command control.</li>
            <li>See the setup notes for Wake-on-LAN, MQTT access control, and optional logging.</li>
          </ul>
          <Separator />
          <div className="grid gap-2 text-sm">
            <span className="text-muted-foreground text-xs tracking-wide uppercase">Jump to</span>
            <div className="flex flex-wrap gap-2">
              {ABOUT_DOCS.map((doc) => (
                <a
                  key={doc.sourcePath}
                  href={`#${doc.slug}`}
                  className="bg-muted text-foreground hover:bg-muted/80 rounded-md border px-3 py-1.5 text-xs font-medium no-underline transition-colors"
                >
                  {doc.title}
                </a>
              ))}
            </div>
          </div>
          <Separator />
          <a
            href={ABOUT_REPO_URL}
            target="_blank"
            rel="noopener noreferrer"
            className="text-foreground decoration-border hover:decoration-foreground underline underline-offset-4"
          >
            View the repository on GitHub
          </a>
        </CardContent>
      </Card>

      {ABOUT_DOCS.map((doc) => (
        <DocSection
          key={doc.sourcePath}
          {...doc}
          onLocalLinkClick={(href) => {
            setBackTarget({ href: globalThis.location.hash || '#about-top', label: 'Back' })
            globalThis.location.hash = href
          }}
        />
      ))}

      <a
        href={backTarget?.href ?? '#about-top'}
        onClick={() => setBackTarget(null)}
        className={cn(
          buttonVariants({ variant: 'default', size: 'lg' }),
          'fixed right-10 bottom-4 z-50 h-12 gap-2 px-4 text-sm shadow-xl',
          !showScrollTop && 'hidden',
        )}
        aria-label={backTarget?.label ?? 'Top'}
        title={backTarget?.label ?? 'Top'}
      >
        {backTarget ? <ArrowLeftIcon className="size-5" /> : <ArrowUpIcon className="size-5" />}
        <span>{backTarget?.label ?? 'Top'}</span>
      </a>
    </div>
  )
}

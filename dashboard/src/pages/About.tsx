import ReactMarkdown, { type Components } from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { Badge } from '@/components/ui/badge'
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import { Separator } from '@/components/ui/separator'
import { ABOUT_DOCS, ABOUT_REPO_URL, resolveAboutHref } from '@/lib/about-docs'

const markdownComponents: Components = {
  h1: ({ children }) => (
    <h1 className="text-foreground text-2xl leading-tight font-semibold tracking-tight">
      {children}
    </h1>
  ),
  h2: ({ children }) => (
    <h2 className="text-foreground mt-8 mb-3 text-lg leading-tight font-semibold tracking-tight first:mt-0">
      {children}
    </h2>
  ),
  h3: ({ children }) => (
    <h3 className="text-foreground mt-6 mb-2 text-base leading-tight font-semibold tracking-tight">
      {children}
    </h3>
  ),
  p: ({ children }) => <p className="text-muted-foreground leading-7">{children}</p>,
  ul: ({ children }) => (
    <ul className="text-muted-foreground my-4 list-disc space-y-2 pl-6">{children}</ul>
  ),
  ol: ({ children }) => (
    <ol className="text-muted-foreground my-4 list-decimal space-y-2 pl-6">{children}</ol>
  ),
  li: ({ children }) => <li className="leading-7">{children}</li>,
  blockquote: ({ children }) => (
    <blockquote className="border-border text-muted-foreground my-4 border-l-2 pl-4 italic">
      {children}
    </blockquote>
  ),
  hr: () => <hr className="border-border my-6 border-t" />,
  a: ({ href, children }) => {
    const resolvedHref = href ? resolveAboutHref('', href) : href
    const safeExternal =
      !!resolvedHref &&
      (resolvedHref.startsWith('http://') ||
        resolvedHref.startsWith('https://') ||
        resolvedHref.startsWith('//'))

    return (
      <a
        href={resolvedHref}
        target={safeExternal ? '_blank' : undefined}
        rel={safeExternal ? 'noopener noreferrer' : undefined}
        className="text-foreground decoration-border hover:decoration-foreground underline underline-offset-4"
      >
        {children}
      </a>
    )
  },
  code: ({ children, className }) => {
    const inline = !className
    return (
      <code
        className={
          inline
            ? 'bg-muted text-foreground rounded px-1.5 py-0.5 font-mono text-[0.85em]'
            : 'text-foreground font-mono text-[0.85em]'
        }
      >
        {children}
      </code>
    )
  },
  pre: ({ children }) => (
    <pre className="bg-muted text-foreground my-4 overflow-x-auto rounded-lg border p-4 text-sm leading-6">
      {children}
    </pre>
  ),
  table: ({ children }) => (
    <div className="my-4 overflow-x-auto">
      <table className="text-muted-foreground w-full border-collapse text-left text-sm">
        {children}
      </table>
    </div>
  ),
  thead: ({ children }) => <thead className="border-border border-b">{children}</thead>,
  tbody: ({ children }) => <tbody>{children}</tbody>,
  tr: ({ children }) => <tr className="border-border border-b last:border-b-0">{children}</tr>,
  th: ({ children }) => <th className="text-foreground py-3 pr-4 font-semibold">{children}</th>,
  td: ({ children }) => <td className="py-3 pr-4 align-top">{children}</td>,
}

export default function About() {
  return (
    <div className="mx-auto grid max-w-5xl gap-5">
      <Card className="overflow-hidden">
        <CardHeader className="space-y-4">
          <div className="flex flex-wrap items-center gap-2">
            <Badge variant="secondary">ESP32</Badge>
            <Badge variant="secondary">MQTT</Badge>
            <Badge variant="secondary">Telegram</Badge>
            <Badge variant="secondary">Wake-on-LAN</Badge>
            <Badge variant="secondary">Grafana</Badge>
          </div>
          <div className="grid gap-2">
            <CardTitle className="text-2xl">About this project</CardTitle>
            <p className="text-muted-foreground text-sm leading-6">
              <span className="text-foreground font-medium">esp32-pc-remote</span> is a private PC
              control stack: an ESP32 firmware plus a React dashboard for waking, checking, and
              managing a desktop over MQTT or Telegram.
            </p>
          </div>
        </CardHeader>
        <CardContent className="grid gap-3 text-sm leading-6">
          <ul className="text-muted-foreground list-disc space-y-2 pl-6">
            <li>Wake your PC with confirmation or force mode.</li>
            <li>Check online status, ping health, and reboot the controller.</li>
            <li>Use MQTT over WebSocket for the dashboard, or Telegram for command control.</li>
            <li>See the setup notes for Wake-on-LAN, MQTT access control, and optional logging.</li>
          </ul>
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
        <Card key={doc.sourcePath} className="overflow-hidden">
          <CardHeader className="space-y-2">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <CardTitle className="text-xl">{doc.title}</CardTitle>
              <Badge variant="secondary">{doc.sourcePath}</Badge>
            </div>
            <p className="text-muted-foreground text-xs">
              Rendered from the committed markdown at build time.
            </p>
          </CardHeader>
          <Separator />
          <CardContent className="pt-6">
            <article className="space-y-4">
              <ReactMarkdown
                remarkPlugins={[remarkGfm]}
                components={{
                  ...markdownComponents,
                  a: ({ href, children }) => {
                    const resolvedHref = href ? resolveAboutHref(doc.sourceDir, href) : href
                    const safeExternal =
                      !!resolvedHref &&
                      (resolvedHref.startsWith('http://') ||
                        resolvedHref.startsWith('https://') ||
                        resolvedHref.startsWith('//'))

                    return (
                      <a
                        href={resolvedHref}
                        target={safeExternal ? '_blank' : undefined}
                        rel={safeExternal ? 'noopener noreferrer' : undefined}
                        className="text-foreground decoration-border hover:decoration-foreground underline underline-offset-4"
                      >
                        {children}
                      </a>
                    )
                  },
                }}
              >
                {doc.markdown}
              </ReactMarkdown>
            </article>
          </CardContent>
        </Card>
      ))}
    </div>
  )
}

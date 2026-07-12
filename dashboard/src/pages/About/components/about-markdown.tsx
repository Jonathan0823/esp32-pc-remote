import { type ReactNode, isValidElement } from 'react'
import ReactMarkdown, { type Components } from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { Badge } from '@/components/ui/badge'
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import { Separator } from '@/components/ui/separator'
import { resolveAboutHref } from '@/lib/about-docs'
import { MermaidBlock } from '@/pages/About/components/mermaid-block'

const githubMarkdownFont =
  'ui-sans-serif, -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif, "Apple Color Emoji", "Segoe UI Emoji"'

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
    if (className === 'language-mermaid') {
      return <MermaidBlock diagram={children as string} />
    }
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
  pre: ({ children }) => {
    const child = Array.isArray(children) ? children[0] : children
    if (isValidElement(child) && child.type === MermaidBlock) {
      return child
    }
    return (
      <pre className="bg-muted text-foreground my-4 overflow-x-auto rounded-lg border p-4 text-sm leading-6">
        {children}
      </pre>
    )
  },
  img: ({ src, alt }) => {
    if (!src) return null
    let resolved = src
    if (!src.startsWith('http://') && !src.startsWith('https://') && !src.startsWith('//')) {
      const normalized = resolveAboutHref('', src)
      if (normalized.startsWith('http')) {
        resolved = normalized
          .replace('https://github.com/', 'https://raw.githubusercontent.com/')
          .replace('/blob/', '/')
      }
    }
    const isBadge = resolved.includes('shields.io')
    return (
      <img
        src={resolved}
        alt={alt || ''}
        className={isBadge ? 'inline h-5 align-middle' : 'my-4 h-auto max-w-full rounded-lg'}
      />
    )
  },
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

type MarkdownLinkProps = Readonly<{
  href?: string
  children?: ReactNode
  sourceDir: string
  onLocalLinkClick: (href: string) => void
}>

function MarkdownLink({ href, children, sourceDir, onLocalLinkClick }: MarkdownLinkProps) {
  const resolvedHref = href ? resolveAboutHref(sourceDir, href) : href
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
      onClick={() => {
        if (resolvedHref?.startsWith('#')) onLocalLinkClick(resolvedHref)
      }}
      className="text-foreground decoration-border hover:decoration-foreground underline underline-offset-4"
    >
      {children}
    </a>
  )
}

function createMarkdownComponents(
  sourceDir: string,
  onLocalLinkClick: (href: string) => void,
): Components {
  return {
    ...markdownComponents,
    a: (props) => (
      <MarkdownLink {...props} sourceDir={sourceDir} onLocalLinkClick={onLocalLinkClick} />
    ),
  }
}

export function DocSection({
  title,
  sourcePath,
  slug,
  sourceDir,
  markdown,
  onLocalLinkClick,
}: Readonly<{
  title: string
  sourcePath: string
  slug: string
  sourceDir: string
  markdown: string
  onLocalLinkClick: (href: string) => void
}>) {
  return (
    <Card id={slug} className="scroll-mt-6 overflow-hidden">
      <CardHeader className="space-y-2 pb-2">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <CardTitle className="text-xl">{title}</CardTitle>
          <Badge variant="secondary">{sourcePath}</Badge>
        </div>
      </CardHeader>
      <Separator />
      <CardContent className="pt-6">
        <article
          className="space-y-4 text-[15px] leading-7"
          style={{ fontFamily: githubMarkdownFont }}
        >
          <ReactMarkdown
            remarkPlugins={[remarkGfm]}
            components={createMarkdownComponents(sourceDir, onLocalLinkClick)}
          >
            {markdown}
          </ReactMarkdown>
        </article>
      </CardContent>
    </Card>
  )
}

export const ABOUT_REPO_URL = 'https://github.com/Jonathan0823/esp32-pc-remote'
export const ABOUT_REPO_BLOB_URL = `${ABOUT_REPO_URL}/blob/main`

export type AboutDoc = {
  title: string
  sourcePath: string
  sourceDir: string
  slug: string
  markdown: string
}

import rootReadme from '../../../README.md?raw'
import dashboardReadme from '../../README.md?raw'
import grafanaDoc from '../../../docs/grafana.md?raw'
import telegramDoc from '../../../docs/telegram.md?raw'
import wakeOnLanDoc from '../../../docs/wake-on-lan.md?raw'

export const ABOUT_DOCS: AboutDoc[] = [
  {
    title: 'Repository README',
    sourcePath: 'README.md',
    sourceDir: '',
    slug: 'repository-readme',
    markdown: rootReadme,
  },
  {
    title: 'Dashboard README',
    sourcePath: 'dashboard/README.md',
    sourceDir: 'dashboard',
    slug: 'dashboard-readme',
    markdown: dashboardReadme,
  },
  {
    title: 'Wake-on-LAN setup',
    sourcePath: 'docs/wake-on-lan.md',
    sourceDir: 'docs',
    slug: 'wake-on-lan-setup',
    markdown: wakeOnLanDoc,
  },
  {
    title: 'Telegram bot setup',
    sourcePath: 'docs/telegram.md',
    sourceDir: 'docs',
    slug: 'telegram-bot-setup',
    markdown: telegramDoc,
  },
  {
    title: 'Grafana Cloud logging',
    sourcePath: 'docs/grafana.md',
    sourceDir: 'docs',
    slug: 'grafana-cloud-logging',
    markdown: grafanaDoc,
  },
]

const ABOUT_DOC_PATH_TO_SLUG = new Map(ABOUT_DOCS.map((doc) => [doc.sourcePath, doc.slug]))

function normalizeRelativePath(baseDir: string, relativePath: string) {
  const parts = `${baseDir}/${relativePath}`.split('/')
  const stack: string[] = []

  for (const part of parts) {
    if (!part || part === '.') continue
    if (part === '..') {
      stack.pop()
      continue
    }
    stack.push(part)
  }

  return stack.join('/')
}

export function resolveAboutHref(sourceDir: string, href: string) {
  if (!href || href.startsWith('#') || href.startsWith('//') || /^[a-z][a-z0-9+.-]*:/i.test(href)) {
    return href
  }

  const normalizedPath = normalizeRelativePath(sourceDir, href.replace(/^\//, ''))
  const slug = ABOUT_DOC_PATH_TO_SLUG.get(normalizedPath)
  if (slug) return `#${slug}`

  return `${ABOUT_REPO_BLOB_URL}/${normalizedPath}`
}

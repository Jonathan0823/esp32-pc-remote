export const ABOUT_REPO_URL = 'https://github.com/Jonathan0823/esp32-pc-remote'
export const ABOUT_REPO_BLOB_URL = `${ABOUT_REPO_URL}/blob/main`

export type AboutDoc = {
  title: string
  sourcePath: string
  sourceDir: string
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
    markdown: rootReadme,
  },
  {
    title: 'Dashboard README',
    sourcePath: 'dashboard/README.md',
    sourceDir: 'dashboard',
    markdown: dashboardReadme,
  },
  {
    title: 'Wake-on-LAN setup',
    sourcePath: 'docs/wake-on-lan.md',
    sourceDir: 'docs',
    markdown: wakeOnLanDoc,
  },
  {
    title: 'Telegram bot setup',
    sourcePath: 'docs/telegram.md',
    sourceDir: 'docs',
    markdown: telegramDoc,
  },
  {
    title: 'Grafana Cloud logging',
    sourcePath: 'docs/grafana.md',
    sourceDir: 'docs',
    markdown: grafanaDoc,
  },
]

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
  return `${ABOUT_REPO_BLOB_URL}/${normalizedPath}`
}

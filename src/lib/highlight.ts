// Shiki-powered syntax highlighting for code blocks
// Singleton highlighter with curated language set + dual theme (light/dark).

import { createHighlighter, type Highlighter, type BundledLanguage } from 'shiki'

const LANGS: BundledLanguage[] = [
  'rust', 'typescript', 'tsx', 'javascript', 'jsx', 'jsx',
  'python', 'go', 'java', 'kotlin', 'swift', 'c', 'cpp', 'csharp',
  'html', 'css', 'scss', 'json', 'yaml', 'toml', 'xml',
  'bash', 'shell', 'powershell', 'sql', 'dockerfile', 'graphql',
  'markdown', 'diff', 'ini',
]

const THEMES = ['github-light', 'github-dark'] as const

let highlighterPromise: Promise<Highlighter> | null = null

export function getHighlighter(): Promise<Highlighter> {
  if (!highlighterPromise) {
    highlighterPromise = createHighlighter({
      themes: [...THEMES],
      langs: LANGS,
    })
  }
  return highlighterPromise
}

const LANG_ALIAS: Record<string, string> = {
  ts: 'typescript',
  js: 'javascript',
  py: 'python',
  rs: 'rust',
  sh: 'bash',
  shell: 'bash',
  yml: 'yaml',
  vue: 'html',
  md: 'markdown',
  'c++': 'cpp',
  cs: 'csharp',
}

export function normalizeLang(lang: string): string {
  const l = lang.toLowerCase().trim()
  return LANG_ALIAS[l] ?? l
}

export function isSupportedLang(lang: string): boolean {
  const n = normalizeLang(lang)
  return (LANGS as string[]).includes(n)
}

/**
 * Render a code block to HTML using Shiki with dual-theme.
 * Returns the HTML string. Falls back to a plain <pre><code> if the
 * language is unknown.
 */
export async function renderCode(code: string, lang: string): Promise<string> {
  const normalized = normalizeLang(lang)
  const hl = await getHighlighter()
  if (!isSupportedLang(lang)) {
    const escaped = code
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
    return `<pre class="md-code-fallback"><code class="language-${lang}">${escaped}</code></pre>`
  }
  return hl.codeToHtml(code, {
    lang: normalized,
    themes: {
      light: 'github-light',
      dark: 'github-dark',
    },
    defaultColor: false, // emit both themes in CSS vars
  })
}

#!/usr/bin/env node
// NoteVault Static Site Generator (Quartz-style)
// Usage:
//   node scripts/static-build.mjs --vault <path> --out <path> [--base /repo-name/]
//
// What it does:
//   1. Walk <vault> for .md files (excluding .config/, index.sqlite, .git/)
//   2. Parse YAML frontmatter
//   3. Render markdown -> HTML (markdown-it + shiki + mermaid placeholder)
//   4. Resolve [[wikilink]] and ((block-ref)) to relative HTML
//   5. Generate per-note HTML pages
//   6. Generate index.html, tag pages, RSS, sitemap, search index
//   7. Apply embedded CSS template (no external deps, GitHub Pages friendly)

import fs from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'
import { fileURLToPath } from 'node:url'
import fg from 'fast-glob'
import MarkdownIt from 'markdown-it'
import markdownItAnchor from 'markdown-it-anchor'
import { createHighlighter } from 'shiki'
import yaml from 'yaml'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const TEMPLATE_PATH = path.join(__dirname, 'static-template.html')

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

function parseArgs() {
  const args = { vault: null, out: null, base: '/' }
  for (let i = 2; i < process.argv.length; i++) {
    const a = process.argv[i]
    if (a === '--vault' || a === '-v') args.vault = process.argv[++i]
    else if (a === '--out' || a === '-o') args.out = process.argv[++i]
    else if (a === '--base' || a === '-b') args.base = process.argv[++i]
    else if (a === '--help' || a === '-h') {
      console.log('Usage: node static-build.mjs --vault <path> --out <path> [--base /repo/]')
      process.exit(0)
    }
  }
  if (!args.vault || !args.out) {
    console.error('ERROR: --vault and --out are required')
    process.exit(1)
  }
  return args
}

// ---------------------------------------------------------------------------
// Markdown rendering (same look as live app: wikilink + block_ref + shiki)
// ---------------------------------------------------------------------------

const LANG_ALIAS = {
  ts: 'typescript', js: 'javascript', py: 'python', rs: 'rust',
  sh: 'bash', shell: 'bash', yml: 'yaml', vue: 'html', md: 'markdown',
  'c++': 'cpp', cs: 'csharp',
}
const LANGS = [
  'rust', 'typescript', 'tsx', 'javascript', 'jsx',
  'python', 'go', 'java', 'kotlin', 'swift', 'c', 'cpp', 'csharp',
  'html', 'css', 'scss', 'json', 'yaml', 'toml', 'xml',
  'bash', 'shell', 'powershell', 'sql', 'dockerfile', 'graphql',
  'markdown', 'diff', 'ini',
]
const THEMES = ['github-light', 'github-dark']

const WIKILINK_RE = /\[\[([^\[\]\|]+?)(?:\|([^\]]+?))?\]\]/g
const EMBED_RE = /!\[\[([^\[\]\|]+?)(?:\|([^\]]+?))?\]\]/g
const BLOCK_REF_RE = /\(\((blk_[a-z0-9]+)\)\)/g

function buildMarkdown(resolveLink) {
  const md = new MarkdownIt({
    html: false,
    xhtmlOut: false,
    breaks: true,
    linkify: true,
    typographer: true,
  })
  md.use(markdownItAnchor, {
    permalink: markdownItAnchor.permalink.linkInsideHeader({
      symbol: '#',
      placement: 'before',
      ariaHidden: true,
    }),
    slugify: (s) =>
      s
        .toLowerCase()
        .replace(/\s+/g, '-')
        .replace(/[^\w\u4e00-\u9fa5-]/g, '')
        .replace(/-+/g, '-')
        .replace(/^-|-$/g, ''),
  })

  md.inline.ruler.before('emphasis', 'wikilink', (state, silent) => {
    if (state.src.charCodeAt(state.pos) !== 0x5b) return false
    if (state.src.charCodeAt(state.pos + 1) !== 0x5b) return false
    const re = new RegExp(WIKILINK_RE.source, 'g')
    const m = re.exec(state.src.slice(state.pos))
    if (!m) return false
    if (silent) return true
    const target = (m[1] ?? '').trim()
    const alias = (m[2] ?? '').trim() || target
    const token = state.push('wikilink', '', 0)
    token.content = alias
    token.info = target
    state.pos += m[0].length
    return true
  })
  md.renderer.rules.wikilink = (tokens, idx) => {
    const t = tokens[idx]
    const target = t.info
    const label = md.utils.escapeHtml(t.content)
    const href = resolveLink(target)
    const cls = href ? 'wikilink resolved' : 'wikilink broken'
    const dataAttrs = href
      ? `data-resolved="${md.utils.escapeHtml(href)}"`
      : `data-broken="${md.utils.escapeHtml(target)}"`
    return `<a class="${cls}" ${dataAttrs} href="${md.utils.escapeHtml(href || '#')}">${label}</a>`
  }

  // Embed / transclusion: ![[note]] / ![[note#section]]
  md.inline.ruler.before('emphasis', 'embed', (state, silent) => {
    if (state.src.charCodeAt(state.pos) !== 0x21 /* ! */) return false
    if (state.src.charCodeAt(state.pos + 1) !== 0x5b) return false
    if (state.src.charCodeAt(state.pos + 2) !== 0x5b) return false
    const re = new RegExp(EMBED_RE.source, 'g')
    const m = re.exec(state.src.slice(state.pos))
    if (!m) return false
    if (silent) return true
    const target = (m[1] ?? '').trim()
    const token = state.push('embed', '', 0)
    token.content = target
    state.pos += m[0].length
    return true
  })
  md.renderer.rules.embed = (tokens, idx) => {
    const t = tokens[idx]
    const target = t.content
    // Placeholder — replaced in post-pass after all notes are rendered.
    // We base64-encode the target to avoid HTML escaping issues.
    const enc = Buffer.from(target, 'utf-8').toString('base64')
    return `<!--TRANSCLUDE:${enc}-->`
  }

  md.inline.ruler.before('emphasis', 'block_ref', (state, silent) => {
    if (state.src.charCodeAt(state.pos) !== 0x28) return false
    if (state.src.charCodeAt(state.pos + 1) !== 0x28) return false
    const re = new RegExp(BLOCK_REF_RE.source, 'g')
    const m = re.exec(state.src.slice(state.pos))
    if (!m) return false
    if (silent) return true
    const target = m[1]
    const token = state.push('block_ref', '', 0)
    token.content = target
    state.pos += m[0].length
    return true
  })
  md.renderer.rules.block_ref = (tokens, idx) => {
    const t = tokens[idx]
    const target = t.content
    const short = target.replace(/^blk_/, '#')
    return `<a class="block-ref" href="#${md.utils.escapeHtml(target)}" title="块引用 ${md.utils.escapeHtml(target)}">${short}</a>`
  }

  // Override fence renderer to defer shiki to batch pass (we need async).
  // For now, emit placeholder; we'll replace in a post-pass.
  md.renderer.rules.fence = (tokens, idx) => {
    const token = tokens[idx]
    const info = (token.info || '').trim()
    const code = token.content
    const safe = code.replace(/&/g, '&amp;').replace(/"/g, '&quot;')
    if (info === 'mermaid') {
      return `<div class="mermaid-block" data-source="${safe}"></div>`
    }
    return `<pre class="md-code-block" data-lang="${info || 'text'}" data-source="${safe}"><code></code></pre>`
  }

  return md
}

// ---------------------------------------------------------------------------
// Frontmatter parsing
// ---------------------------------------------------------------------------

function parseFrontmatter(raw) {
  if (!raw.startsWith('---')) return { fm: {}, body: raw }
  // Find closing --- on its own line
  const rest = raw.slice(3)
  const end = rest.indexOf('\n---')
  if (end === -1) return { fm: {}, body: raw }
  const yamlText = rest.slice(0, end)
  const body = rest.slice(end + 4).replace(/^\n/, '')
  try {
    const fm = yaml.parse(yamlText) || {}
    return { fm, body }
  } catch (e) {
    console.warn('Frontmatter parse error:', e.message)
    return { fm: {}, body: raw }
  }
}

// ---------------------------------------------------------------------------
// Wikilink resolution
// ---------------------------------------------------------------------------

function slugForNotePath(relPath) {
  // notes/foo.md -> foo
  // topics/rust/ownership.md -> topics/rust/ownership (keep hierarchy)
  return relPath.replace(/\.md$/, '').replace(/\\/g, '/')
}

function urlForNotePath(relPath, base) {
  const slug = slugForNotePath(relPath)
  // if it's in 0-inbox/ we want to expose without the prefix
  const clean = slug.replace(/^\d+-/, (m) => m)
  return base + clean + '.html'
}

function buildResolver(allNotes) {
  // Build a map: title/path/alias -> relative out path
  const byPath = new Map()    // "rust/ownership" -> "rust/ownership.html"
  const byTitle = new Map()   // "Rust 所有权" -> "rust/ownership.html"
  const byBasename = new Map()// "ownership" -> "rust/ownership.html"
  for (const n of allNotes) {
    const slug = slugForNotePath(n.relPath)
    const out = slug + '.html'
    byPath.set(slug, out)
    byPath.set(slug.toLowerCase(), out)
    if (n.fm.title) {
      byPath.set(n.fm.title, out)
      byTitle.set(n.fm.title, out)
      byTitle.set(n.fm.title.toLowerCase(), out)
    }
    if (Array.isArray(n.fm.aliases)) {
      for (const a of n.fm.aliases) {
        byTitle.set(a, out)
        byTitle.set(a.toLowerCase(), out)
      }
    }
    const base = path.basename(slug)
    if (!byBasename.has(base)) byBasename.set(base, out)
  }
  return (target) => {
    // Strip #section
    let [notePart, anchor] = target.split('#', 2)
    const norm = (s) => s.replace(/\\/g, '/').replace(/\.md$/, '')
    // Try: exact path, basename, title
    let out =
      byPath.get(norm(notePart)) ||
      byPath.get(norm(notePart).toLowerCase()) ||
      byTitle.get(notePart) ||
      byTitle.get(notePart.toLowerCase()) ||
      byBasename.get(path.basename(norm(notePart)))
    if (!out) return null
    return out + (anchor ? '#' + anchor : '')
  }
}

// ---------------------------------------------------------------------------
// Transclusion: replace <!--TRANSCLUDE:base64(target)--> with the rendered
// HTML of the target note (or its section). Recursion is capped at 3 levels
// to avoid cycles.
// ---------------------------------------------------------------------------

function resolveTransclusions(html, notes, originNote, depth = 0) {
  if (depth >= 3) return html
  const re = /<!--TRANSCLUDE:([A-Za-z0-9+/=]+)-->/g
  const matches = [...html.matchAll(re)]
  if (matches.length === 0) return html

  // Build a quick lookup: title / path / basename -> note
  const lookup = new Map()
  for (const n of notes) {
    const slug = slugForNotePath(n.relPath)
    if (!lookup.has(slug)) lookup.set(slug, n)
    if (!lookup.has(path.basename(slug))) lookup.set(path.basename(slug), n)
    if (n.fm.title && !lookup.has(n.fm.title)) lookup.set(n.fm.title, n)
    if (n.fm.title) {
      const k = n.fm.title.toLowerCase()
      if (!lookup.has(k)) lookup.set(k, n)
    }
  }

  let out = html
  for (const m of matches) {
    const target = Buffer.from(m[1], 'base64').toString('utf-8')
    const [name, anchor] = target.split('#', 2)
    const norm = name.replace(/\\/g, '/').replace(/\.md$/, '')
    const note =
      lookup.get(norm) ||
      lookup.get(path.basename(norm)) ||
      lookup.get(norm.toLowerCase())
    let replacement
    if (!note) {
      replacement = `<div class="transclude transclude-missing">⚠️ 嵌入失败: <code>${escapeHtml(target)}</code> 找不到</div>`
    } else {
      // Compute relative URL from originNote to target note
      const fromDir = path.posix.dirname(slugForNotePath(originNote.relPath)) || ''
      const toPath = slugForNotePath(note.relPath) + '.html'
      const relUrl = posixRelative(fromDir, toPath) + (anchor ? '#' + anchor : '')
      const body = extractSection(note.html, anchor) || note.html
      const inner = resolveTransclusions(body, notes, note, depth + 1)
      replacement = `<aside class="transclude">
        <header class="transclude-header">
          <span>📄 嵌入自</span>
          <a class="transclude-link" href="${escapeHtml(relUrl)}">${escapeHtml(note.title)}</a>
        </header>
        <div class="transclude-body">${inner}</div>
      </aside>`
    }
    out = out.replace(m[0], replacement)
  }
  return out
}

function posixRelative(fromDir, toPath) {
  // Compute relative path from fromDir to toPath, both posix-style.
  if (!fromDir) return toPath
  const fromParts = fromDir.split('/').filter(Boolean)
  const toParts = toPath.split('/')
  let i = 0
  while (i < fromParts.length && i < toParts.length - 1 && fromParts[i] === toParts[i]) i++
  const ups = fromParts.length - i
  const rest = toParts.slice(i)
  return (ups > 0 ? '../'.repeat(ups) : '') + rest.join('/')
}

/**
 * Pull a section of rendered HTML by heading id, stopping at the next same-or-higher
 * level heading. Returns the slice between them. If no matching heading, returns null.
 */
function extractSection(html, anchor) {
  if (!anchor) return null
  const id = anchor
  // Find heading element with this id
  const re = new RegExp(`<h([1-6])\\s+id=["']${escapeRegex(id)}["']`, 'i')
  const m = re.exec(html)
  if (!m) return null
  const startLevel = parseInt(m[1], 10)
  const startIdx = m.index
  // Find the next heading with level <= startLevel
  const tail = html.slice(startIdx + m[0].length)
  const stopRe = new RegExp(`<h[1-${startLevel}]\\s+id=`, 'i')
  const stopM = stopRe.exec(tail)
  const endIdx = stopM ? startIdx + m[0].length + stopM.index : html.length
  return html.slice(startIdx, endIdx)
}

function escapeRegex(s) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

// ---------------------------------------------------------------------------
// Shiki batch highlighting
// ---------------------------------------------------------------------------

async function highlightCodeBlocks(html) {
  const re = /<pre class="md-code-block" data-lang="([^"]*)" data-source="([^"]*)"><code><\/code><\/pre>/g
  const matches = [...html.matchAll(re)]
  if (matches.length === 0) return html

  const hl = await createHighlighter({ themes: [...THEMES], langs: LANGS })
  const replacements = []
  for (const m of matches) {
    const lang = (m[1] || 'text').toLowerCase()
    const src = m[2]
        .replace(/&quot;/g, '"')
        .replace(/&amp;/g, '&')
        .replace(/&lt;/g, '<')
        .replace(/&gt;/g, '>')
    const norm = LANG_ALIAS[lang] ?? lang
    let rendered
    if ((LANGS).includes(norm)) {
      try {
        rendered = hl.codeToHtml(src, {
          lang: norm,
          themes: { light: 'github-light', dark: 'github-dark' },
          defaultColor: false,
        })
      } catch (e) {
        rendered = `<pre class="md-code-fallback"><code class="language-${lang}">${escapeHtml(src)}</code></pre>`
      }
    } else {
      rendered = `<pre class="md-code-fallback"><code class="language-${lang}">${escapeHtml(src)}</code></pre>`
    }
    // Wrap in a div for toolbar (no toolbar in static for now, keep simple)
    replacements.push({ from: m[0], to: rendered })
  }
  let out = html
  for (const r of replacements) out = out.replace(r.from, r.to)
  return out
}

function escapeHtml(s) {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
}

// ---------------------------------------------------------------------------
// HTML template
// ---------------------------------------------------------------------------

async function readTemplate() {
  return await fs.readFile(TEMPLATE_PATH, 'utf-8')
}

function applyTemplate(tpl, vars) {
  return tpl
    .replace(/\{\{TITLE\}\}/g, () => escapeHtml(vars.title))
    .replace(/\{\{DESCRIPTION\}\}/g, () => escapeHtml(vars.description || ''))
    .replace(/\{\{CONTENT\}\}/g, () => vars.content)
    .replace(/\{\{BASE\}\}/g, () => vars.base)
    .replace(/\{\{GENERATED_AT\}\}/g, () => vars.generatedAt)
    .replace(/\{\{VAULT_NAME\}\}/g, () => escapeHtml(vars.vaultName))
    .replace(/\{\{EXTRA_HEAD\}\}/g, () => vars.extraHead || '')
    .replace(/\{\{EXTRA_BODY\}\}/g, () => vars.extraBody || '')
}

// ---------------------------------------------------------------------------
// Note walking
// ---------------------------------------------------------------------------

async function loadAllNotes(vaultPath) {
  const files = await fg('**/*.md', {
    cwd: vaultPath,
    ignore: ['.git/**', '.config/**', 'index.sqlite', '**/.DS_Store'],
    dot: false,
    onlyFiles: true,
  })
  const notes = []
  for (const rel of files) {
    const abs = path.join(vaultPath, rel)
    const raw = await fs.readFile(abs, 'utf-8')
    const { fm, body } = parseFrontmatter(raw)
    const title =
      fm.title || path.basename(rel, '.md')
    const stat = await fs.stat(abs)
    notes.push({
      relPath: rel.replace(/\\/g, '/'),
      absPath: abs,
      raw,
      body,
      fm,
      title,
      tags: Array.isArray(fm.tags) ? fm.tags : [],
      mtime: stat.mtime,
      // Will be computed later
      html: null,
      outPath: null,
    })
  }
  return notes
}

// ---------------------------------------------------------------------------
// Page generation
// ---------------------------------------------------------------------------

function noteListItem(n, base) {
  const url = urlForNotePath(n.relPath, base)
  const tags = (n.tags || [])
    .map((t) => `<span class="tag">#${escapeHtml(t)}</span>`)
    .join(' ')
  const desc = n.fm.description ? `<div class="note-desc">${escapeHtml(n.fm.description)}</div>` : ''
  return `<li class="note-item">
    <a class="note-link" href="${escapeHtml(url)}">${escapeHtml(n.title)}</a>
    <div class="note-meta">${escapeHtml(n.relPath)} ${tags ? '· ' + tags : ''}</div>
    ${desc}
  </li>`
}

function buildIndex(notes, base, vaultName, generatedAt) {
  const items = notes
    .slice()
    .sort((a, b) => b.mtime - a.mtime)
    .map((n) => noteListItem(n, base))
    .join('\n')
  const tags = new Set()
  for (const n of notes) for (const t of n.tags || []) tags.add(t)
  const tagList = [...tags]
    .sort()
    .map((t) => `<a class="tag-chip" href="${base}tags/${encodeURIComponent(t)}.html">#${escapeHtml(t)}</a>`)
    .join(' ')
  return `<div class="index-page">
    <header class="page-header">
      <h1>${escapeHtml(vaultName)}</h1>
      <p class="lead">共 ${notes.length} 篇笔记 · ${tags.size} 个标签</p>
      <div class="tag-cloud">${tagList}</div>
    </header>
    <ul class="note-list">${items}</ul>
  </div>`
}

function buildTagPage(tag, notes, base, vaultName) {
  const items = notes
    .filter((n) => (n.tags || []).includes(tag))
    .map((n) => noteListItem(n, base))
    .join('\n')
  return `<div class="tag-page">
    <header class="page-header">
      <p class="crumb"><a href="${base}">← ${escapeHtml(vaultName)}</a></p>
      <h1>#${escapeHtml(tag)}</h1>
      <p class="lead">${notes.filter((n) => (n.tags || []).includes(tag)).length} 篇笔记</p>
    </header>
    <ul class="note-list">${items}</ul>
  </div>`
}

function buildSearchIndex(notes, base) {
  // Items used by the fuse.js client-side search.
  const items = notes.map((n) => ({
    title: n.title,
    url: urlForNotePath(n.relPath, base),
    path: n.relPath,
    tags: (n.tags || []).join(' '),
    body: stripMd(n.body).slice(0, 4000),
  }))
  return `window.__NOTEVAULT_SEARCH__ = ${JSON.stringify(items)};`
}

function stripMd(s) {
  return s
    .replace(/^---[\s\S]*?---\n/, '')
    .replace(/```[\s\S]*?```/g, '')
    .replace(/`[^`]+`/g, '')
    .replace(/\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g, (_, a, b) => b || a)
    .replace(/!\[[^\]]*\]\([^)]+\)/g, '')
    .replace(/\[([^\]]+)\]\([^)]+\)/g, '$1')
    .replace(/[#>*_~`]/g, '')
    .replace(/\s+/g, ' ')
    .trim()
}

/**
 * Build a 1200x630 SVG OG image. Modern Twitter / LinkedIn / Slack / Discord
 * render these. (Facebook prefers PNG/JPG; if you target FB, swap to PNG via
 * a headless browser or sharp.)
 */
function ogImageSvg({ title, subtitle, tags }) {
  const safeTitle = String(title || '').slice(0, 100)
  const safeSub = String(subtitle || '').slice(0, 140)
  // Word-wrap title to ~16 chars/line
  const wrapped = wrapText(safeTitle, 14)
  const titleY = 270
  const tagY = 480
  const tagsHtml = (tags || [])
    .slice(0, 4)
    .map(
      (t) =>
        `<g transform="translate(${100 + (t.length > 6 ? 90 : 60)} ${tagY})">
          <rect x="0" y="0" rx="20" ry="20" width="${t.length * 16 + 28}" height="40" fill="#ddf4ff" />
          <text x="${(t.length * 16 + 28) / 2}" y="26" font-family="-apple-system,Segoe UI,sans-serif" font-size="20" font-weight="600" fill="#0969da" text-anchor="middle">#${escapeXml(t)}</text>
        </g>`,
    )
    .join('')
  return `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="630" viewBox="0 0 1200 630">
  <defs>
    <linearGradient id="bg" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#0d1117"/>
      <stop offset="100%" stop-color="#161b22"/>
    </linearGradient>
    <linearGradient id="accent" x1="0%" y1="0%" x2="100%" y2="0%">
      <stop offset="0%" stop-color="#58a6ff"/>
      <stop offset="100%" stop-color="#1f6feb"/>
    </linearGradient>
  </defs>
  <rect width="1200" height="630" fill="url(#bg)"/>
  <!-- top accent bar -->
  <rect x="80" y="80" width="120" height="6" fill="url(#accent)" rx="3"/>
  <!-- title -->
  ${wrapped
    .map(
      (line, i) =>
        `<text x="100" y="${titleY + i * 64}" font-family="-apple-system,Segoe UI,Helvetica,sans-serif" font-size="56" font-weight="700" fill="#e6edf3">${escapeXml(line)}</text>`,
    )
    .join('\n  ')}
  <!-- subtitle -->
  ${
    safeSub
      ? `<text x="100" y="${titleY + wrapped.length * 64 + 30}" font-family="-apple-system,Segoe UI,Helvetica,sans-serif" font-size="24" fill="#9198a1">${escapeXml(safeSub.slice(0, 80))}${safeSub.length > 80 ? '…' : ''}</text>`
      : ''
  }
  <!-- tags -->
  ${tagsHtml}
  <!-- brand at bottom -->
  <text x="100" y="570" font-family="-apple-system,Segoe UI,Helvetica,sans-serif" font-size="20" font-weight="600" fill="#58a6ff">📓 NoteVault</text>
  <text x="1100" y="570" text-anchor="end" font-family="-apple-system,Segoe UI,Helvetica,sans-serif" font-size="16" fill="#6e7681">${new Date().toISOString().slice(0, 10)}</text>
</svg>`
}

function wrapText(s, maxChars) {
  if (!s) return ['']
  // For CJK text, wrap by characters; for Latin, wrap by words.
  const out = []
  let cur = ''
  const isCjk = /[\u4e00-\u9fff]/.test(s)
  if (isCjk) {
    for (const ch of s) {
      if (cur.length >= maxChars) {
        out.push(cur)
        cur = ''
      }
      cur += ch
    }
  } else {
    const words = s.split(/\s+/)
    for (const w of words) {
      if (cur.length + w.length + 1 > maxChars && cur) {
        out.push(cur)
        cur = w
      } else {
        cur = cur ? cur + ' ' + w : w
      }
    }
  }
  if (cur) out.push(cur)
  return out.slice(0, 3) // max 3 lines
}

function escapeXml(s) {
  return String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&apos;')
}

function ogMetaTags({ title, description, url, image, type }) {
  return `
  <meta property="og:type" content="${escapeHtml(type || 'website')}" />
  <meta property="og:title" content="${escapeHtml(title)}" />
  <meta property="og:description" content="${escapeHtml(description || '')}" />
  <meta property="og:url" content="${escapeHtml(url)}" />
  <meta property="og:image" content="${escapeHtml(image)}" />
  <meta property="og:image:width" content="1200" />
  <meta property="og:image:height" content="630" />
  <meta name="twitter:card" content="summary_large_image" />
  <meta name="twitter:title" content="${escapeHtml(title)}" />
  <meta name="twitter:description" content="${escapeHtml(description || '')}" />
  <meta name="twitter:image" content="${escapeHtml(image)}" />`.trim()
}

function buildRss(notes, base, vaultName) {
  const recent = notes.slice().sort((a, b) => b.mtime - a.mtime).slice(0, 20)
  const items = recent
    .map((n) => {
      const url = base.replace(/\/$/, '') + urlForNotePath(n.relPath, '/')
      return `  <item>
    <title>${escapeHtml(n.title)}</title>
    <link>${escapeHtml(url)}</link>
    <guid>${escapeHtml(url)}</guid>
    <pubDate>${n.mtime.toUTCString()}</pubDate>
    <description>${escapeHtml(n.fm.description || n.body.slice(0, 200))}</description>
  </item>`
    })
    .join('\n')
  return `<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
<channel>
  <title>${escapeHtml(vaultName)}</title>
  <link>${escapeHtml(base)}</link>
  <description>${escapeHtml(vaultName)} — NoteVault 静态发布</description>
  <lastBuildDate>${new Date().toUTCString()}</lastBuildDate>
${items}
</channel>
</rss>`
}

function buildSitemap(notes, base) {
  const urls = notes
    .map((n) => {
      const u = base.replace(/\/$/, '') + urlForNotePath(n.relPath, '/')
      return `  <url>
    <loc>${escapeHtml(u)}</loc>
    <lastmod>${n.mtime.toISOString()}</lastmod>
  </url>`
    })
    .join('\n')
  return `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${urls}
</urlset>`
}

// ---------------------------------------------------------------------------
// Mermaid inline script (CDN with fallback)
// ---------------------------------------------------------------------------

function mermaidScript(base) {
  return `<script type="module">
  import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs';
  mermaid.initialize({ startOnLoad: true, theme: 'default', securityLevel: 'loose', fontFamily: 'inherit' });
  // Render our placeholder blocks
  for (const el of document.querySelectorAll('.mermaid-block[data-source]')) {
    const src = el.getAttribute('data-source');
    try {
      const { svg } = await mermaid.render('mmd-' + Math.random().toString(36).slice(2), src);
      el.innerHTML = svg;
      el.classList.add('mermaid-rendered');
    } catch (e) {
      el.innerHTML = '<pre class="mermaid-error">⚠️ ' + (e?.message || e) + '</pre>';
    }
  }
</script>`
}

function searchScript(base) {
  return `<script type="module">
import Fuse from 'https://cdn.jsdelivr.net/npm/fuse.js@7/dist/fuse.mjs';
(function () {
  const INDEX = window.__NOTEVAULT_SEARCH__ || [];
  const input = document.getElementById('search-input');
  const results = document.getElementById('search-results');
  const meta = document.getElementById('search-meta');
  if (!input) return;
  const fuse = new Fuse(INDEX, {
    keys: [
      { name: 'title', weight: 0.5 },
      { name: 'tags', weight: 0.2 },
      { name: 'path', weight: 0.1 },
      { name: 'body', weight: 0.2 },
    ],
    includeMatches: true,
    includeScore: true,
    threshold: 0.4,
    ignoreLocation: true,
    minMatchCharLength: 2,
  });
  function highlight(text, indices) {
    if (!indices || indices.length === 0) return text;
    const pairs = indices.slice().sort((a, b) => a[0] - b[0]);
    let out = '';
    let cursor = 0;
    for (const [s, e] of pairs) {
      if (s < cursor) continue; // overlapping
      out += text.slice(cursor, s);
      out += '<mark>' + text.slice(s, e + 1) + '</mark>';
      cursor = e + 1;
    }
    out += text.slice(cursor);
    return out;
  }
  function snippetAround(text, indices, len = 120) {
    if (!indices || indices.length === 0) return text.slice(0, len);
    const [s, e] = indices[0];
    const start = Math.max(0, s - 40);
    const end = Math.min(text.length, e + 1 + (len - (e - s + 1) - 40));
    return (start > 0 ? '…' : '') + text.slice(start, end) + (end < text.length ? '…' : '');
  }
  function run() {
    const q = input.value.trim();
    if (!q) {
      results.innerHTML = '<p class="empty">输入关键词开始搜索（fuzzy · 中文友好）</p>';
      if (meta) meta.textContent = '';
      return;
    }
    const hits = fuse.search(q, { limit: 30 });
    if (hits.length === 0) {
      results.innerHTML = '<p class="empty">没找到匹配</p>';
      if (meta) meta.textContent = '0 个结果';
      return;
    }
    if (meta) meta.textContent = hits.length + ' 个结果';
    results.innerHTML = '<ul class="search-results">' + hits.map(h => {
      const item = h.item;
      const titleMatch = (h.matches || []).find(m => m.key === 'title');
      const bodyMatch = (h.matches || []).find(m => m.key === 'body');
      const titleHtml = titleMatch ? highlight(item.title, titleMatch.indices) : escape(item.title);
      const snip = bodyMatch ? snippetAround(item.body, bodyMatch.indices) : item.body.slice(0, 120);
      const snipHtml = bodyMatch ? highlight(snip, bodyMatch.indices) : escape(snip);
      return \`<li>
        <a href="\${item.url}"><strong>\${titleHtml}</strong></a>
        <div class="path">\${escape(item.path)}</div>
        <div class="snippet">\${snipHtml}</div>
        <div class="tags">\${item.tags ? item.tags.split(' ').map(t => '#' + t).join(' ') : ''}</div>
      </li>\`;
    }).join('') + '</ul>';
  }
  function escape(s) {
    return String(s).replace(/[&<>"']/g, c => ({
      '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;'
    }[c]));
  }
  let t
  input.addEventListener('input', () => { clearTimeout(t); t = setTimeout(run, 120); });
  run();
})();
</script>`
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main() {
  const { vault, out, base } = parseArgs()
  const vaultAbs = path.resolve(vault)
  const outAbs = path.resolve(out)
  const generatedAt = new Date().toISOString()
  const vaultName = path.basename(vaultAbs)
  const tpl = await readTemplate()

  console.log(`[ssg] vault:  ${vaultAbs}`)
  console.log(`[ssg] output: ${outAbs}`)
  console.log(`[ssg] base:   ${base}`)

  // Load notes
  const notes = await loadAllNotes(vaultAbs)
  console.log(`[ssg] loaded ${notes.length} notes`)

  // Build resolver (links between notes)
  const resolveLink = buildResolver(notes)

  // Build markdown renderer with link resolver
  const md = buildMarkdown(resolveLink)

  // Render each note
  for (const n of notes) {
    const html = md.render(n.body)
    const highlighted = await highlightCodeBlocks(html)
    n.html = highlighted
    n.outPath = slugForNotePath(n.relPath) + '.html'
  }

  // Post-pass: resolve ![[transclusions]]
  for (const n of notes) {
    n.html = resolveTransclusions(n.html, notes, n)
  }

  // Write per-note HTML
  await fs.rm(outAbs, { recursive: true, force: true })
  await fs.mkdir(outAbs, { recursive: true })
  for (const n of notes) {
    const absOut = path.join(outAbs, n.outPath)
    await fs.mkdir(path.dirname(absOut), { recursive: true })
    const noteUrl = base + n.outPath
    const ogPath = (n.outPath.replace(/\.html$/, '.svg'))
    const ogUrl = base + 'og/' + ogPath
    const tplVars = {
      title: n.title + ' · ' + vaultName,
      description: n.fm.description || '',
      content: `<article class="note">
        <header class="article-header">
          <p class="crumb"><a href="${base}">← ${escapeHtml(vaultName)}</a></p>
          <h1>${escapeHtml(n.title)}</h1>
          <div class="article-meta">
            <span>${escapeHtml(n.relPath)}</span>
            ${(n.tags || []).map(t => `<a class="tag" href="${base}tags/${encodeURIComponent(t)}.html">#${escapeHtml(t)}</a>`).join(' ')}
          </div>
        </header>
        <div class="article-body">${n.html}</div>
      </article>`,
      base,
      generatedAt,
      vaultName,
      extraHead: ogMetaTags({ title: n.title, description: n.fm.description || '', url: noteUrl, image: ogUrl, type: 'article' }),
      extraBody: mermaidScript(base),
    }
    const html = applyTemplate(tpl, tplVars)
    await fs.writeFile(absOut, html, 'utf-8')
  }
  console.log(`[ssg] wrote ${notes.length} note pages`)

  // Index
  const indexTags = new Set()
  for (const n of notes) for (const t of n.tags || []) indexTags.add(t)
  const indexContent = buildIndex(notes, base, vaultName, generatedAt)
  const indexTpl = {
    title: vaultName,
    description: `${notes.length} 篇本地笔记`,
    content: indexContent,
    base,
    generatedAt,
    vaultName,
    extraHead: ogMetaTags({ title: vaultName, description: `${notes.length} 篇本地笔记 · ${indexTags.size} 个标签`, url: base, image: base + 'og/og-image.svg', type: 'website' }),
    extraBody: mermaidScript(base),
  }
  await fs.writeFile(path.join(outAbs, 'index.html'), applyTemplate(tpl, indexTpl), 'utf-8')
  console.log(`[ssg] wrote index.html`)

  // Tag pages
  const tagsDir = path.join(outAbs, 'tags')
  await fs.mkdir(tagsDir, { recursive: true })
  const allTags = new Set()
  for (const n of notes) for (const t of n.tags || []) allTags.add(t)
  for (const t of allTags) {
    const tagTpl = {
      title: `#${t} · ${vaultName}`,
      description: '',
      content: buildTagPage(t, notes, base, vaultName),
      base,
      generatedAt,
      vaultName,
      extraHead: '',
      extraBody: mermaidScript(base),
    }
    await fs.writeFile(
      path.join(tagsDir, encodeURIComponent(t) + '.html'),
      applyTemplate(tpl, tagTpl),
      'utf-8',
    )
  }
  console.log(`[ssg] wrote ${allTags.size} tag pages`)

  // Search page
  const searchPage = `<div class="search-page">
    <header class="page-header">
      <p class="crumb"><a href="${base}">← ${escapeHtml(vaultName)}</a></p>
      <h1>搜索</h1>
      <input id="search-input" type="search" placeholder="输入关键词，回车无动作（自动）" autofocus>
      <div id="search-meta" class="search-meta"></div>
    </header>
    <div id="search-results"><p class="empty">输入关键词开始搜索（fuzzy · 中文友好）</p></div>
  </div>`
  const searchIdxJs = buildSearchIndex(notes, base)
  const searchTpl = {
    title: `搜索 · ${vaultName}`,
    description: '',
    content: searchPage,
    base,
    generatedAt,
    vaultName,
    extraHead: '',
    extraBody: `<script>${searchIdxJs}</script>\n${searchScript(base)}`,
  }
  await fs.writeFile(path.join(outAbs, 'search.html'), applyTemplate(tpl, searchTpl), 'utf-8')
  console.log(`[ssg] wrote search.html`)

  // RSS
  await fs.writeFile(path.join(outAbs, 'rss.xml'), buildRss(notes, base, vaultName), 'utf-8')
  console.log(`[ssg] wrote rss.xml`)

  // Sitemap
  await fs.writeFile(path.join(outAbs, 'sitemap.xml'), buildSitemap(notes, base), 'utf-8')
  console.log(`[ssg] wrote sitemap.xml`)

  // 404
  await fs.writeFile(path.join(outAbs, '404.html'), applyTemplate(tpl, {
    title: '404',
    description: '页面不存在',
    content: `<div class="not-found"><h1>404</h1><p>页面不存在</p><p><a href="${base}">← ${escapeHtml(vaultName)}</a></p></div>`,
    base, generatedAt, vaultName, extraHead: '', extraBody: '',
  }), 'utf-8')

  // .nojekyll (GitHub Pages 默认 jekyll，禁用避免下划线开头的文件被过滤)
  await fs.writeFile(path.join(outAbs, '.nojekyll'), '', 'utf-8')

  // CNAME (自定义域名)
  const cnameSrc = path.join(vaultAbs, 'CNAME')
  try {
    const cnameContent = await fs.readFile(cnameSrc, 'utf-8')
    const cname = cnameContent.trim().split('\n')[0].trim()
    if (cname) {
      await fs.writeFile(path.join(outAbs, 'CNAME'), cname, 'utf-8')
      console.log(`[ssg] wrote CNAME: ${cname}`)
    }
  } catch {
    // 没有 CNAME 文件，正常
  }

  // OG images (per-note + default)
  console.log(`[ssg] generating OG images…`)
  await fs.mkdir(path.join(outAbs, 'og'), { recursive: true })
  // Default
  await fs.writeFile(
    path.join(outAbs, 'og', 'og-image.svg'),
    ogImageSvg({ title: vaultName, subtitle: `${notes.length} 篇本地笔记`, tags: [] }),
    'utf-8',
  )
  // Per-note (in parallel)
  await Promise.all(
    notes.map(async (n) => {
      const url = urlForNotePath(n.relPath, '').replace(/\.html$/, '.svg')
      const tags = (n.tags || []).slice(0, 4)
      const svg = ogImageSvg({ title: n.title, subtitle: n.fm.description || n.relPath, tags })
      const dir = path.dirname(path.join(outAbs, 'og', url))
      await fs.mkdir(dir, { recursive: true })
      await fs.writeFile(path.join(outAbs, 'og', url), svg, 'utf-8')
    }),
  )
  console.log(`[ssg] wrote ${notes.length + 1} OG images`)

  console.log(`[ssg] done. output: ${outAbs}`)
}

main().catch((e) => {
  console.error('[ssg] failed:', e)
  process.exit(1)
})

// Markdown rendering - markdown-it with Shiki, Mermaid, wikilinks, block refs

import MarkdownIt from 'markdown-it'

const md: MarkdownIt = new MarkdownIt({
  html: false,
  xhtmlOut: false,
  breaks: true,
  linkify: true,
  typographer: true,
})

// Wikilink: [[note]] / [[note|alias]] / [[note#section]] / [[note#^block]]
const WIKILINK_RE = /\[\[([^\[\]\|]+?)(?:\|([^\]]+?))?\]\]/g
md.inline.ruler.before('emphasis', 'wikilink', (state, silent) => {
  const pos = state.pos
  if (state.src.charCodeAt(pos) !== 0x5b /* [ */) return false
  if (state.src.charCodeAt(pos + 1) !== 0x5b) return false
  const re = new RegExp(WIKILINK_RE.source, 'g')
  const match = re.exec(state.src.slice(pos))
  if (!match) return false
  if (silent) return true

  const target = (match[1] ?? '').trim()
  const alias = (match[2] ?? '').trim() || target
  const token = state.push('wikilink', '', 0)
  token.content = alias
  token.info = target
  state.pos += match[0].length
  return true
})

md.renderer.rules.wikilink = (tokens, idx) => {
  const t = tokens[idx]
  const target = t!.info
  const label = md.utils.escapeHtml(t!.content)
  // Use data-wikilink for click, class for hover preview
  return `<a class="wikilink" data-wikilink="${md.utils.escapeHtml(target)}" href="#${encodeURIComponent(target)}">${label}</a>`
}

// Block ref: ((block-id))
const BLOCK_REF_RE = /\(\((blk_[a-z0-9]+)\)\)/g
md.inline.ruler.before('emphasis', 'block_ref', (state, silent) => {
  const pos = state.pos
  if (state.src.charCodeAt(pos) !== 0x28 /* ( */) return false
  if (state.src.charCodeAt(pos + 1) !== 0x28) return false
  const re = new RegExp(BLOCK_REF_RE.source, 'g')
  const match = re.exec(state.src.slice(pos))
  if (!match) return false
  if (silent) return true
  const target = match[1]
  const token = state.push('block_ref', '', 0)
  token.content = target
  state.pos += match[0].length
  return true
})

md.renderer.rules.block_ref = (tokens, idx) => {
  const t = tokens[idx]
  const target = t!.content
  // Display short form
  const short = target.replace(/^blk_/, '#')
  return `<a class="block-ref" data-block-ref="${md.utils.escapeHtml(target)}" href="#${md.utils.escapeHtml(target)}" title="块引用 ${md.utils.escapeHtml(target)}">${short}</a>`
}

// Add a "block-anchor" wrapper around headings so we can attach block IDs.
// We override the default heading_open rule. The block id is computed by the
// backend; for now we render headings normally and let the post-process step
// in Preview.vue inject the anchor.

// Fence (code block) — emit placeholder for post-processing
md.renderer.rules.fence = (tokens, idx) => {
  const token = tokens[idx]
  const info = (token!.info || '').trim()
  const code = token!.content

  if (info === 'mermaid') {
    const safe = code
      .replace(/&/g, '&amp;')
      .replace(/"/g, '&quot;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
    return `<div class="mermaid-block" data-source="${safe}"></div>`
  }

  const safe = code
    .replace(/&/g, '&amp;')
    .replace(/"/g, '&quot;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
  return `<pre class="md-code-block language-${info || 'text'}" data-lang="${info || 'text'}" data-source="${safe}"><code></code></pre>`
}

export function renderMarkdown(src: string): string {
  return md.render(src)
}

export default md

<script setup lang="ts">
import { computed, ref, watch, nextTick, onMounted, onBeforeUnmount } from 'vue'
import { renderMarkdown } from '../lib/markdown'
import { renderCode } from '../lib/highlight'
import { renderMermaid, initMermaid } from '../lib/mermaid'
import { useNotesStore } from '../stores/notes'
import { useLinksStore } from '../stores/links'
import { api } from '../lib/tauri'
import type { Block } from '../types'

const notes = useNotesStore()
const links = useLinksStore()
const container = ref<HTMLDivElement | null>(null)
const hoverTip = ref<{ x: number; y: number; title: string; snippet: string } | null>(null)

const html = computed(() => {
  if (!notes.current) return ''
  return renderMarkdown(notes.current.body ?? notes.current.raw)
})

initMermaid()

// Post-process: inject block IDs, render code, render Mermaid
async function postProcess(host: HTMLElement) {
  // 0. Inject block IDs into headings + paragraphs
  //    We use the blocks list from the store (which mirrors the DB)
  //    by matching heading text to block content. The order matches.
  //    For simplicity we attach the block ID to the heading and the
  //    next paragraph, in the same order they appear in the blocks list.
  injectBlockAnchors(host)

  // 1. Code blocks
  const codeBlocks = host.querySelectorAll<HTMLPreElement>('pre.md-code-block')
  for (const pre of Array.from(codeBlocks)) {
    if ((pre as HTMLElement).dataset.processed === '1') continue
    ;(pre as HTMLElement).dataset.processed = '1'
    const code = unescapeHtml(pre.getAttribute('data-source') ?? '')
    const lang = pre.getAttribute('data-lang') || 'text'
    try {
      const rendered = await renderCode(code, lang)
      const tmp = document.createElement('div')
      tmp.innerHTML = rendered
      const shikiPre = tmp.querySelector('pre.shiki')
      if (shikiPre) {
        shikiPre.classList.add('md-code-block', `language-${lang}`)
        ;(shikiPre as HTMLElement).dataset.lang = lang
        const wrap = document.createElement('div')
        wrap.className = 'md-code-wrap'
        const toolbar = document.createElement('div')
        toolbar.className = 'md-code-toolbar'
        toolbar.innerHTML = `
          <span class="md-code-lang">${lang}</span>
          <button class="md-code-copy" type="button" title="复制代码">
            <span class="copy-label">复制</span>
          </button>
        `
        wrap.appendChild(toolbar)
        wrap.appendChild(shikiPre)
        pre.replaceWith(wrap)
      }
    } catch (e) {
      console.error('shiki render failed', e)
    }
  }

  // 2. Mermaid
  const mermaidBlocks = host.querySelectorAll<HTMLDivElement>('div.mermaid-block')
  for (const block of Array.from(mermaidBlocks)) {
    if ((block as HTMLElement).dataset.processed === '1') continue
    ;(block as HTMLElement).dataset.processed = '1'
    const source = unescapeHtml(block.getAttribute('data-source') ?? '')
    block.classList.add('mermaid-loading')
    block.textContent = '正在渲染图表...'
    try {
      const svg = await renderMermaid(source)
      block.classList.remove('mermaid-loading')
      block.innerHTML = svg
      const svgEl = block.querySelector('svg')
      if (svgEl) {
        svgEl.removeAttribute('width')
        svgEl.removeAttribute('height')
        ;(svgEl as unknown as HTMLElement).style.maxWidth = '100%'
        ;(svgEl as unknown as HTMLElement).style.height = 'auto'
      }
    } catch (e) {
      console.error('mermaid render failed', e)
    }
  }
}

function injectBlockAnchors(host: HTMLElement) {
  // Walk through block-level elements in order. For each block, find the
  // matching block from the store by order_index. If found, attach the ID.
  const blocksByOrder = new Map<number, Block>()
  for (const b of links.blocks) blocksByOrder.set(b.order_index, b)
  if (blocksByOrder.size === 0) return

  // The simplest heuristic: headings and paragraphs in the rendered HTML
  // appear in the same order as in the source. We walk them sequentially
  // and pair with the next unused block.
  const candidates = host.querySelectorAll<HTMLElement>('h1, h2, h3, h4, h5, h6, p, pre, blockquote, ul, ol')
  let orderIdx = 0
  for (const el of Array.from(candidates)) {
    const b = blocksByOrder.get(orderIdx)
    if (b) {
      ;(el as HTMLElement).dataset.blockId = b.id
      el.classList.add('md-block')
      // Add a hover-only block anchor
      const anchor = document.createElement('a')
      anchor.className = 'md-block-anchor'
      anchor.href = `#${b.id}`
      anchor.dataset.blockAnchor = b.id
      anchor.title = `点击复制块 ID: ${b.id}`
      anchor.textContent = '#'
      el.appendChild(anchor)
    }
    orderIdx += 1
  }
}

function unescapeHtml(s: string): string {
  return s
    .replace(/&quot;/g, '"')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&amp;/g, '&')
}

// Click handler: wikilink, block-ref, block-anchor, copy
function onClick(e: MouseEvent) {
  const target = e.target as HTMLElement

  // Copy button
  const copyBtn = target.closest('.md-code-copy') as HTMLButtonElement | null
  if (copyBtn) {
    const wrap = copyBtn.closest('.md-code-wrap')
    const pre = wrap?.querySelector('pre')
    const code = pre?.textContent ?? ''
    navigator.clipboard.writeText(code).then(() => {
      const label = copyBtn.querySelector('.copy-label')
      if (label) {
        const orig = label.textContent
        label.textContent = '✓ 已复制'
        setTimeout(() => { if (label) label.textContent = orig }, 1500)
      }
    }).catch(err => console.error('copy failed', err))
    return
  }

  // Block anchor (the # that appears on hover)
  const blockAnchor = target.closest('.md-block-anchor') as HTMLAnchorElement | null
  if (blockAnchor) {
    e.preventDefault()
    const id = blockAnchor.dataset.blockAnchor ?? ''
    if (id) {
      navigator.clipboard.writeText(`((${id}))`).then(() => {
        const orig = blockAnchor.textContent
        blockAnchor.textContent = '✓'
        setTimeout(() => { if (blockAnchor) blockAnchor.textContent = orig }, 1200)
      })
    }
    return
  }

  // Block ref ((blk_xxx))
  const blockRef = target.closest('a.block-ref') as HTMLAnchorElement | null
  if (blockRef) {
    e.preventDefault()
    const blockId = blockRef.dataset.blockRef ?? ''
    jumpToBlock(blockId)
    return
  }

  // Wikilink
  const wikilink = target.closest('a[data-wikilink]') as HTMLAnchorElement | null
  if (wikilink) {
    e.preventDefault()
    const wl = wikilink.getAttribute('data-wikilink') ?? ''
    // Parse out block id if present (note#^blk_xxxx)
    let targetPath = wl
    let blockId: string | null = null
    const caretIdx = wl.indexOf('^')
    if (caretIdx > 0) {
      targetPath = wl.substring(0, caretIdx)
      blockId = wl.substring(caretIdx + 1)
    } else {
      const hashIdx = wl.indexOf('#')
      if (hashIdx > 0) {
        targetPath = wl.substring(0, hashIdx)
        // Section heading: would need to resolve to a block
        const section = wl.substring(hashIdx + 1)
        const block = links.blocks.find(b => b.type === 'heading' && b.content.trim() === section.trim())
        if (block) blockId = block.id
      }
    }

    const targetNote = notes.recent.find(n =>
      n.title === targetPath || n.path === `${targetPath}.md` || n.path === targetPath
    )
    const openAndJump = async () => {
      try {
        await notes.openNote(targetNote ? targetNote.path : `${targetPath}.md`)
        if (blockId) {
          // Wait for preview to render, then scroll
          await nextTick()
          setTimeout(() => jumpToBlock(blockId!), 100)
        }
      } catch (err) {
        // Note doesn't exist; create it
        try {
          await notes.createNote(`${targetPath}.md`)
        } catch (e) {
          console.error('Failed to open/create note', e)
        }
      }
    }
    openAndJump()
  }
}

function jumpToBlock(blockId: string) {
  if (!container.value) return
  const el = container.value.querySelector(`[data-block-id="${blockId}"]`) as HTMLElement | null
  if (el) {
    el.scrollIntoView({ behavior: 'smooth', block: 'center' })
    el.classList.add('md-block-flash')
    setTimeout(() => el.classList.remove('md-block-flash'), 1500)
  }
}

// Hover preview for wikilinks
let hoverTimer: ReturnType<typeof setTimeout> | null = null
let currentLink: HTMLElement | null = null

function onMouseOver(e: MouseEvent) {
  const target = e.target as HTMLElement
  const wikilink = target.closest('a[data-wikilink]') as HTMLAnchorElement | null
  if (!wikilink) {
    hideTip()
    return
  }
  if (wikilink === currentLink) return
  currentLink = wikilink
  if (hoverTimer) clearTimeout(hoverTimer)
  hoverTimer = setTimeout(() => showTip(wikilink, e as MouseEvent), 500)
}

function onMouseOut(e: MouseEvent) {
  const target = e.target as HTMLElement
  const wikilink = target.closest('a[data-wikilink]') as HTMLAnchorElement | null
  if (!wikilink) return
  // Only hide if we're leaving the link itself
  const related = e.relatedTarget as HTMLElement | null
  if (related && wikilink.contains(related)) return
  hideTip()
}

function onMouseMove(e: MouseEvent) {
  if (!hoverTip.value) return
  hoverTip.value.x = e.clientX + 12
  hoverTip.value.y = e.clientY + 16
}

async function showTip(link: HTMLElement, _e: MouseEvent) {
  const target = link.getAttribute('data-wikilink') ?? ''
  // Strip ^ and # for path resolution
  const cleanPath = target.split('#')[0].split('^')[0]
  const found = notes.recent.find(n =>
    n.title === cleanPath || n.path === `${cleanPath}.md` || n.path === cleanPath
  )
  if (!found) {
    hoverTip.value = {
      x: _e.clientX + 12,
      y: _e.clientY + 16,
      title: target,
      snippet: '（点击创建）',
    }
    return
  }
  // Load note body for snippet (lazy)
  try {
    const note = await api.readNote(found.path)
    const snippet = (note.body || '')
      .replace(/^---[\s\S]*?---\n?/, '')
      .replace(/^#+\s+.*/gm, '')
      .replace(/\n+/g, ' ')
      .trim()
      .slice(0, 200)
    hoverTip.value = {
      x: _e.clientX + 12,
      y: _e.clientY + 16,
      title: found.title,
      snippet: snippet || '（空笔记）',
    }
  } catch (err) {
    console.error('hover preview failed', err)
  }
}

function hideTip() {
  if (hoverTimer) { clearTimeout(hoverTimer); hoverTimer = null }
  currentLink = null
  hoverTip.value = null
}

async function refresh() {
  await nextTick()
  if (container.value) {
    container.value.scrollTop = 0
    await postProcess(container.value)
  }
}

watch(() => notes.currentPath, refresh)
watch(() => links.blocks, refresh, { deep: false })

onMounted(() => {
  document.addEventListener('mousemove', onMouseMove)
})
onBeforeUnmount(() => {
  document.removeEventListener('mousemove', onMouseMove)
  hideTip()
})
</script>

<template>
  <aside
    class="border-l border-border bg-bg overflow-y-auto scrollbar-thin relative"
    :style="{ width: '480px' }"
    @click="onClick"
    @mouseover="onMouseOver"
    @mouseout="onMouseOut"
  >
    <div ref="container" class="md-preview px-6 py-4 max-w-none" v-html="html" />

    <!-- Hover preview tooltip -->
    <Teleport to="body">
      <div
        v-if="hoverTip"
        class="fixed z-50 max-w-[360px] px-3 py-2 rounded-md shadow-xl border border-border bg-bg text-sm pointer-events-none"
        :style="{ left: hoverTip.x + 'px', top: hoverTip.y + 'px' }"
      >
        <div class="font-medium text-fg mb-1">{{ hoverTip.title }}</div>
        <div class="text-fg-muted text-xs line-clamp-4">{{ hoverTip.snippet }}</div>
      </div>
    </Teleport>
  </aside>
</template>

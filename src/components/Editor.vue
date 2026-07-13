<script setup lang="ts">
import { ref, watch, onMounted, onBeforeUnmount, computed } from 'vue'
import { EditorView, keymap, lineNumbers, highlightActiveLine, drawSelection } from '@codemirror/view'
import { EditorState } from '@codemirror/state'
import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands'
import { markdown } from '@codemirror/lang-markdown'
import { syntaxHighlighting, HighlightStyle, defaultHighlightStyle, bracketMatching, foldGutter, foldKeymap, indentOnInput } from '@codemirror/language'
import { tags as t } from '@lezer/highlight'
import { searchKeymap, search, highlightSelectionMatches } from '@codemirror/search'
import { autocompletion, completionKeymap, CompletionContext } from '@codemirror/autocomplete'
import { oneDark } from '@codemirror/theme-one-dark'
import { useNotesStore } from '../stores/notes'
import { useUiStore } from '../stores/ui'

const notes = useNotesStore()
const ui = useUiStore()

const container = ref<HTMLDivElement | null>(null)
let view: EditorView | null = null
let suppressNextChange = false

// Wikilink + tag autocompletion based on notes in the vault
function wikilinkCompletionSource() {
  return (ctx: CompletionContext) => {
    const before = ctx.matchBefore(/\[\[[^\]]*$/)
    if (!before) return null
    const query = before.text.slice(2).toLowerCase()
    const options = notes.recent
      .filter(n => n.title.toLowerCase().includes(query))
      .slice(0, 10)
      .map(n => ({ label: n.title, type: 'text', apply: n.title + ']]' }))
    if (options.length === 0) return null
    return { from: before.from + 2, options }
  }
}

function tagCompletionSource() {
  return (ctx: CompletionContext) => {
    const before = ctx.matchBefore(/#[A-Za-z\u4e00-\u9fff]*$/)
    if (!before) return null
    const query = before.text.slice(1).toLowerCase()
    const tagSet = new Set<string>()
    for (const n of notes.recent) for (const t of n.tags) tagSet.add(t)
    const options = Array.from(tagSet)
      .filter(t => t.toLowerCase().includes(query))
      .slice(0, 10)
      .map(t => ({ label: t, type: 'text', apply: t }))
    if (options.length === 0) return null
    return { from: before.from + 1, options }
  }
}

function buildView(initial: string) {
  if (!container.value) return
  const state = EditorState.create({
    doc: initial,
    extensions: [
      lineNumbers(),
      highlightActiveLine(),
      drawSelection(),
      history(),
      bracketMatching(),
      foldGutter(),
      indentOnInput(),
      highlightSelectionMatches(),
      search(),
      autocompletion({
        override: [wikilinkCompletionSource(), tagCompletionSource()],
        activateOnTyping: true,
      }),
      markdown(),
      syntaxHighlighting(HighlightStyle.define([
        { tag: t.heading, color: 'var(--accent)', fontWeight: 'bold' },
        ...defaultHighlightStyle.specs,
      ])),
      keymap.of([
        indentWithTab,
        ...defaultKeymap,
        ...historyKeymap,
        ...searchKeymap,
        ...foldKeymap,
        ...completionKeymap,
        {
          key: 'Mod-s',
          preventDefault: true,
          run: () => { notes.save(); return true },
        },
      ]),
      EditorView.lineWrapping,
      ui.theme === 'dark' ? oneDark : [],
      EditorView.theme({
        '&': { height: '100%', fontSize: '14px' },
        '.cm-scroller': { fontFamily: 'var(--font-mono, ui-monospace, monospace)' },
        '.cm-content': { padding: '12px 8px' },
        '.cm-gutters': { backgroundColor: 'transparent', borderRight: '1px solid rgb(var(--border))' },
      }),
      EditorView.updateListener.of((u) => {
        if (u.docChanged && !suppressNextChange) {
          notes.updateContent(u.state.doc.toString())
        }
      }),
    ],
  })
  view = new EditorView({ state, parent: container.value })
}

onMounted(() => {
  if (notes.current) buildView(notes.current.raw)
})

onBeforeUnmount(() => {
  view?.destroy()
  view = null
})

watch(() => notes.currentPath, async (path) => {
  if (path && notes.current) {
    suppressNextChange = true
    view?.destroy()
    buildView(notes.current.raw)
    queueMicrotask(() => { suppressNextChange = false })
  }
})

const stats = computed(() => {
  if (!notes.current) return { words: 0, chars: 0, lines: 0 }
  const text = notes.current.body || ''
  return {
    chars: text.length,
    words: text.split(/\s+/).filter(Boolean).length,
    lines: text.split('\n').length,
  }
})

defineExpose({ stats })
</script>

<template>
  <section class="flex-1 flex flex-col overflow-hidden bg-bg">
    <div v-if="notes.current" class="px-4 py-2 border-b border-border flex items-center gap-2 text-sm">
      <span class="text-fg-muted truncate">{{ notes.current.path }}</span>
      <span v-if="notes.dirty" class="text-amber-500 text-xs">● 未保存</span>
      <span v-else-if="notes.saving" class="text-fg-subtle text-xs">保存中...</span>
      <span v-else class="text-fg-subtle text-xs">已保存</span>
      <div class="flex-1" />
      <span class="text-xs text-fg-subtle">
        {{ stats.chars }} 字 · {{ stats.lines }} 行
      </span>
    </div>
    <div ref="container" class="flex-1 overflow-hidden" />
  </section>
</template>

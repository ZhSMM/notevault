<script setup lang="ts">
import { computed, onMounted, watch } from 'vue'
import { useNotesStore } from '../stores/notes'
import { useLinksStore } from '../stores/links'
import type { BacklinkHit } from '../types'

const notes = useNotesStore()
const links = useLinksStore()

// Group backlinks by source note
const grouped = computed(() => {
  const groups = new Map<string, { path: string; title: string; hits: BacklinkHit[] }>()
  for (const b of links.backlinks) {
    if (!groups.has(b.source_path)) {
      groups.set(b.source_path, {
        path: b.source_path,
        title: b.source_title || b.source_path,
        hits: [],
      })
    }
    groups.get(b.source_path)!.hits.push(b)
  }
  return Array.from(groups.values())
})

const totalBacklinks = computed(() => links.backlinks.length)
const totalForward = computed(() => links.forwardLinks.length)
const totalDangling = computed(() => links.dangling.length)

const showForward = computed(() => links.forwardLinks.length > 0)
const showDangling = computed(() => links.dangling.length > 0)

async function openSource(path: string) {
  await notes.openNote(path)
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
}

function highlightWikilinks(context: string): string {
  // Make the wikilink target visible in the context snippet
  return escapeHtml(context)
    .replace(/\[\[([^\]]+)\]\]/g, '<span class="text-accent font-medium">[[$1]]</span>')
    .replace(/\(\(([^\)]+)\)\)/g, '<span class="text-accent font-medium">(($1))</span>')
}

onMounted(() => {
  if (notes.currentPath) links.refreshAll(notes.currentPath)
})

watch(() => notes.currentPath, (p) => {
  if (p) links.refreshAll(p)
})
</script>

<template>
  <div class="border-t border-border bg-bg-soft">
    <!-- Header -->
    <div class="px-4 py-2 flex items-center gap-3 text-xs text-fg-muted">
      <span class="font-medium text-fg">🔗 反向链接</span>
      <span>{{ totalBacklinks }} 个引用</span>
      <span v-if="totalForward > 0">· 链出 {{ totalForward }}</span>
      <span v-if="totalDangling > 0" class="text-amber-500">· {{ totalDangling }} 个悬空</span>
    </div>

    <!-- Empty -->
    <div v-if="totalBacklinks === 0 && totalForward === 0" class="px-4 py-4 text-xs text-fg-subtle">
      还没有反向链接。在别的笔记里写 <code class="px-1 rounded bg-bg-muted">[[{{ notes.current?.title || '笔记名' }}]]</code> 试试。
    </div>

    <!-- Backlinks grouped by source -->
    <div v-else-if="totalBacklinks > 0" class="border-t border-border">
      <div
        v-for="group in grouped"
        :key="group.path"
        class="border-b border-border last:border-b-0"
      >
        <button
          class="w-full text-left px-4 py-2 hover:bg-bg-muted flex items-baseline gap-2"
          @click="openSource(group.path)"
        >
          <span class="text-sm font-medium text-fg truncate flex-1">{{ group.title }}</span>
          <span class="text-xs text-fg-subtle truncate">{{ group.path }}</span>
        </button>
        <div class="px-4 pb-2 space-y-1">
          <div
            v-for="(hit, i) in group.hits"
            :key="i"
            class="text-xs text-fg-muted pl-3 border-l-2 border-border"
            v-html="highlightWikilinks(hit.context)"
          />
        </div>
      </div>
    </div>

    <!-- Forward links (outgoing) -->
    <div v-if="showForward" class="border-t border-border px-4 py-2">
      <div class="text-xs uppercase tracking-wide text-fg-subtle mb-1.5">链出</div>
      <div class="flex flex-wrap gap-1.5">
        <button
          v-for="f in links.forwardLinks"
          :key="f.source_path"
          class="text-xs px-2 py-1 rounded bg-bg-muted hover:bg-bg-soft text-fg"
          @click="openSource(f.source_path)"
        >
          → {{ f.source_title || f.source_path }}
        </button>
      </div>
    </div>

    <!-- Dangling (unresolved) -->
    <div v-if="showDangling" class="border-t border-border px-4 py-2">
      <div class="text-xs uppercase tracking-wide text-amber-500 mb-1.5">⚠ 悬空链接</div>
      <div class="space-y-1">
        <div v-for="d in links.dangling" :key="d.from_note + d.to_alias" class="text-xs text-fg-muted">
          <span class="font-mono text-amber-500">{{ d.to_alias }}</span>
          <span class="text-fg-subtle"> — </span>
          <span class="truncate">{{ d.context }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

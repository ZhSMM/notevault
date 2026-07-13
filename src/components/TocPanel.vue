<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useNotesStore } from '../stores/notes'

const notes = useNotesStore()
const open = ref(false)
const activeId = ref<string | null>(null)

interface TocItem {
  id: string
  text: string
  level: number
}

const items = computed<TocItem[]>(() => {
  // Read the rendered preview DOM (which already has heading IDs)
  const container = document.querySelector('.article-body')
  if (!container) return []
  const headings = container.querySelectorAll<HTMLElement>('h1, h2, h3, h4, h5, h6')
  const out: TocItem[] = []
  for (const h of headings) {
    const id = h.getAttribute('id')
    if (!id) continue
    const text = (h.textContent || '').replace(/\s*#\s*$/, '').trim()
    const level = parseInt(h.tagName[1], 10)
    out.push({ id, text, level })
  }
  return out
})

// Track which heading is currently in view
let observer: IntersectionObserver | null = null
function attachObserver() {
  observer?.disconnect()
  if (!items.value.length) return
  const targets = items.value
    .map(i => document.getElementById(i.id))
    .filter((el): el is HTMLElement => !!el)
  if (targets.length === 0) return
  observer = new IntersectionObserver(
    (entries) => {
      for (const e of entries) {
        if (e.isIntersecting) {
          activeId.value = e.target.id
        }
      }
    },
    { rootMargin: '-20% 0px -70% 0px', threshold: 0 },
  )
  for (const t of targets) observer.observe(t)
}

watch(
  [() => notes.currentPath, () => open.value],
  () => {
    activeId.value = null
    setTimeout(attachObserver, 200)
  },
)

function jump(id: string) {
  const el = document.getElementById(id)
  if (!el) return
  el.scrollIntoView({ behavior: 'smooth', block: 'start' })
  activeId.value = id
  // Flash like the wikilink jump
  el.classList.add('block-flash')
  setTimeout(() => el.classList.remove('block-flash'), 1500)
}

function levelIndent(l: number) {
  return { paddingLeft: `${(l - 1) * 12 + 8}px` }
}
</script>

<template>
  <button
    v-if="!open"
    class="fixed top-1/2 right-3 -translate-y-1/2 z-30 btn btn-secondary text-xs h-9 px-2 shadow-md"
    @click="open = true"
    title="显示大纲"
  >
    ☰
  </button>

  <aside
    v-if="open"
    class="fixed top-12 right-0 bottom-7 w-64 bg-bg border-l border-border z-30 shadow-lg flex flex-col"
  >
    <header class="h-9 px-3 flex items-center gap-2 border-b border-border bg-bg-soft text-sm">
      <span class="font-medium">☰ 大纲</span>
      <span class="text-xs text-fg-subtle">{{ items.length }} 节</span>
      <div class="flex-1" />
      <button class="text-xs hover:text-fg" @click="open = false" title="关闭">✕</button>
    </header>
    <div class="flex-1 overflow-y-auto p-2">
      <div v-if="!notes.currentPath" class="text-xs text-fg-subtle p-2">
        打开笔记后这里会显示大纲。
      </div>
      <div v-else-if="items.length === 0" class="text-xs text-fg-subtle p-2">
        当前笔记没有标题。
      </div>
      <ul v-else class="text-sm space-y-0.5">
        <li v-for="it in items" :key="it.id">
          <button
            class="w-full text-left truncate hover:bg-bg-soft rounded px-2 py-1 text-xs"
            :class="activeId === it.id ? 'bg-accent/10 text-accent font-medium' : 'text-fg-subtle hover:text-fg'"
            :style="levelIndent(it.level)"
            :title="it.text"
            @click="jump(it.id)"
          >
            <span class="text-[10px] opacity-60 mr-1">H{{ it.level }}</span>{{ it.text }}
          </button>
        </li>
      </ul>
    </div>
  </aside>
</template>

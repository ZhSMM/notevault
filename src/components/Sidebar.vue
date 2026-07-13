<script setup lang="ts">
import { ref, computed } from 'vue'
import { useNotesStore } from '../stores/notes'
import { useUiStore } from '../stores/ui'
import { useVaultStore } from '../stores/vault'
import FileTreeNode from './FileTreeNode.vue'

const notes = useNotesStore()
const ui = useUiStore()
const vault = useVaultStore()

const newNoteName = ref('')
const showNewInput = ref(false)

const tags = computed(() => {
  const set = new Set<string>()
  for (const n of notes.recent) for (const t of n.tags) set.add(t)
  return Array.from(set).sort()
})

const filteredTree = computed(() => notes.tree)

async function submitNew() {
  const name = newNoteName.value.trim()
  if (!name) return
  const meta = await notes.createNoteSimple(name)
  if (meta) {
    newNoteName.value = ''
    showNewInput.value = false
  }
}

function openNew() {
  showNewInput.value = true
  // focus will be set by next tick
  setTimeout(() => {
    const el = document.getElementById('new-note-input') as HTMLInputElement | null
    el?.focus()
  }, 50)
}
</script>

<template>
  <aside
    class="panel h-full"
    :style="{ width: ui.sidebarWidth + 'px' }"
  >
    <div class="px-3 py-2 border-b border-border flex items-center gap-2">
      <div class="flex-1 min-w-0">
        <div class="text-xs uppercase tracking-wide text-fg-subtle">Vault</div>
        <div class="text-sm font-medium truncate">
          {{ vault.info?.name || '' }}
        </div>
      </div>
      <button
        class="btn btn-primary py-1 px-2 text-xs flex items-center gap-1"
        @click="openNew"
        title="新建笔记 (Ctrl+N)"
      >
        <span class="text-base leading-none">+</span>
        <span>新建</span>
      </button>
      <button class="icon-btn" @click="ui.toggleTheme()" :title="ui.theme === 'dark' ? '亮色' : '暗色'">
        {{ ui.theme === 'dark' ? '☀️' : '🌙' }}
      </button>
    </div>

    <div v-if="showNewInput" class="px-3 py-2 border-b border-border bg-bg-soft flex gap-1.5">
      <input
        id="new-note-input"
        v-model="newNoteName"
        class="input flex-1"
        placeholder="笔记名（不含 .md）"
        @keydown.enter="submitNew"
        @keydown.escape="showNewInput = false"
      />
      <button class="btn btn-primary text-xs" @click="submitNew">创建</button>
    </div>

    <button
      class="mx-3 my-2 px-2 py-1.5 text-xs text-fg-muted bg-bg-soft border border-border rounded-md hover:bg-bg-muted text-left flex items-center gap-2"
      @click="ui.openSearch()"
    >
      <span>🔍</span>
      <span class="flex-1">搜索...</span>
      <kbd class="text-fg-subtle">Ctrl+P</kbd>
    </button>

    <div class="flex-1 overflow-y-auto scrollbar-thin px-1 py-1">
      <FileTreeNode
        v-for="node in filteredTree"
        :key="node.path"
        :node="node"
        :depth="0"
      />
      <div
        v-if="!filteredTree.length || (filteredTree[0] && !filteredTree[0].children.length)"
        class="text-center text-fg-subtle text-xs py-8 px-4"
      >
        <p>vault 里还没有笔记</p>
        <button class="btn btn-primary mt-3" @click="openNew">+ 新建第一篇</button>
      </div>
    </div>

    <div v-if="tags.length" class="border-t border-border px-3 py-2">
      <div class="text-xs uppercase tracking-wide text-fg-subtle mb-1.5">Tags</div>
      <div class="flex flex-wrap gap-1">
        <span
          v-for="t in tags"
          :key="t"
          class="text-xs px-1.5 py-0.5 rounded bg-bg-muted text-fg-muted"
        >#{{ t }}</span>
      </div>
    </div>
  </aside>
</template>

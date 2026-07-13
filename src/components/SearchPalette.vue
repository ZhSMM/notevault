<script setup lang="ts">
import { ref, onMounted, nextTick } from 'vue'
import { useNotesStore } from '../stores/notes'
import type { SearchMode } from '../stores/ui'

const props = defineProps<{
  mode?: SearchMode
}>()

const emit = defineEmits<{
  close: []
  switchMode: [{ mode: SearchMode }]
}>()

const notes = useNotesStore()
const input = ref<HTMLInputElement | null>(null)
const selected = ref(0)
const newNoteName = ref('')

const currentMode = ref<SearchMode>(props.mode ?? 'search')

onMounted(async () => {
  await nextTick()
  input.value?.focus()
})

function close() {
  emit('close')
}

function switchToMode(m: SearchMode) {
  currentMode.value = m
  emit('switchMode', { mode: m })
  selected.value = 0
  if (m === 'new') {
    setTimeout(() => input.value?.focus(), 50)
  }
}

function highlightSnippet(snippet: string): string {
  return snippet
    .replace(/<mark>/g, '<span class="bg-mark/40 rounded px-0.5">')
    .replace(/<\/mark>/g, '</span>')
}

function onSearchKey(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    e.preventDefault()
    close()
  } else if (e.key === 'ArrowDown') {
    e.preventDefault()
    selected.value = Math.min(selected.value + 1, notes.searchResults.length - 1)
  } else if (e.key === 'ArrowUp') {
    e.preventDefault()
    selected.value = Math.max(selected.value - 1, 0)
  } else if (e.key === 'Enter') {
    e.preventDefault()
    const hit = notes.searchResults[selected.value]
    if (hit) {
      notes.openNote(hit.path)
      close()
    }
  }
}

async function onNewSubmit() {
  const name = newNoteName.value.trim()
  if (!name) return
  const meta = await notes.createNoteSimple(name)
  if (meta) {
    close()
  }
}

function onNewKey(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    e.preventDefault()
    close()
  } else if (e.key === 'Enter') {
    e.preventDefault()
    onNewSubmit()
  }
}
</script>

<template>
  <div class="fixed inset-0 z-50 flex items-start justify-center pt-24 bg-black/30" @click.self="close">
    <div class="w-[600px] max-w-[90vw] bg-bg border border-border rounded-lg shadow-2xl overflow-hidden">
      <!-- Mode tabs -->
      <div class="flex border-b border-border">
        <button
          class="flex-1 px-3 py-1.5 text-xs uppercase tracking-wide transition-colors"
          :class="currentMode === 'search' ? 'bg-bg-soft text-fg' : 'text-fg-subtle hover:text-fg'"
          @click="switchToMode('search')"
        >搜索</button>
        <button
          class="flex-1 px-3 py-1.5 text-xs uppercase tracking-wide transition-colors"
          :class="currentMode === 'new' ? 'bg-bg-soft text-fg' : 'text-fg-subtle hover:text-fg'"
          @click="switchToMode('new')"
        >新建笔记</button>
      </div>

      <!-- Search mode -->
      <template v-if="currentMode === 'search'">
        <div class="border-b border-border">
          <input
            ref="input"
            :value="notes.searchQuery"
            @input="notes.setSearchQuery(($event.target as HTMLInputElement).value)"
            @keydown="onSearchKey"
            class="w-full px-4 py-3 bg-transparent outline-none text-sm"
            placeholder="搜索笔记（标题、内容、标签）..."
          />
        </div>
        <div class="max-h-[60vh] overflow-y-auto scrollbar-thin">
          <div v-if="!notes.searchQuery" class="px-4 py-8 text-center text-fg-subtle text-sm">
            开始输入以搜索
          </div>
          <div v-else-if="notes.searchResults.length === 0" class="px-4 py-8 text-center text-fg-subtle text-sm">
            没有匹配
          </div>
          <button
            v-for="(hit, i) in notes.searchResults"
            :key="hit.path"
            class="w-full text-left px-4 py-2.5 border-b border-border last:border-b-0 hover:bg-bg-soft"
            :class="{ 'bg-bg-soft': i === selected }"
            @click="notes.openNote(hit.path); close()"
            @mouseenter="selected = i"
          >
            <div class="flex items-center gap-2 mb-1">
              <span class="font-medium text-sm truncate">{{ hit.title || hit.path }}</span>
              <span class="text-xs text-fg-subtle truncate">{{ hit.path }}</span>
            </div>
            <div class="text-xs text-fg-muted line-clamp-2" v-html="highlightSnippet(hit.snippet)" />
          </button>
        </div>
        <div class="px-3 py-1.5 text-xs text-fg-subtle border-t border-border flex items-center gap-3">
          <span><kbd>↑</kbd> <kbd>↓</kbd> 选择</span>
          <span><kbd>Enter</kbd> 打开</span>
          <span><kbd>Esc</kbd> 关闭</span>
          <div class="flex-1" />
          <span>{{ notes.searchResults.length }} 个结果</span>
        </div>
      </template>

      <!-- New note mode -->
      <template v-else>
        <div class="px-4 py-3">
          <input
            ref="input"
            v-model="newNoteName"
            @keydown="onNewKey"
            class="w-full px-3 py-2 rounded-md text-sm bg-bg-soft border border-border placeholder:text-fg-subtle focus:outline-none focus:ring-2 focus:ring-accent/40"
            placeholder="给新笔记起个名字..."
          />
          <p class="text-xs text-fg-subtle mt-2">
            💡 笔记会自动放到 <code>0-inbox/</code> 目录（如果存在），否则放在 vault 根目录。
          </p>
          <p class="text-xs text-fg-subtle mt-1">
            标题来自文件名，frontmatter / 标签可以后面在编辑器里加。
          </p>
        </div>
        <div class="px-3 py-1.5 text-xs text-fg-subtle border-t border-border flex items-center gap-3">
          <span><kbd>Enter</kbd> 创建</span>
          <span><kbd>Esc</kbd> 关闭</span>
          <div class="flex-1" />
          <button class="btn btn-primary text-xs" @click="onNewSubmit" :disabled="!newNoteName.trim()">
            创建
          </button>
        </div>
      </template>
    </div>
  </div>
</template>

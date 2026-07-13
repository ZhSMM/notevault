<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useVaultStore } from '../stores/vault'
import { useNotesStore } from '../stores/notes'
import { useUiStore } from '../stores/ui'
import { useCardsStore } from '../stores/cards'
import { useGitStore } from '../stores/git'
import { useGraphStore } from '../stores/graph'
import { useAiStore } from '../stores/ai'
import { usePublishStore } from '../stores/publish'
import { formatDate } from '../lib/format'

const vault = useVaultStore()
const notes = useNotesStore()
const ui = useUiStore()
const cards = useCardsStore()
const git = useGitStore()
const ai = useAiStore()
const pub = usePublishStore()

onMounted(() => {
  cards.loadStats()
  if (vault.isOpen) git.refresh()
})

const total = computed(() => notes.recent.length)
const dueCount = computed(() => cards.stats?.due ?? 0)
const gitDirty = computed(() => {
  if (!git.status?.is_repo) return 0
  return git.status.modified.length + git.status.untracked.length
})
</script>

<template>
  <footer class="h-7 px-3 flex items-center gap-3 text-xs text-fg-subtle border-t border-border bg-bg-soft">
    <span>{{ vault.info?.name }}</span>
    <span>·</span>
    <span>{{ total }} 笔记</span>
    <span v-if="notes.currentPath">·</span>
    <span v-if="notes.current" class="truncate max-w-[30vw]">
      {{ notes.current.title }} · {{ formatDate(notes.current.modified) }}
    </span>
    <div class="flex-1" />
    <button
      v-if="dueCount > 0"
      class="hover:text-fg flex items-center gap-1 px-1.5 py-0.5 rounded bg-accent/10 text-accent"
      @click="cards.startReview()"
      :title="`有 ${dueCount} 张卡片待复习 (含 ${cards.stats?.new_count ?? 0} 张新卡)`"
    >
      🎓 {{ dueCount }}
    </button>
    <button
      class="hover:text-fg flex items-center gap-1"
      :class="gitDirty > 0 ? 'text-amber-500' : ''"
      @click="git.openPanel()"
      :title="git.status?.is_repo ? `打开 git 面板 (${gitDirty} 个改动)` : '初始化 git 仓库'"
    >
      ⎇ {{ git.status?.branch ?? 'init' }}
      <span v-if="gitDirty > 0" class="text-amber-500">●{{ gitDirty }}</span>
    </button>
    <button
      v-if="notes.dirty"
      class="text-amber-500"
    >● 未保存</button>
    <button class="hover:text-fg" @click="ui.togglePreview()" :title="ui.showPreview ? '隐藏预览' : '显示预览'">
      {{ ui.showPreview ? '◧' : '◨' }} 预览
    </button>
    <button class="hover:text-fg" @click="ui.openSearch({ mode: 'new' })" title="新建笔记 (Ctrl+N)">
      ＋ 新建
    </button>
    <button class="hover:text-fg" @click="ui.openSearch()" title="搜索 (Ctrl+P)">
      🔍 搜索
    </button>
    <button class="hover:text-fg" @click="notes.reindex()" title="重建索引">
      ⟳ 索引
    </button>
    <button class="hover:text-fg" @click="useGraphStore().load().then(() => useGraphStore().open())" title="图谱 (Ctrl+Shift+G)">
      🕸️ 图谱
    </button>
    <button
      class="hover:text-fg"
      :class="ai.pending ? 'text-accent' : ''"
      @click="ai.openPanel()"
      title="AI 助手 (Ctrl+Shift+A)"
    >
      🤖 AI
    </button>
    <button
      class="hover:text-fg"
      @click="pub.openPanel()"
      title="静态发布 (Ctrl+Shift+E)"
    >
      🌐 发布
    </button>
  </footer>
</template>

<script setup lang="ts">
import { onMounted } from 'vue'
import { useVaultStore } from './stores/vault'
import { useNotesStore } from './stores/notes'
import { useUiStore } from './stores/ui'
import { useCardsStore } from './stores/cards'
import { useGraphStore } from './stores/graph'
import { useAiStore } from './stores/ai'
import { usePublishStore } from './stores/publish'
import Sidebar from './components/Sidebar.vue'
import Editor from './components/Editor.vue'
import Preview from './components/Preview.vue'
import BacklinksPanel from './components/BacklinksPanel.vue'
import CardReviewPanel from './components/CardReviewPanel.vue'
import GitPanel from './components/GitPanel.vue'
import GraphPanel from './components/GraphPanel.vue'
import AIPanel from './components/AIPanel.vue'
import PublishPanel from './components/PublishPanel.vue'
import TocPanel from './components/TocPanel.vue'
import SearchPalette from './components/SearchPalette.vue'
import StatusBar from './components/StatusBar.vue'
import Welcome from './components/Welcome.vue'
import ToastHost from './components/ToastHost.vue'

const vault = useVaultStore()
const notes = useNotesStore()
const ui = useUiStore()
const cards = useCardsStore()
const graph = useGraphStore()
const ai = useAiStore()
const pub = usePublishStore()

async function newNote() {
  ui.openSearch({ mode: 'new' })
}

onMounted(async () => {
  await vault.refresh()
  if (vault.isOpen) {
    await Promise.all([notes.refreshTree(), notes.refreshRecent()])
  } else {
    const restored = await vault.tryRestoreLast()
    if (restored && vault.isOpen) {
      await Promise.all([notes.refreshTree(), notes.refreshRecent()])
    }
  }
  // Load card stats
  if (vault.isOpen) {
    await cards.loadStats()
  }
  window.addEventListener('keydown', onKey)
})

function onKey(e: KeyboardEvent) {
  const mod = e.ctrlKey || e.metaKey
  if (mod && e.key === 'p') {
    e.preventDefault()
    ui.toggleSearch()
  } else if (mod && e.key === 's') {
    e.preventDefault()
    notes.save()
  } else if (mod && e.shiftKey && e.key.toLowerCase() === 'p') {
    e.preventDefault()
    ui.togglePreview()
  } else if (mod && e.key === 'n') {
    e.preventDefault()
    newNote()
  } else if (mod && e.key === 'r') {
    if (cards.stats && cards.stats.due > 0) {
      e.preventDefault()
      cards.startReview()
    }
  } else if (mod && e.shiftKey && e.key.toLowerCase() === 'g') {
    e.preventDefault()
    if (graph.showGraph) {
      graph.close()
    } else {
      graph.load().then(() => graph.open())
    }
  } else if (mod && e.shiftKey && e.key.toLowerCase() === 'a') {
    e.preventDefault()
    ai.openPanel()
  } else if (mod && e.shiftKey && e.key.toLowerCase() === 'e') {
    e.preventDefault()
    pub.openPanel()
  } else if (e.key === 'Escape') {
    if (ai.showPanel) {
      ai.closePanel()
    } else if (pub.showPanel) {
      pub.closePanel()
    } else if (ui.showSearch) {
      ui.closeSearch()
    }
  }
}
</script>

<template>
  <div class="h-screen w-screen flex flex-col bg-bg text-fg overflow-hidden">
    <template v-if="!vault.isOpen">
      <Welcome />
    </template>
    <template v-else>
      <div class="flex-1 flex overflow-hidden">
        <Sidebar />
        <div class="flex-1 flex overflow-hidden">
          <Editor v-if="notes.currentPath" />
          <div v-else class="flex-1 flex items-center justify-center text-fg-subtle">
            <div class="text-center">
              <p class="text-lg mb-3">没有打开的笔记</p>
              <button class="btn btn-primary" @click="newNote">
                + 新建笔记
              </button>
              <p class="text-xs mt-3 text-fg-subtle">或按 <kbd>Ctrl</kbd>+<kbd>P</kbd> 搜索</p>
            </div>
          </div>
          <Preview v-if="notes.currentPath && ui.showPreview" />
        </div>
      </div>
      <BacklinksPanel v-if="notes.currentPath" />
      <StatusBar />
      <SearchPalette
        v-if="ui.showSearch"
        :mode="ui.searchMode"
        @close="ui.closeSearch"
        @switch-mode="ui.openSearch"
      />
      <CardReviewPanel />
      <GitPanel />
      <GraphPanel />
      <AIPanel />
      <PublishPanel />
      <TocPanel />
    </template>
    <ToastHost />
  </div>
</template>

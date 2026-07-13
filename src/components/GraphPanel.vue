<script setup lang="ts">
import { useGraphStore } from '../stores/graph'

const graph = useGraphStore()
</script>

<template>
  <Teleport to="body">
    <div
      v-if="graph.showGraph"
      class="fixed inset-0 z-30 bg-bg flex flex-col"
    >
      <div class="px-4 py-2 border-b border-border flex items-center gap-3 bg-bg-soft">
        <span class="text-base">🕸️</span>
        <h2 class="text-sm font-medium">知识图谱</h2>
        <span class="text-xs text-fg-subtle">{{ graph.nodes.length }} 节点 · {{ graph.edges.length }} 边</span>
        <div class="flex-1" />
        <button class="btn" @click="graph.close()">关闭 (Esc)</button>
      </div>
      <div class="flex-1 overflow-hidden">
        <GraphView />
      </div>
    </div>
  </Teleport>
</template>

<script lang="ts">
import GraphView from './GraphView.vue'
export default {}
</script>

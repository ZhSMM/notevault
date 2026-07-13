<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch, nextTick } from 'vue'
import cytoscape, { type Core, type ElementDefinition } from 'cytoscape'
import { api } from '../lib/tauri'
import { useNotesStore } from '../stores/notes'

const notes = useNotesStore()
const containerEl = ref<HTMLDivElement | null>(null)
const stats = ref({ nodes: 0, edges: 0 })
const orphanOnly = ref(false)
const tagFilter = ref<string>('')
let cy: Core | null = null

async function loadAndRender() {
  if (!containerEl.value) return
  const data = await api.getGraphData()
  stats.value = { nodes: data.nodes.length, edges: data.edges.length }

  // Apply filters
  let nodes = data.nodes
  let edges = data.edges
  if (orphanOnly.value) {
    const connected = new Set<string>()
    for (const e of edges) {
      connected.add(e.source)
      connected.add(e.target)
    }
    nodes = nodes.filter(n => !connected.has(n.id))
    edges = []
  }
  if (tagFilter.value.trim()) {
    const t = tagFilter.value.toLowerCase()
    nodes = nodes.filter(n => n.tags.some(tag => tag.toLowerCase().includes(t)))
    const keptIds = new Set(nodes.map(n => n.id))
    edges = edges.filter(e => keptIds.has(e.source) && keptIds.has(e.target))
  }

  const elements: ElementDefinition[] = [
    ...nodes.map(n => ({
      data: { id: n.id, label: n.label, size: n.size, in: n.in_degree, out: n.out_degree },
    })),
    ...edges.map(e => ({
      data: { id: e.id, source: e.source, target: e.target, kind: e.kind },
    })),
  ]

  if (cy) {
    cy.elements().remove()
    cy.add(elements)
    cy.layout({
      name: 'cose',
      animate: false,
      idealEdgeLength: () => 80,
      nodeRepulsion: () => 8000,
      gravity: 0.3,
    } as any).run()
  } else {
    cy = cytoscape({
      container: containerEl.value,
      elements,
      style: [
        {
          selector: 'node',
          style: {
            'background-color': '#6366f1',
            'label': 'data(label)',
            'color': '#fff',
            'font-size': '10px',
            'text-valign': 'center',
            'text-halign': 'center',
            'text-wrap': 'wrap',
            'text-max-width': '80px',
            'width': 'data(size)',
            'height': 'data(size)',
            'border-width': 1.5,
            'border-color': '#4f46e5',
            'text-outline-width': 1,
            'text-outline-color': '#6366f1',
            'text-outline-opacity': 0.6,
          },
        },
        {
          selector: 'node:selected',
          style: {
            'background-color': '#fbbf24',
            'border-color': '#f59e0b',
            'color': '#1f2937',
            'text-outline-color': '#fbbf24',
          },
        },
        {
          selector: 'edge',
          style: {
            'width': 1,
            'line-color': '#94a3b8',
            'opacity': 0.5,
            'target-arrow-shape': 'triangle',
            'target-arrow-color': '#94a3b8',
            'arrow-scale': 0.8,
            'curve-style': 'bezier',
          },
        },
        {
          selector: 'edge[kind = "block_ref"]',
          style: { 'line-style': 'dashed', 'line-color': '#a78bfa' },
        },
        {
          selector: 'edge[kind = "transclusion"]',
          style: { 'line-color': '#34d399', 'width': 1.5 },
        },
      ],
      layout: {
        name: 'cose',
        animate: false,
        idealEdgeLength: () => 80,
        nodeRepulsion: () => 8000,
        gravity: 0.3,
      } as any,
      minZoom: 0.2,
      maxZoom: 3,
      wheelSensitivity: 0.2,
    })

    cy.on('tap', 'node', (evt) => {
      const id = evt.target.id()
      notes.openNote(id)
    })
  }
}

onMounted(() => {
  nextTick(() => loadAndRender())
})

onBeforeUnmount(() => {
  cy?.destroy()
  cy = null
})

watch(orphanOnly, () => loadAndRender())
watch(tagFilter, () => loadAndRender())
watch(() => notes.currentPath, () => loadAndRender())

function recenter() {
  cy?.fit(undefined, 30)
}
</script>

<template>
  <div class="flex flex-col h-full">
    <div class="px-3 py-2 border-b border-border flex items-center gap-2 bg-bg-soft text-xs">
      <span class="font-medium">图谱</span>
      <span class="text-fg-subtle">{{ stats.nodes }} 节点 / {{ stats.edges }} 边</span>
      <div class="flex-1" />
      <input
        v-model="tagFilter"
        class="input py-1 text-xs w-32"
        placeholder="按 tag 过滤..."
      />
      <label class="flex items-center gap-1 text-fg-muted cursor-pointer">
        <input v-model="orphanOnly" type="checkbox" class="accent-accent" />
        只看悬空
      </label>
      <button class="icon-btn" @click="recenter" title="重新居中">⤧</button>
    </div>
    <div ref="containerEl" class="flex-1 bg-bg" />
    <div class="px-3 py-1.5 text-xs text-fg-subtle border-t border-border bg-bg-soft">
      点击节点跳转 · 滚轮缩放 · 拖拽移动
    </div>
  </div>
</template>

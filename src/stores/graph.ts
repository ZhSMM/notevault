// Graph store: nodes/edges from backend

import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api } from '../lib/tauri'
import type { GraphEdge, GraphNode } from '../types'

export const useGraphStore = defineStore('graph', () => {
  const showGraph = ref(false)
  const nodes = ref<GraphNode[]>([])
  const edges = ref<GraphEdge[]>([])
  const loading = ref(false)

  async function load() {
    loading.value = true
    try {
      const data = await api.getGraphData()
      nodes.value = data.nodes
      edges.value = data.edges
    } catch (e) {
      console.error('graph load failed', e)
    } finally {
      loading.value = false
    }
  }

  function open() { showGraph.value = true }
  function close() { showGraph.value = false }

  return { showGraph, nodes, edges, loading, load, open, close }
})

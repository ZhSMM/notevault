// Links store: backlinks, forward links, dangling

import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api } from '../lib/tauri'
import type { BacklinkHit, Block, DanglingLink } from '../types'

export const useLinksStore = defineStore('links', () => {
  const backlinks = ref<BacklinkHit[]>([])
  const forwardLinks = ref<BacklinkHit[]>([])
  const dangling = ref<DanglingLink[]>([])
  const blocks = ref<Block[]>([])
  const loading = ref(false)

  async function refreshBacklinks(notePath: string) {
    if (!notePath) {
      backlinks.value = []
      return
    }
    loading.value = true
    try {
      backlinks.value = await api.getBacklinks(notePath)
    } catch (e) {
      console.error('getBacklinks failed', e)
      backlinks.value = []
    } finally {
      loading.value = false
    }
  }

  async function refreshForwardLinks(notePath: string) {
    if (!notePath) {
      forwardLinks.value = []
      return
    }
    try {
      forwardLinks.value = await api.getForwardLinks(notePath)
    } catch (e) {
      console.error('getForwardLinks failed', e)
      forwardLinks.value = []
    }
  }

  async function refreshBlocks(notePath: string) {
    if (!notePath) {
      blocks.value = []
      return
    }
    try {
      blocks.value = await api.getBlocks(notePath)
    } catch (e) {
      console.error('getBlocks failed', e)
      blocks.value = []
    }
  }

  async function refreshAll(notePath: string) {
    await Promise.all([
      refreshBacklinks(notePath),
      refreshForwardLinks(notePath),
      refreshBlocks(notePath),
    ])
  }

  return {
    backlinks,
    forwardLinks,
    dangling,
    blocks,
    loading,
    refreshBacklinks,
    refreshForwardLinks,
    refreshBlocks,
    refreshAll,
  }
})

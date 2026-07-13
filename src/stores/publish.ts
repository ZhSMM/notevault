// Publish store - manages the static site export flow.

import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api } from '../lib/tauri'
import { useVaultStore } from './vault'
import { useToastStore } from './toast'

export const usePublishStore = defineStore('publish', () => {
  const showPanel = ref(false)
  const running = ref(false)
  const outputPath = ref('')
  const baseUrl = ref('/')
  const lastResult = ref<{
    pages: number
    tags: number
    log: string
    outputPath: string
  } | null>(null)
  const lastError = ref<string | null>(null)

  function openPanel() {
    showPanel.value = true
    // Default output: <vault>/.public
    const vault = useVaultStore()
    if (vault.info && !outputPath.value) {
      // Windows path handling: just join with forward/back slashes
      const sep = vault.info.path.includes('\\') ? '\\' : '/'
      outputPath.value = vault.info.path + sep + '.public'
    }
  }
  function closePanel() { showPanel.value = false }

  async function run() {
    const vault = useVaultStore()
    const toast = useToastStore()
    if (!vault.info) {
      toast.error('请先打开 vault')
      return
    }
    if (!outputPath.value) {
      toast.error('请指定输出目录')
      return
    }
    running.value = true
    lastError.value = null
    try {
      const r = await api.exportStatic(vault.info.path, {
        outputPath: outputPath.value,
        baseUrl: baseUrl.value,
      })
      lastResult.value = {
        pages: r.pages,
        tags: r.tags,
        log: r.log,
        outputPath: r.outputPath,
      }
      toast.success(`导出完成: ${r.pages} 个页面 · ${r.tags} 个标签页`)
    } catch (e: any) {
      lastError.value = String(e?.message ?? e)
      toast.error('导出失败：' + lastError.value)
    } finally {
      running.value = false
    }
  }

  return {
    showPanel,
    running,
    outputPath,
    baseUrl,
    lastResult,
    lastError,
    openPanel,
    closePanel,
    run,
  }
})

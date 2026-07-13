// Git store: status, log, init, commit

import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api } from '../lib/tauri'
import { useToastStore } from './toast'
import type { GitLogEntry, GitStatus } from '../types'

export const useGitStore = defineStore('git', () => {
  const status = ref<GitStatus | null>(null)
  const log = ref<GitLogEntry[]>([])
  const showPanel = ref(false)
  const loading = ref(false)

  async function refresh() {
    loading.value = true
    try {
      status.value = await api.gitStatus()
      if (status.value.is_repo) {
        log.value = await api.gitLog(20)
      } else {
        log.value = []
      }
    } catch (e) {
      console.error('git refresh failed', e)
    } finally {
      loading.value = false
    }
  }

  async function init() {
    const toast = useToastStore()
    try {
      status.value = await api.gitInit()
      log.value = await api.gitLog(20)
      toast.success('已初始化 git 仓库 ✓')
    } catch (e: any) {
      toast.error(`初始化失败: ${e?.message ?? e}`)
    }
  }

  async function commit(message: string) {
    const toast = useToastStore()
    if (!message.trim()) {
      toast.error('提交信息不能为空')
      return
    }
    try {
      const entry = await api.gitCommit(message)
      toast.success(`已提交: ${entry.id} · ${entry.summary}`)
      await refresh()
    } catch (e: any) {
      toast.error(`提交失败: ${e?.message ?? e}`)
    }
  }

  function openPanel() { showPanel.value = true }
  function closePanel() { showPanel.value = false }

  return { status, log, showPanel, loading, refresh, init, commit, openPanel, closePanel }
})

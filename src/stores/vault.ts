// Vault store: tracks currently open vault, persists last-used path

import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { api } from '../lib/tauri'
import type { VaultInfo } from '../types'

const STORAGE_KEY = 'notevault.lastVault'

export const useVaultStore = defineStore('vault', () => {
  const info = ref<VaultInfo | null>(null)
  const isOpen = computed(() => info.value !== null)
  const loading = ref(false)
  const error = ref<string | null>(null)

  async function refresh() {
    try {
      info.value = await api.getVaultInfo()
    } catch (e) {
      info.value = null
    }
  }

  async function pickAndOpen() {
    loading.value = true
    error.value = null
    try {
      const path = await api.pickVault()
      if (!path) {
        loading.value = false
        return false
      }
      await open(path)
      return true
    } catch (e: any) {
      error.value = String(e?.message ?? e)
      return false
    } finally {
      loading.value = false
    }
  }

  async function open(path: string) {
    loading.value = true
    error.value = null
    try {
      info.value = await api.openVault(path)
      localStorage.setItem(STORAGE_KEY, path)
    } catch (e: any) {
      error.value = String(e?.message ?? e)
      throw e
    } finally {
      loading.value = false
    }
  }

  async function close() {
    await api.closeVault()
    info.value = null
  }

  async function tryRestoreLast() {
    const last = localStorage.getItem(STORAGE_KEY)
    if (!last) return false
    try {
      await open(last)
      return true
    } catch {
      return false
    }
  }

  return { info, isOpen, loading, error, refresh, pickAndOpen, open, close, tryRestoreLast }
})

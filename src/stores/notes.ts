// Notes store: current note, list, search, save logic

import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api } from '../lib/tauri'
import { useToastStore } from './toast'
import type { NoteContent, NoteMeta, SearchHit, TreeNode } from '../types'

export const useNotesStore = defineStore('notes', () => {
  const current = ref<NoteContent | null>(null)
  const currentPath = ref<string | null>(null)
  const dirty = ref(false)
  const saving = ref(false)

  const tree = ref<TreeNode[]>([])
  const recent = ref<NoteMeta[]>([])

  const searchQuery = ref('')
  const searchResults = ref<SearchHit[]>([])

  async function refreshTree() {
    tree.value = await api.getFileTree()
  }

  async function refreshRecent() {
    recent.value = await api.listNotes()
  }

  async function reindex() {
    const toast = useToastStore()
    try {
      const r = await api.reindexVault()
      toast.success(`索引完成: 新增 ${r.added}，移除 ${r.removed}，总计 ${r.total}`)
      await refreshTree()
      await refreshRecent()
    } catch (e: any) {
      toast.error(`重建索引失败: ${e?.message ?? e}`)
    }
  }

  async function openNote(path: string) {
    // Save current first if dirty
    if (dirty.value && currentPath.value) {
      await save()
    }
    const note = await api.readNote(path)
    current.value = note
    currentPath.value = path
    dirty.value = false
  }

  /** Create a note by name (auto-resolves to inbox or vault root) */
  async function createNoteSimple(name: string) {
    const toast = useToastStore()
    if (!name.trim()) {
      toast.error('笔记名不能为空')
      return null
    }
    try {
      const meta = await api.createNoteSimple(name.trim())
      await refreshTree()
      await refreshRecent()
      await openNote(meta.path)
      toast.success(`已创建: ${meta.title}`)
      return meta
    } catch (e: any) {
      const msg = String(e?.message ?? e)
      toast.error(`创建失败: ${msg}`)
      console.error('createNoteSimple failed', e)
      return null
    }
  }

  /** Create a note at a specific path (more advanced, kept for power use) */
  async function createNote(path: string, template?: string) {
    const toast = useToastStore()
    try {
      const meta = await api.createNote(path, template)
      await refreshTree()
      await openNote(meta.path)
      toast.success(`已创建: ${meta.title}`)
      return meta
    } catch (e: any) {
      const msg = String(e?.message ?? e)
      toast.error(`创建失败: ${msg}`)
      console.error('createNote failed', e)
      return null
    }
  }

  function updateContent(raw: string) {
    if (!current.value) return
    current.value = { ...current.value, raw }
    dirty.value = true
  }

  async function save() {
    if (!current.value || !currentPath.value || !dirty.value) return
    const toast = useToastStore()
    saving.value = true
    try {
      await api.writeNote(currentPath.value, current.value.raw)
      dirty.value = false
      // Re-read to update title, tags, etc.
      const fresh = await api.readNote(currentPath.value)
      current.value = fresh
      await refreshRecent()
    } catch (e: any) {
      const msg = String(e?.message ?? e)
      toast.error(`保存失败: ${msg}`)
      console.error('save failed', e)
    } finally {
      saving.value = false
    }
  }

  async function deleteNote(path: string) {
    const toast = useToastStore()
    try {
      await api.deleteNote(path)
      if (currentPath.value === path) {
        current.value = null
        currentPath.value = null
        dirty.value = false
      }
      await refreshTree()
      await refreshRecent()
      toast.success('已删除')
    } catch (e: any) {
      const msg = String(e?.message ?? e)
      toast.error(`删除失败: ${msg}`)
    }
  }

  let searchTimer: ReturnType<typeof setTimeout> | null = null
  function setSearchQuery(q: string) {
    searchQuery.value = q
    if (searchTimer) clearTimeout(searchTimer)
    if (!q.trim()) {
      searchResults.value = []
      return
    }
    searchTimer = setTimeout(async () => {
      try {
        searchResults.value = await api.search(q, 30)
      } catch (e) {
        console.error('search failed', e)
      }
    }, 150)
  }

  return {
    current,
    currentPath,
    dirty,
    saving,
    tree,
    recent,
    searchQuery,
    searchResults,
    refreshTree,
    refreshRecent,
    reindex,
    openNote,
    createNote,
    createNoteSimple,
    updateContent,
    save,
    deleteNote,
    setSearchQuery,
  }
})

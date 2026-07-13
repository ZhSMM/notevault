// UI store: theme, layout, panels, search mode

import { defineStore } from 'pinia'
import { ref, watch } from 'vue'

type Theme = 'light' | 'dark'
export type SearchMode = 'search' | 'new'

export const useUiStore = defineStore('ui', () => {
  const theme = ref<Theme>(
    (localStorage.getItem('notevault.theme') as Theme) ?? 'light',
  )
  const showPreview = ref(true)
  const showSearch = ref(false)
  const searchMode = ref<SearchMode>('search')
  const sidebarWidth = ref(parseInt(localStorage.getItem('notevault.sidebarWidth') ?? '280', 10))
  const previewWidth = ref(parseInt(localStorage.getItem('notevault.previewWidth') ?? '480', 10))

  function applyTheme(t: Theme) {
    document.documentElement.classList.toggle('dark', t === 'dark')
  }
  applyTheme(theme.value)

  function toggleTheme() {
    theme.value = theme.value === 'light' ? 'dark' : 'light'
  }
  watch(theme, (t) => {
    localStorage.setItem('notevault.theme', t)
    applyTheme(t)
  })
  watch(sidebarWidth, (w) => localStorage.setItem('notevault.sidebarWidth', String(w)))
  watch(previewWidth, (w) => localStorage.setItem('notevault.previewWidth', String(w)))

  function openSearch(opts?: { mode?: SearchMode }) {
    showSearch.value = true
    searchMode.value = opts?.mode ?? 'search'
  }
  function closeSearch() {
    showSearch.value = false
    searchMode.value = 'search'
  }
  function toggleSearch() {
    if (showSearch.value) closeSearch()
    else openSearch()
  }
  function togglePreview() { showPreview.value = !showPreview.value }

  return {
    theme,
    showPreview,
    showSearch,
    searchMode,
    sidebarWidth,
    previewWidth,
    toggleTheme,
    openSearch,
    closeSearch,
    toggleSearch,
    togglePreview,
  }
})

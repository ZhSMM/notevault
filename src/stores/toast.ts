// Toast store: simple non-blocking notifications (success / error / info)

import { defineStore } from 'pinia'
import { ref } from 'vue'

export interface Toast {
  id: number
  type: 'success' | 'error' | 'info'
  message: string
  ttl: number
}

let nextId = 1

export const useToastStore = defineStore('toast', () => {
  const items = ref<Toast[]>([])

  function show(type: Toast['type'], message: string, ttl = 3000) {
    const id = nextId++
    items.value.push({ id, type, message, ttl })
    setTimeout(() => {
      items.value = items.value.filter(t => t.id !== id)
    }, ttl)
  }

  function success(message: string) { show('success', message) }
  function error(message: string)   { show('error', message, 5000) }
  function info(message: string)    { show('info', message) }

  function dismiss(id: number) {
    items.value = items.value.filter(t => t.id !== id)
  }

  return { items, success, error, info, dismiss }
})

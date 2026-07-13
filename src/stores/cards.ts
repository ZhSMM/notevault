// Cards store: due cards, review queue, stats

import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { api } from '../lib/tauri'
import { useToastStore } from './toast'
import type { Card, CardStats, ReviewResult } from '../types'

export const useCardsStore = defineStore('cards', () => {
  const dueCards = ref<Card[]>([])
  const currentIndex = ref(0)
  const revealed = ref(false)
  const loading = ref(false)
  const stats = ref<CardStats | null>(null)
  const showReviewPanel = ref(false)
  const lastReview = ref<ReviewResult | null>(null)

  const currentCard = computed<Card | null>(() => {
    return dueCards.value[currentIndex.value] ?? null
  })

  const remaining = computed(() => Math.max(0, dueCards.value.length - currentIndex.value))
  const progress = computed(() => {
    if (dueCards.value.length === 0) return { done: 0, total: 0, pct: 0 }
    return {
      done: currentIndex.value,
      total: dueCards.value.length,
      pct: Math.round((currentIndex.value / dueCards.value.length) * 100),
    }
  })

  async function loadDue(limit = 50) {
    loading.value = true
    try {
      dueCards.value = await api.listDueCards(limit)
      currentIndex.value = 0
      revealed.value = false
    } catch (e) {
      console.error('loadDue failed', e)
    } finally {
      loading.value = false
    }
  }

  async function loadStats() {
    try {
      stats.value = await api.cardStats()
    } catch (e) {
      console.error('loadStats failed', e)
    }
  }

  async function startReview() {
    await loadDue()
    showReviewPanel.value = true
  }

  function closeReview() {
    showReviewPanel.value = false
    revealed.value = false
  }

  function reveal() {
    revealed.value = true
  }

  async function rate(rating: 1 | 2 | 3 | 4) {
    const card = currentCard.value
    if (!card) return
    const toast = useToastStore()
    try {
      const result = await api.reviewCard(card.id, rating)
      lastReview.value = result
      revealed.value = false
      currentIndex.value += 1
      // After review, refresh stats in background
      loadStats()
      if (remaining.value === 0) {
        toast.success('本次复习完成！🎉')
      } else if (rating === 1) {
        toast.info(`已标记 Again — 10 分钟后复习`)
      } else if (rating === 3) {
        toast.info(`Good — ${result.next_due_in_days.toFixed(1)} 天后再来`)
      }
    } catch (e: any) {
      const msg = String(e?.message ?? e)
      toast.error(`复习失败: ${msg}`)
    }
  }

  async function reindex(notePath?: string) {
    const toast = useToastStore()
    try {
      const r = await api.reindexCards(notePath)
      toast.success(`卡组重建: ${r.notes_indexed} 笔记 / ${r.cards_total} 卡`)
      await loadStats()
      if (!notePath) await loadDue()
    } catch (e: any) {
      toast.error(`重建失败: ${e?.message ?? e}`)
    }
  }

  return {
    dueCards,
    currentIndex,
    currentCard,
    revealed,
    loading,
    stats,
    showReviewPanel,
    lastReview,
    remaining,
    progress,
    loadDue,
    loadStats,
    startReview,
    closeReview,
    reveal,
    rate,
    reindex,
  }
})

<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount } from 'vue'
import { useCardsStore } from '../stores/cards'
import { useNotesStore } from '../stores/notes'

const cards = useCardsStore()
const notes = useNotesStore()

onMounted(async () => {
  await cards.loadStats()
})

function formatCloze(text: string, revealed: boolean): string {
  const re = /\{\{c\d+::([^}]*?)(?:::([^}]*?))?\}\}/g
  return text.replace(re, (_, hidden, hint) => {
    if (revealed) return `<span class="px-1 rounded bg-accent/20 text-accent">${hint || hidden}</span>`
    return `<span class="px-1 rounded bg-bg-muted text-fg-subtle">${'_'.repeat(Math.max(3, (hidden || '').length))}</span>`
  })
}

function onKey(e: KeyboardEvent) {
  if (!cards.showReviewPanel) return
  // Don't intercept when typing in inputs
  const target = e.target as HTMLElement
  if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA') return

  if (e.key === ' ' || e.key === 'Enter') {
    e.preventDefault()
    if (!cards.revealed) cards.reveal()
  } else if (e.key === '1') {
    if (cards.revealed) cards.rate(1)
  } else if (e.key === '2') {
    if (cards.revealed) cards.rate(2)
  } else if (e.key === '3') {
    if (cards.revealed) cards.rate(3)
  } else if (e.key === '4') {
    if (cards.revealed) cards.rate(4)
  } else if (e.key === 'Escape') {
    cards.closeReview()
  }
}

onMounted(() => {
  window.addEventListener('keydown', onKey)
})
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKey)
})

const card = computed(() => cards.currentCard)
const sourceNote = computed(() => {
  const c = card.value
  if (!c) return null
  return notes.recent.find(n => n.path === c.note_path) ?? null
})

function openSource() {
  const c = card.value
  if (c) {
    notes.openNote(c.note_path)
  }
}

function againHint(rating: 1 | 2 | 3 | 4): string {
  if (!card.value) return ''
  switch (rating) {
    case 1: return '~10 min'
    case 2: return '< 1 day'
    case 3: return `${Math.max(1, Math.round(card.value.interval_days))} day${card.value.interval_days >= 2 ? 's' : ''}`
    case 4: return `${Math.max(4, Math.round(card.value.interval_days * card.value.stability))} days`
  }
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="cards.showReviewPanel"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm"
      @click.self="cards.closeReview()"
    >
      <div class="w-[680px] max-w-[92vw] max-h-[88vh] bg-bg border border-border rounded-xl shadow-2xl flex flex-col overflow-hidden">
        <!-- Header -->
        <div class="px-5 py-3 border-b border-border flex items-center gap-3 bg-bg-soft">
          <span class="text-xl">🎓</span>
          <div class="flex-1">
            <div class="text-sm font-medium">间隔复习 (FSRS)</div>
            <div class="text-xs text-fg-subtle">
              {{ cards.progress.done }} / {{ cards.progress.total }} · 还剩 {{ cards.remaining }} 张
            </div>
          </div>
          <div class="text-xs text-fg-subtle">
            <kbd class="px-1.5 py-0.5 rounded bg-bg-muted border border-border">Space</kbd> 翻面
            <kbd class="ml-1 px-1.5 py-0.5 rounded bg-bg-muted border border-border">1-4</kbd> 评分
          </div>
          <button class="icon-btn" @click="cards.closeReview()" title="关闭 (Esc)">✕</button>
        </div>

        <!-- Progress bar -->
        <div class="h-1 bg-bg-muted">
          <div
            class="h-full bg-accent transition-all"
            :style="{ width: cards.progress.pct + '%' }"
          />
        </div>

        <!-- Body -->
        <div class="flex-1 overflow-y-auto p-6 flex items-center justify-center">
          <!-- Empty / done state -->
          <div v-if="!card" class="text-center py-12">
            <div class="text-5xl mb-3">🎉</div>
            <h3 class="text-lg font-medium mb-2">
              {{ cards.dueCards.length === 0 ? '没有需要复习的卡片' : '本次复习完成！' }}
            </h3>
            <p class="text-sm text-fg-muted mb-4">
              {{ cards.dueCards.length === 0
                ? '写新的笔记时加上 #card 标记，就能自动生成闪卡。'
                : '所有到期的卡片都已复习。下次有新的到期再回来。' }}
            </p>
            <div class="flex gap-2 justify-center">
              <button class="btn" @click="cards.closeReview()">关闭</button>
              <button v-if="cards.dueCards.length === 0" class="btn btn-primary" @click="cards.reindex()">
                重建卡组
              </button>
            </div>
          </div>

          <!-- Card -->
          <div v-else class="w-full max-w-2xl">
            <div class="text-xs text-fg-subtle mb-2 flex items-center gap-2">
              <span class="px-1.5 py-0.5 rounded bg-bg-muted border border-border uppercase tracking-wide">
                {{ card.card_type === 'basic' ? '基础卡' : card.card_type === 'reverse' ? '双向卡' : '填空卡' }}
              </span>
              <button
                v-if="sourceNote"
                class="hover:text-accent truncate"
                @click="openSource"
                :title="card.note_path"
              >
                来自：{{ sourceNote.title }}
              </button>
              <span v-else class="truncate">{{ card.note_path }}</span>
            </div>

            <!-- Question -->
            <div class="rounded-lg border border-border bg-bg-soft p-5 min-h-[120px]">
              <div class="text-sm text-fg-muted mb-1">问题</div>
              <div class="text-lg leading-relaxed">
                <span v-if="card.card_type === 'cloze' && card.cloze_text" v-html="formatCloze(card.cloze_text, cards.revealed)" />
                <span v-else>{{ card.question }}</span>
              </div>
            </div>

            <!-- Answer (revealed) -->
            <div v-if="cards.revealed" class="mt-3 rounded-lg border border-accent/30 bg-accent/5 p-5">
              <div class="text-sm text-accent mb-1">答案</div>
              <div class="text-base leading-relaxed">{{ card.answer || '(无)' }}</div>
            </div>

            <!-- Last review feedback -->
            <div v-if="cards.lastReview" class="mt-3 px-3 py-2 rounded bg-bg-muted text-xs text-fg-muted">
              ✓ 已记录：稳定性 {{ cards.lastReview.card.stability.toFixed(1) }} ·
              下次：{{ cards.lastReview.next_due_in_days.toFixed(1) }} 天
            </div>
          </div>
        </div>

        <!-- Footer: rating buttons -->
        <div v-if="card" class="border-t border-border p-3 grid grid-cols-4 gap-2 bg-bg-soft">
          <button
            class="flex flex-col items-center py-2.5 px-2 rounded-md border-2 border-red-300 dark:border-red-700
                   bg-red-50 dark:bg-red-950/30 hover:bg-red-100 dark:hover:bg-red-900/50
                   disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
            :disabled="!cards.revealed"
            @click="cards.rate(1)"
            :title="!cards.revealed ? '先按 Space 翻面' : '再次评分 (1)'"
          >
            <div class="text-sm font-bold text-red-700 dark:text-red-300">Again</div>
            <div class="text-[10px] text-red-600/70 dark:text-red-400/70 mt-0.5">{{ againHint(1) }}</div>
            <div class="text-[10px] text-fg-subtle mt-0.5"><kbd>1</kbd></div>
          </button>
          <button
            class="flex flex-col items-center py-2.5 px-2 rounded-md border-2 border-amber-300 dark:border-amber-700
                   bg-amber-50 dark:bg-amber-950/30 hover:bg-amber-100 dark:hover:bg-amber-900/50
                   disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
            :disabled="!cards.revealed"
            @click="cards.rate(2)"
            :title="!cards.revealed ? '先按 Space 翻面' : 'Hard (2)'"
          >
            <div class="text-sm font-bold text-amber-700 dark:text-amber-300">Hard</div>
            <div class="text-[10px] text-amber-600/70 dark:text-amber-400/70 mt-0.5">{{ againHint(2) }}</div>
            <div class="text-[10px] text-fg-subtle mt-0.5"><kbd>2</kbd></div>
          </button>
          <button
            class="flex flex-col items-center py-2.5 px-2 rounded-md border-2 border-emerald-300 dark:border-emerald-700
                   bg-emerald-50 dark:bg-emerald-950/30 hover:bg-emerald-100 dark:hover:bg-emerald-900/50
                   disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
            :disabled="!cards.revealed"
            @click="cards.rate(3)"
            :title="!cards.revealed ? '先按 Space 翻面' : 'Good (3)'"
          >
            <div class="text-sm font-bold text-emerald-700 dark:text-emerald-300">Good</div>
            <div class="text-[10px] text-emerald-600/70 dark:text-emerald-400/70 mt-0.5">{{ againHint(3) }}</div>
            <div class="text-[10px] text-fg-subtle mt-0.5"><kbd>3</kbd></div>
          </button>
          <button
            class="flex flex-col items-center py-2.5 px-2 rounded-md border-2 border-blue-300 dark:border-blue-700
                   bg-blue-50 dark:bg-blue-950/30 hover:bg-blue-100 dark:hover:bg-blue-900/50
                   disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
            :disabled="!cards.revealed"
            @click="cards.rate(4)"
            :title="!cards.revealed ? '先按 Space 翻面' : 'Easy (4)'"
          >
            <div class="text-sm font-bold text-blue-700 dark:text-blue-300">Easy</div>
            <div class="text-[10px] text-blue-600/70 dark:text-blue-400/70 mt-0.5">{{ againHint(4) }}</div>
            <div class="text-[10px] text-fg-subtle mt-0.5"><kbd>4</kbd></div>
          </button>
        </div>

        <!-- Reveal button when not revealed -->
        <div v-if="card && !cards.revealed" class="border-t border-border p-3 bg-bg-soft">
          <button class="btn btn-primary w-full justify-center py-2" @click="cards.reveal()">
            <span>翻面查看答案</span>
            <kbd class="text-xs opacity-60 ml-2">Space</kbd>
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

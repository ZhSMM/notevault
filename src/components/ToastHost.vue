<script setup lang="ts">
import { useToastStore } from '../stores/toast'

const toast = useToastStore()

const ICONS: Record<string, string> = {
  success: '✓',
  error: '✕',
  info: 'ⓘ',
}
</script>

<template>
  <Teleport to="body">
    <div class="fixed top-4 right-4 z-[100] flex flex-col gap-2 pointer-events-none">
      <transition-group name="toast">
        <div
          v-for="t in toast.items"
          :key="t.id"
          class="pointer-events-auto px-3 py-2 rounded-md shadow-lg text-sm border flex items-center gap-2 min-w-[240px] max-w-[400px]"
          :class="{
            'bg-emerald-50 border-emerald-300 text-emerald-900 dark:bg-emerald-950 dark:border-emerald-700 dark:text-emerald-100': t.type === 'success',
            'bg-red-50 border-red-300 text-red-900 dark:bg-red-950 dark:border-red-700 dark:text-red-100': t.type === 'error',
            'bg-bg border-border text-fg': t.type === 'info',
          }"
          @click="toast.dismiss(t.id)"
        >
          <span class="font-bold text-base leading-none">{{ ICONS[t.type] }}</span>
          <span class="flex-1">{{ t.message }}</span>
        </div>
      </transition-group>
    </div>
  </Teleport>
</template>

<style scoped>
.toast-enter-active, .toast-leave-active {
  transition: all 0.2s ease;
}
.toast-enter-from, .toast-leave-to {
  opacity: 0;
  transform: translateX(20px);
}
</style>

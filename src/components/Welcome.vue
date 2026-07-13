<script setup lang="ts">
import { useVaultStore } from '../stores/vault'
import { useUiStore } from '../stores/ui'

const vault = useVaultStore()
const ui = useUiStore()
</script>

<template>
  <div class="h-full w-full flex flex-col items-center justify-center bg-bg p-8">
    <div class="max-w-md text-center">
      <div class="text-6xl mb-4">📓</div>
      <h1 class="text-3xl font-semibold mb-2">NoteVault</h1>
      <p class="text-fg-muted mb-8">本地优先的笔记应用，支持双向链接、间隔重复与 AI 查询</p>

      <div class="flex flex-col gap-3">
        <button
          class="btn btn-primary justify-center py-2"
          :disabled="vault.loading"
          @click="vault.pickAndOpen()"
        >
          <span v-if="vault.loading">打开中...</span>
          <span v-else>📁 打开 Vault（笔记目录）</span>
        </button>

        <p v-if="vault.error" class="text-red-500 text-sm mt-2">
          {{ vault.error }}
        </p>
      </div>

      <div class="mt-12 text-xs text-fg-subtle">
        <p>建议：把笔记存在一个 git 仓库里，这样多设备同步 + 历史回溯都白送。</p>
      </div>
    </div>

    <button
      class="absolute top-4 right-4 icon-btn"
      @click="ui.toggleTheme()"
      :title="ui.theme === 'dark' ? '切到亮色' : '切到暗色'"
    >
      {{ ui.theme === 'dark' ? '☀️' : '🌙' }}
    </button>
  </div>
</template>

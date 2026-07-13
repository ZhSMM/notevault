<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useGitStore } from '../stores/git'
import { useVaultStore } from '../stores/vault'

const git = useGitStore()
const vault = useVaultStore()

const commitMsg = ref('')

onMounted(() => {
  if (vault.isOpen) git.refresh()
})

watch(() => vault.isOpen, (open) => {
  if (open) git.refresh()
})

function formatTime(ts: number): string {
  if (!ts) return ''
  const d = new Date(ts * 1000)
  const now = new Date()
  const sameDay = d.toDateString() === now.toDateString()
  if (sameDay) return d.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })
  return d.toLocaleDateString('zh-CN', { month: '2-digit', day: '2-digit' })
}

const isRepo = computed(() => git.status?.is_repo ?? false)
const dirtyCount = computed(() => {
  if (!git.status) return 0
  return git.status.modified.length + git.status.untracked.length
})
</script>

<template>
  <Teleport to="body">
    <div
      v-if="git.showPanel"
      class="fixed inset-0 z-40 flex items-start justify-center pt-20 bg-black/30"
      @click.self="git.closePanel()"
    >
      <div class="w-[640px] max-w-[92vw] max-h-[80vh] bg-bg border border-border rounded-lg shadow-2xl flex flex-col overflow-hidden">
        <div class="px-4 py-2.5 border-b border-border flex items-center gap-3 bg-bg-soft">
          <span class="text-base">⎇</span>
          <div class="flex-1">
            <div class="text-sm font-medium">Git 历史</div>
            <div class="text-xs text-fg-subtle" v-if="isRepo">
              {{ git.status?.branch ?? git.status?.head ?? '?' }}
              <span v-if="git.status?.ahead" class="text-emerald-500 ml-1">↑{{ git.status.ahead }}</span>
              <span v-if="git.status?.behind" class="text-amber-500 ml-1">↓{{ git.status.behind }}</span>
              <span v-if="dirtyCount > 0" class="text-amber-500 ml-1">● {{ dirtyCount }} 个改动</span>
            </div>
            <div class="text-xs text-fg-subtle" v-else>还不是 git 仓库</div>
          </div>
          <button class="icon-btn" @click="git.closePanel()">✕</button>
        </div>

        <!-- Not a repo: prompt to init -->
        <div v-if="!isRepo" class="p-6 text-center">
          <div class="text-4xl mb-3">⎇</div>
          <h3 class="text-lg font-medium mb-2">初始化 git 仓库？</h3>
          <p class="text-sm text-fg-muted mb-4">
            NoteVault 会自动创建 <code>.gitignore</code>（忽略 <code>.notevault/</code>、<code>.config/</code> 等）。<br>
            之后每次保存会自动 commit，多设备同步开箱即用。
          </p>
          <button class="btn btn-primary" @click="git.init()">
            初始化 + 创建初始 commit
          </button>
        </div>

        <!-- Repo exists: show log + commit UI -->
        <div v-else class="flex-1 flex flex-col overflow-hidden">
          <!-- Commit form -->
          <div class="px-4 py-3 border-b border-border bg-bg-soft">
            <div class="flex gap-2">
              <input
                v-model="commitMsg"
                class="input flex-1"
                placeholder="提交信息（描述这次的改动）"
                @keydown.enter="git.commit(commitMsg); commitMsg = ''"
              />
              <button class="btn btn-primary" @click="git.commit(commitMsg); commitMsg = ''">
                提交
              </button>
            </div>
            <p class="text-xs text-fg-subtle mt-1.5">
              写笔记时已自动 commit。这里提交额外的整体改动。
            </p>
          </div>

          <!-- Log -->
          <div class="flex-1 overflow-y-auto scrollbar-thin">
            <div v-if="git.log.length === 0" class="px-4 py-8 text-center text-fg-subtle text-sm">
              还没有 commit
            </div>
            <div v-else>
              <div
                v-for="entry in git.log"
                :key="entry.full_id"
                class="px-4 py-2 border-b border-border last:border-b-0 hover:bg-bg-soft"
              >
                <div class="flex items-baseline gap-2">
                  <code class="text-xs font-mono text-accent">{{ entry.id }}</code>
                  <span class="text-sm flex-1 truncate">{{ entry.summary }}</span>
                </div>
                <div class="text-xs text-fg-subtle mt-0.5 flex items-center gap-2">
                  <span>{{ entry.author }}</span>
                  <span>·</span>
                  <span>{{ formatTime(entry.timestamp) }}</span>
                  <span v-if="entry.files_changed > 0">· {{ entry.files_changed }} 文件</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </Teleport>
</template>

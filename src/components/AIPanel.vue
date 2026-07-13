<script setup lang="ts">
import { nextTick, ref, watch } from 'vue'
import { useAiStore, type AiSource } from '../stores/ai'
import { useNotesStore } from '../stores/notes'
import { useCardsStore } from '../stores/cards'

const ai = useAiStore()
const notes = useNotesStore()
const cards = useCardsStore()

const tab = ref<'chat' | 'cards'>('chat')
const cardCount = ref(5)
const accepted = ref<Set<number>>(new Set())
const expanded = ref<Set<number>>(new Set())

watch(
  () => ai.cardDraftsFor,
  () => {
    if (ai.cardDrafts.length) {
      accepted.value = new Set(ai.cardDrafts.map((_, i) => i))
    } else {
      accepted.value = new Set()
    }
    expanded.value = new Set()
  },
)

async function startGenerate() {
  if (!notes.currentPath) {
    alert('请先打开一篇笔记')
    return
  }
  tab.value = 'cards'
  await ai.generateCards(notes.currentPath, cardCount.value)
}

function toggleAccepted(i: number) {
  const s = new Set(accepted.value)
  if (s.has(i)) s.delete(i)
  else s.add(i)
  accepted.value = s
}

function toggleExpanded(i: number) {
  const s = new Set(expanded.value)
  if (s.has(i)) s.delete(i)
  else s.add(i)
  expanded.value = s
}

async function commitCards() {
  const picked = ai.cardDrafts.filter((_, i) => accepted.value.has(i))
  if (picked.length === 0) return
  const n = await ai.commitCardDrafts(picked)
  if (n > 0) {
    tab.value = 'chat'
    await cards.loadStats()
  }
}

const messagesEl = ref<HTMLElement | null>(null)
const inputEl = ref<HTMLTextAreaElement | null>(null)

const QUICK_PROMPTS = [
  { label: '总结最近一周', text: '总结我最近一周写的笔记，按主题归类。' },
  { label: '找 X 相关', text: '找出所有和「Rust 所有权」相关的笔记，并指出它们之间的关联。' },
  { label: '找薄弱点', text: '基于我的笔记，找出还没掌握清楚或者记错的地方。' },
  { label: '生成闪卡', text: '从最近的笔记里挑出 5 个核心概念，生成闪卡。' },
  { label: '建周报', text: '把本月的日志聚合成一份周报草稿。' },
]

watch(
  () => ai.messages.length,
  async () => {
    await nextTick()
    if (messagesEl.value) {
      messagesEl.value.scrollTop = messagesEl.value.scrollHeight
    }
  },
)

watch(
  () => ai.showPanel,
  (v) => {
    if (v) {
      setTimeout(() => inputEl.value?.focus(), 80)
    }
  },
)

function onSend() {
  ai.send()
}

function onQuick(text: string) {
  ai.send(text)
}

function onKey(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    onSend()
  }
}

function openSource(src: AiSource) {
  notes.openNote(src.path).catch(() => {})
  ai.closePanel()
}

function fmtTime(ts: number) {
  const d = new Date(ts)
  return d.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })
}
</script>

<template>
  <div
    v-if="ai.showPanel"
    class="fixed inset-0 z-40 flex bg-black/40 backdrop-blur-sm"
    @click.self="ai.closePanel()"
  >
    <div
      class="ml-auto h-full w-full max-w-[640px] flex flex-col bg-bg border-l border-border shadow-2xl"
    >
      <!-- 顶部 -->
      <header class="h-12 px-4 flex items-center gap-2 border-b border-border bg-bg-soft">
        <span class="text-base font-medium">🤖 AI 助手</span>
        <span
          class="text-xs px-1.5 py-0.5 rounded bg-accent/10 text-accent"
          :title="`${ai.config.provider} · ${ai.config.model}`"
        >
          {{ ai.config.model || '未配置' }}
        </span>
        <!-- Tabs -->
        <div class="flex items-center gap-1 ml-2 border-l border-border pl-2">
          <button
            class="text-xs px-2 py-0.5 rounded"
            :class="tab === 'chat' ? 'bg-accent text-white' : 'text-fg-subtle hover:text-fg'"
            @click="tab = 'chat'"
          >💬 聊天</button>
          <button
            class="text-xs px-2 py-0.5 rounded"
            :class="tab === 'cards' ? 'bg-accent text-white' : 'text-fg-subtle hover:text-fg'"
            @click="tab = 'cards'"
            title="为当前笔记生成闪卡"
          >🪄 建卡</button>
        </div>
        <span v-if="ai.lastError" class="text-xs text-red-500 ml-1 truncate">
          {{ ai.lastError }}
        </span>
        <div class="flex-1" />
        <button v-if="tab === 'chat'" class="text-xs hover:text-fg" @click="ai.clearChat()" title="清空对话">
          🗑️ 清空
        </button>
        <button
          class="text-xs hover:text-fg"
          :class="ai.showSettings ? 'text-accent' : ''"
          @click="ai.toggleSettings()"
          title="AI 设置"
        >
          ⚙️ 设置
        </button>
        <button class="text-xs hover:text-fg" @click="ai.closePanel()" title="关闭 (Esc)">
          ✕
        </button>
      </header>

      <!-- 设置面板 -->
      <div
        v-if="ai.showSettings"
        class="px-4 py-3 border-b border-border bg-bg-soft/50 space-y-2 text-sm"
      >
        <div class="flex items-center gap-2">
          <label class="w-20 text-fg-subtle">服务商</label>
          <select
            class="flex-1 bg-bg border border-border rounded px-2 py-1"
            :value="ai.config.provider"
            @change="(e: any) => { ai.setConfig({ provider: e.target.value }); ai.loadModels() }"
          >
            <option value="ollama">Ollama（本地）</option>
            <option value="openai">OpenAI 兼容（云端）</option>
          </select>
          <button class="text-xs hover:text-fg" @click="ai.loadModels()" title="刷新模型列表">
            ⟳
          </button>
        </div>
        <div class="flex items-center gap-2">
          <label class="w-20 text-fg-subtle">服务地址</label>
          <input
            class="flex-1 bg-bg border border-border rounded px-2 py-1 font-mono text-xs"
            :value="ai.config.base_url"
            @input="(e: any) => ai.setConfig({ base_url: e.target.value })"
            placeholder="http://localhost:11434"
          />
        </div>
        <div v-if="ai.config.provider === 'openai'" class="flex items-center gap-2">
          <label class="w-20 text-fg-subtle">API Key</label>
          <input
            type="password"
            class="flex-1 bg-bg border border-border rounded px-2 py-1 font-mono text-xs"
            :value="ai.config.api_key"
            @input="(e: any) => ai.setConfig({ api_key: e.target.value })"
            placeholder="sk-..."
          />
        </div>
        <div class="flex items-center gap-2">
          <label class="w-20 text-fg-subtle">模型</label>
          <input
            list="ai-models"
            class="flex-1 bg-bg border border-border rounded px-2 py-1 text-xs"
            :value="ai.config.model"
            @input="(e: any) => ai.setConfig({ model: e.target.value })"
            :placeholder="ai.availableModels.length ? '选择或输入' : '请先刷新'"
          />
          <datalist id="ai-models">
            <option v-for="m in ai.availableModels" :key="m" :value="m" />
          </datalist>
        </div>
        <details class="text-xs">
          <summary class="cursor-pointer text-fg-subtle">高级参数</summary>
          <div class="grid grid-cols-3 gap-2 pt-2">
            <label>
              <div class="text-fg-subtle">Top-K</div>
              <input
                type="number"
                min="1"
                max="50"
                class="w-full bg-bg border border-border rounded px-1.5 py-0.5"
                :value="ai.config.top_k"
                @input="(e: any) => ai.setConfig({ top_k: +e.target.value })"
              />
            </label>
            <label>
              <div class="text-fg-subtle">温度</div>
              <input
                type="number"
                step="0.1"
                min="0"
                max="2"
                class="w-full bg-bg border border-border rounded px-1.5 py-0.5"
                :value="ai.config.temperature"
                @input="(e: any) => ai.setConfig({ temperature: +e.target.value })"
              />
            </label>
            <label>
              <div class="text-fg-subtle">最大长度</div>
              <input
                type="number"
                min="64"
                max="8192"
                step="64"
                class="w-full bg-bg border border-border rounded px-1.5 py-0.5"
                :value="ai.config.max_tokens"
                @input="(e: any) => ai.setConfig({ max_tokens: +e.target.value })"
              />
            </label>
          </div>
        </details>
        <div>
          <label class="text-fg-subtle text-xs">系统提示词</label>
          <textarea
            class="w-full bg-bg border border-border rounded px-2 py-1 text-xs font-mono h-16"
            :value="ai.config.system_prompt"
            @input="(e: any) => ai.setConfig({ system_prompt: e.target.value })"
          />
        </div>
      </div>

      <!-- 消息区 (chat tab) -->
      <div v-if="tab === 'chat'" ref="messagesEl" class="flex-1 overflow-y-auto px-4 py-3 space-y-3">
        <div v-if="!ai.hasMessages" class="text-center text-fg-subtle py-8 space-y-3">
          <div class="text-3xl">🤖</div>
          <p>问点关于你笔记的问题，比如：</p>
          <div class="flex flex-wrap gap-2 justify-center pt-1">
            <button
              v-for="p in QUICK_PROMPTS"
              :key="p.label"
              class="text-xs px-2 py-1 rounded bg-bg-soft hover:bg-accent/10 hover:text-accent border border-border"
              @click="onQuick(p.text)"
            >
              {{ p.label }}
            </button>
          </div>
        </div>

        <div
          v-for="m in ai.messages"
          :key="m.id"
          class="space-y-1"
          :class="m.role === 'user' ? 'flex flex-col items-end' : ''"
        >
          <div
            class="max-w-[88%] rounded-lg px-3 py-2 text-sm whitespace-pre-wrap break-words"
            :class="
              m.role === 'user'
                ? 'bg-accent text-white'
                : m.error
                ? 'bg-red-500/10 text-red-600'
                : 'bg-bg-soft border border-border'
            "
          >
            <template v-if="m.role === 'assistant' && !m.content && !m.error && ai.pending">
              <span class="inline-block animate-pulse text-fg-subtle">▍思考中…</span>
            </template>
            <template v-else>
              <span v-if="m.error">⚠️ {{ m.error }}</span>
              <span v-else>{{ m.content }}</span>
            </template>
          </div>

          <!-- 来源 -->
          <div
            v-if="m.sources && m.sources.length"
            class="max-w-[88%] text-[11px] text-fg-subtle space-y-0.5"
          >
            <div class="font-medium">引用来源：</div>
            <button
              v-for="(s, i) in m.sources"
              :key="i"
              class="block w-full text-left px-2 py-1 rounded bg-bg-soft/50 hover:bg-accent/10 hover:text-accent border border-border/50"
              @click="openSource(s)"
              :title="s.path"
            >
              <div class="font-medium truncate">
                {{ s.title || s.path }}
                <span v-if="s.block_id" class="text-accent">#{{ s.block_id }}</span>
              </div>
              <div class="truncate opacity-70">{{ s.snippet }}</div>
            </button>
          </div>

          <div v-if="m.role === 'user'" class="text-[10px] text-fg-subtle pr-1">
            {{ fmtTime(m.ts) }}
          </div>
        </div>
      </div>

      <!-- 闪卡生成 (cards tab) -->
      <div v-if="tab === 'cards'" class="flex-1 overflow-y-auto px-4 py-3 space-y-3 text-sm">
        <div class="flex items-center gap-2">
          <label class="text-fg-subtle">生成数量</label>
          <input
            type="number"
            min="1"
            max="20"
            class="w-16 bg-bg border border-border rounded px-2 py-1 text-center"
            v-model.number="cardCount"
          />
          <button
            class="btn btn-primary text-xs h-7 px-3"
            :disabled="ai.generatingCards || !notes.currentPath"
            @click="startGenerate"
          >
            {{ ai.generatingCards ? '⏳ 生成中…' : '🪄 为当前笔记生成' }}
          </button>
          <span v-if="notes.currentPath" class="text-xs text-fg-subtle truncate flex-1">
            {{ notes.currentPath }}
          </span>
        </div>

        <div v-if="!notes.currentPath" class="text-fg-subtle text-xs">
          请先在左侧打开一篇笔记。
        </div>

        <div v-if="ai.cardDrafts.length > 0" class="space-y-2">
          <div class="flex items-center gap-2 text-xs text-fg-subtle">
            <label class="flex items-center gap-1">
              <input
                type="checkbox"
                :checked="accepted.size === ai.cardDrafts.length"
                @change="(e: any) => {
                  if (e.target.checked) accepted = new Set(ai.cardDrafts.map((_, i) => i))
                  else accepted = new Set()
                }"
              />
              全选 · 已选 <strong class="text-accent">{{ accepted.size }}</strong> / {{ ai.cardDrafts.length }}
            </label>
            <div class="flex-1" />
            <button
              class="btn btn-primary text-xs h-7 px-3"
              :disabled="accepted.size === 0"
              @click="commitCards"
            >
              💾 添加到笔记
            </button>
          </div>

          <div
            v-for="(c, i) in ai.cardDrafts"
            :key="i"
            class="rounded border border-border bg-bg-soft/40 p-2 text-sm"
          >
            <div class="flex items-start gap-2">
              <input
                type="checkbox"
                class="mt-1"
                :checked="accepted.has(i)"
                @change="toggleAccepted(i)"
              />
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2 mb-1">
                  <span
                    class="text-[10px] px-1.5 py-0.5 rounded font-medium"
                    :class="c.cardType === 'reverse' ? 'bg-purple-500/20 text-purple-600' : c.cardType === 'cloze' ? 'bg-green-500/20 text-green-600' : 'bg-accent/20 text-accent'"
                  >{{ c.cardType }}</span>
                </div>
                <div class="font-medium text-fg">{{ c.question }}</div>
                <button
                  class="text-[11px] text-fg-subtle hover:text-accent mt-1"
                  @click="toggleExpanded(i)"
                >
                  {{ expanded.has(i) ? '▾ 收起答案' : '▸ 显示答案' }}
                </button>
                <div v-if="expanded.has(i)" class="mt-1 text-fg-subtle whitespace-pre-wrap">
                  {{ c.answer }}
                </div>
              </div>
            </div>
          </div>
        </div>

        <div v-else-if="!ai.generatingCards" class="text-fg-subtle text-xs">
          还没有生成的卡片。打开一篇笔记，配置好 AI（Ollama / OpenAI），点上方"为当前笔记生成"。
        </div>

        <details v-if="ai.cardDraftsRaw" class="text-[11px] text-fg-muted">
          <summary class="cursor-pointer">查看 LLM 原始输出</summary>
          <pre class="mt-1 p-2 bg-bg/50 rounded whitespace-pre-wrap break-all max-h-32 overflow-y-auto">{{ ai.cardDraftsRaw }}</pre>
        </details>
      </div>

      <!-- 输入区 -->
      <footer v-if="tab === 'chat'" class="border-t border-border bg-bg-soft px-3 py-2">
        <div class="flex items-end gap-2">
          <textarea
            ref="inputEl"
            v-model="ai.inputText"
            rows="2"
            class="flex-1 resize-none bg-bg border border-border rounded px-2 py-1.5 text-sm focus:outline-none focus:border-accent"
            :placeholder="ai.pending ? '正在生成…' : '向笔记提问，回车发送，Shift+Enter 换行'"
            :disabled="ai.pending"
            @keydown="onKey"
          />
          <button
            v-if="ai.pending"
            class="btn btn-secondary text-xs h-8 px-3"
            @click="ai.abort()"
            title="停止生成"
          >
            ⏹ 停止
          </button>
          <button
            v-else
            class="btn btn-primary text-xs h-8 px-3"
            @click="onSend()"
            :disabled="!ai.inputText.trim()"
          >
            发送
          </button>
        </div>
        <div class="text-[10px] text-fg-subtle pt-1 flex items-center gap-2">
          <kbd>Enter</kbd> 发送
          <kbd>Shift+Enter</kbd> 换行
          <span class="ml-auto">本机检索 + 远程推理</span>
        </div>
      </footer>
    </div>
  </div>
</template>

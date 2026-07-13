// AI store: provider config, chat state, conversation history.

import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { aiApi, type AiSource } from '../lib/tauri'
import { api } from '../lib/tauri'
import { useToastStore } from './toast'
import { useNotesStore } from './notes'

export type ChatRole = 'user' | 'assistant' | 'system'

export interface ChatMessage {
  id: string
  role: ChatRole
  content: string
  sources?: AiSource[]
  ts: number
  error?: string
}

export type { AiSource } from '../lib/tauri'

export interface AiConfig {
  provider: 'ollama' | 'openai'
  model: string
  base_url: string
  api_key: string
  system_prompt: string
  top_k: number
  temperature: number
  max_tokens: number
  scope_paths: string[]    // 限制检索范围（笔记路径），空 = 全库
}

const DEFAULT_CONFIG: AiConfig = {
  provider: 'ollama',
  model: 'qwen2.5:7b',
  base_url: 'http://localhost:11434',
  api_key: '',
  system_prompt:
    '你是一个笔记助手，可以访问用户的本地笔记库。\n' +
    '回答时优先引用相关笔记内容，标注来源路径；不要编造。\n' +
    '回答使用中文，简洁清晰。',
  top_k: 8,
  temperature: 0.3,
  max_tokens: 1024,
  scope_paths: [],
}

const CFG_KEY = 'notevault.ai.config'

function loadConfig(): AiConfig {
  try {
    const raw = localStorage.getItem(CFG_KEY)
    if (raw) return { ...DEFAULT_CONFIG, ...JSON.parse(raw) }
  } catch {}
  return { ...DEFAULT_CONFIG }
}

export const useAiStore = defineStore('ai', () => {
  const config = ref<AiConfig>(loadConfig())
  const messages = ref<ChatMessage[]>([])
  const pending = ref(false)
  const showPanel = ref(false)
  const showSettings = ref(false)
  const inputText = ref('')
  const availableModels = ref<string[]>([])
  const modelsLoaded = ref(false)
  const lastError = ref<string | null>(null)
  const inFlight = ref<AbortController | null>(null)

  function saveConfig() {
    localStorage.setItem(CFG_KEY, JSON.stringify(config.value))
  }

  function setConfig(patch: Partial<AiConfig>) {
    config.value = { ...config.value, ...patch }
    saveConfig()
  }

  function openPanel() {
    showPanel.value = true
    if (!modelsLoaded.value) loadModels()
  }
  function closePanel() {
    showPanel.value = false
    if (inFlight.value) {
      inFlight.value.abort()
      inFlight.value = null
      pending.value = false
    }
  }
  function toggleSettings() {
    showSettings.value = !showSettings.value
    if (showSettings.value && !modelsLoaded.value) loadModels()
  }
  function clearChat() {
    messages.value = []
  }

  // ---- Card generation ----
  const generatingCards = ref(false)
  const cardDrafts = ref<import('../lib/tauri').GeneratedCard[]>([])
  const cardDraftsFor = ref<string | null>(null)   // note path
  const cardDraftsRaw = ref<string>('')

  async function generateCards(notePath: string, count = 6) {
    const toast = useToastStore()
    if (!config.value.base_url) {
      toast.error('请先在 AI 设置中配置服务地址')
      return false
    }
    generatingCards.value = true
    cardDraftsFor.value = notePath
    cardDrafts.value = []
    cardDraftsRaw.value = ''
    try {
      const r = await api.aiGenerateCards({
        provider: config.value.provider,
        model: config.value.model,
        baseUrl: config.value.base_url,
        apiKey: config.value.api_key,
        notePath,
        count,
        language: '中文',
      })
      cardDrafts.value = r.cards
      cardDraftsRaw.value = r.raw
      if (r.cards.length === 0) {
        toast.error('LLM 没返回任何卡片')
        return false
      }
      return true
    } catch (e: any) {
      toast.error('生成失败：' + (e?.message ?? e))
      return false
    } finally {
      generatingCards.value = false
    }
  }

  function clearCardDrafts() {
    cardDrafts.value = []
    cardDraftsFor.value = null
    cardDraftsRaw.value = ''
  }

  /**
   * Append selected drafts to the note as `#card` / `#card-reverse` / `#cloze` blocks,
   * then re-extract cards for the note (FSRS state is preserved by content hash).
   * Returns the count of inserted cards.
   */
  async function commitCardDrafts(accepted: import('../lib/tauri').GeneratedCard[]) {
    const toast = useToastStore()
    const notes = useNotesStore()
    if (!cardDraftsFor.value) {
      toast.error('没有可保存的草稿')
      return 0
    }
    const path = cardDraftsFor.value
    try {
      const note = await api.readNote(path)
      const blocks: string[] = []
      for (const c of accepted) {
        if (c.cardType === 'reverse') {
          blocks.push(`- Q: ${c.question}? #card-reverse\n  A: ${c.answer}`)
        } else if (c.cardType === 'cloze') {
          blocks.push(`- {{c1::${c.answer}::提示}} 问：${c.question}? #cloze`)
        } else {
          blocks.push(`- ${c.question}? #card\n  ${c.answer}`)
        }
      }
      const sep = '\n\n<!-- AI 生成的闪卡（可编辑 / 删除） -->\n\n'
      const newRaw = note.raw.replace(/\s*$/, '') + sep + blocks.join('\n\n') + '\n'
      await api.writeNote(path, newRaw)
      await api.reindexCards(path)
      await notes.refreshRecent()
      const inserted = accepted.length
      clearCardDrafts()
      toast.success(`已添加 ${inserted} 张闪卡到 ${path.split(/[\\/]/).pop()}`)
      return inserted
    } catch (e: any) {
      toast.error('保存失败：' + (e?.message ?? e))
      return 0
    }
  }

  async function loadModels() {
    lastError.value = null
    try {
      const list = await aiApi.listModels({
        provider: config.value.provider,
        base_url: config.value.base_url,
        api_key: config.value.api_key,
      })
      availableModels.value = list
      modelsLoaded.value = true
    } catch (e: any) {
      lastError.value = String(e?.message ?? e)
      availableModels.value = []
    }
  }

  async function send(text?: string) {
    const q = (text ?? inputText.value).trim()
    if (!q || pending.value) return
    if (!config.value.base_url) {
      lastError.value = '请先在 AI 设置中配置服务地址'
      showSettings.value = true
      return
    }
    inputText.value = ''
    const userMsg: ChatMessage = {
      id: 'm' + Date.now() + '-u',
      role: 'user',
      content: q,
      ts: Date.now(),
    }
    messages.value.push(userMsg)
    pending.value = true
    lastError.value = null
    const ctrl = new AbortController()
    inFlight.value = ctrl
    const assistant: ChatMessage = {
      id: 'm' + Date.now() + '-a',
      role: 'assistant',
      content: '',
      ts: Date.now(),
    }
    messages.value.push(assistant)
    try {
      const reply = await aiApi.chatStream(
        {
          provider: config.value.provider,
          model: config.value.model,
          base_url: config.value.base_url,
          api_key: config.value.api_key,
          system_prompt: config.value.system_prompt,
          question: q,
          top_k: config.value.top_k,
          temperature: config.value.temperature,
          max_tokens: config.value.max_tokens,
          scope_paths: config.value.scope_paths,
          history: messages.value
            .filter((m) => m.id !== assistant.id && m.id !== userMsg.id)
            .slice(-10)
            .map((m) => ({ role: m.role, content: m.content })),
        },
        (delta: string) => {
          assistant.content += delta
        },
        (sources: AiSource[]) => {
          assistant.sources = sources
        },
        ctrl.signal,
      )
      if (typeof reply === 'string') assistant.content = reply
    } catch (e: any) {
      if (e?.name !== 'AbortError') {
        assistant.error = String(e?.message ?? e)
        lastError.value = assistant.error
      }
    } finally {
      pending.value = false
      inFlight.value = null
    }
  }

  function abort() {
    if (inFlight.value) {
      inFlight.value.abort()
      inFlight.value = null
    }
    pending.value = false
  }

  const hasMessages = computed(() => messages.value.length > 0)

  return {
    config,
    messages,
    pending,
    showPanel,
    showSettings,
    inputText,
    availableModels,
    modelsLoaded,
    lastError,
    hasMessages,
    setConfig,
    saveConfig,
    openPanel,
    closePanel,
    toggleSettings,
    clearChat,
    loadModels,
    send,
    abort,
    generateCards,
    clearCardDrafts,
    commitCardDrafts,
    generatingCards,
    cardDrafts,
    cardDraftsFor,
    cardDraftsRaw,
  }
})

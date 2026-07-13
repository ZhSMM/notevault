// Tauri command wrappers - thin layer over invoke()

import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { BacklinkHit, Block, Card, CardStats, DanglingLink, GitLogEntry, GitStatus, GraphEdge, GraphNode, NoteContent, NoteMeta, ReviewResult, SearchHit, TreeNode, VaultInfo } from '../types'

// Publish (static site generation)
export interface PublishOptions {
  outputPath: string
  baseUrl?: string
}
export interface PublishResult {
  outputPath: string
  pages: number
  tags: number
  log: string
}

// AI card generation
export interface AiGenerateCardsRequest {
  provider: 'ollama' | 'openai'
  model: string
  baseUrl: string
  apiKey: string
  notePath: string
  count: number
  language: string
}
export interface GeneratedCard {
  question: string
  answer: string
  cardType: 'basic' | 'reverse' | 'cloze'
  source: string
}
export interface AiGenerateCardsResult {
  cards: GeneratedCard[]
  raw: string
}

export interface AiSource {
  path: string
  title: string
  snippet: string
  block_id?: string | null
  score: number
}

export interface AiChatRequest {
  provider: 'ollama' | 'openai'
  model: string
  base_url: string
  api_key: string
  system_prompt: string
  question: string
  top_k: number
  temperature: number
  max_tokens: number
  scope_paths: string[]
  history: { role: string; content: string }[]
}

export interface AiListModelsRequest {
  provider: 'ollama' | 'openai'
  base_url: string
  api_key: string
}

export const aiApi = {
  /** 列出可用模型（Ollama /api/tags，OpenAI /v1/models） */
  async listModels(req: AiListModelsRequest): Promise<string[]> {
    return invoke<string[]>('ai_list_models', { req })
  },

  /**
   * 单次非流式问答。返回完整 markdown 文本 + 引用来源。
   */
  async chat(req: AiChatRequest): Promise<{ content: string; sources: AiSource[] }> {
    return invoke<{ content: string; sources: AiSource[] }>('ai_chat', { req })
  },

  /**
   * 流式问答。Rust 端分两个 event 推过来：
   *   ai-chunk  -> { request_id, delta }  每段文本增量
   *   ai-sources -> { request_id, sources } 一次性推送检索源（流开始时）
   * 结束时抛 "ai-done" 事件；中途取消由 AbortSignal 触发。
   */
  async chatStream(
    req: AiChatRequest,
    onDelta: (delta: string) => void,
    onSources: (sources: AiSource[]) => void,
    signal?: AbortSignal,
  ): Promise<string> {
    const requestId = 'ai-' + Date.now() + '-' + Math.random().toString(36).slice(2, 8)
    const unlistens: UnlistenFn[] = []
    let buffer = ''
    let aborted = false
    try {
      const offChunk = await listen<{ request_id: string; delta: string }>('ai-chunk', (e) => {
        if (e.payload.request_id !== requestId) return
        buffer += e.payload.delta
        onDelta(e.payload.delta)
      })
      unlistens.push(offChunk)
      const offSources = await listen<{ request_id: string; sources: AiSource[] }>(
        'ai-sources',
        (e) => {
          if (e.payload.request_id !== requestId) return
          onSources(e.payload.sources)
        },
      )
      unlistens.push(offSources)
      const offDone = await listen<{ request_id: string; content: string }>('ai-done', (e) => {
        if (e.payload.request_id !== requestId) return
        buffer = e.payload.content
      })
      unlistens.push(offDone)
      const offErr = await listen<{ request_id: string; error: string }>('ai-error', (e) => {
        if (e.payload.request_id !== requestId) return
        throw new Error(e.payload.error)
      })
      unlistens.push(offErr)

      const onAbort = () => {
        aborted = true
        invoke('ai_cancel', { requestId }).catch(() => {})
      }
      if (signal) {
        if (signal.aborted) onAbort()
        else signal.addEventListener('abort', onAbort, { once: true })
      }

      await invoke<void>('ai_chat_stream', { req, requestId })
      if (aborted) throw Object.assign(new Error('cancelled'), { name: 'AbortError' })
      return buffer
    } finally {
      unlistens.forEach((u) => u())
    }
  },
}

export const api = {
  // Vault
  openVault: (path: string) => invoke<VaultInfo>('open_vault', { path }),
  closeVault: () => invoke<void>('close_vault'),
  getVaultInfo: () => invoke<VaultInfo | null>('get_vault_info'),
  pickVault: () => invoke<string | null>('pick_vault'),

  // Notes
  listNotes: (dir?: string) => invoke<NoteMeta[]>('list_notes', { dir: dir ?? null }),
  readNote: (path: string) => invoke<NoteContent>('read_note', { path }),
  writeNote: (path: string, content: string) =>
    invoke<NoteMeta>('write_note', { path, content }),
  createNote: (path: string, template?: string) =>
    invoke<NoteMeta>('create_note', { path, template: template ?? null }),
  createNoteSimple: (name: string) =>
    invoke<NoteMeta>('create_note_simple', { name }),
  reindexVault: () =>
    invoke<{ added: number; removed: number; total: number }>('reindex_vault'),
  deleteNote: (path: string) => invoke<void>('delete_note', { path }),
  renameNote: (oldPath: string, newPath: string) =>
    invoke<void>('rename_note', { oldPath, newPath }),
  getFileTree: (dir?: string) => invoke<TreeNode[]>('get_file_tree', { dir: dir ?? null }),

  // Search
  search: (query: string, limit?: number) =>
    invoke<SearchHit[]>('search', { query, limit: limit ?? 20 }),

  // Links
  getBacklinks: (notePath: string) =>
    invoke<BacklinkHit[]>('get_backlinks', { notePath }),
  getForwardLinks: (notePath: string) =>
    invoke<BacklinkHit[]>('get_forward_links', { notePath }),
  getDanglingLinks: () => invoke<DanglingLink[]>('get_dangling_links'),
  getBlock: (blockId: string) => invoke<Block | null>('get_block', { blockId }),
  getBlocks: (notePath: string) => invoke<Block[]>('get_blocks', { notePath }),
  reindexBlocks: () =>
    invoke<{ notes_indexed: number; blocks_total: number; links_total: number }>(
      'reindex_blocks',
    ),

  // Cards (FSRS)
  listDueCards: (limit?: number) =>
    invoke<Card[]>('list_due_cards', { limit: limit ?? 50 }),
  getCard: (id: string) => invoke<Card | null>('get_card', { id }),
  reviewCard: (id: string, rating: number) =>
    invoke<ReviewResult>('review_card', { id, rating }),
  countDueCards: () => invoke<number>('count_due_cards'),
  countTotalCards: () => invoke<number>('count_total_cards'),
  cardStats: () => invoke<CardStats>('card_stats'),
  reindexCards: (notePath?: string) =>
    invoke<{ notes_indexed: number; cards_total: number }>('reindex_cards', {
      notePath: notePath ?? null,
    }),

  // Git
  gitStatus: () => invoke<GitStatus>('git_status'),
  gitInit: () => invoke<GitStatus>('git_init'),
  gitIsRepo: () => invoke<boolean>('git_is_repo'),
  gitCommit: (message: string) => invoke<GitLogEntry>('git_commit', { message }),
  gitLog: (limit?: number) =>
    invoke<GitLogEntry[]>('git_log', { limit: limit ?? 20 }),

  // Graph
  getGraphData: () =>
    invoke<{ nodes: GraphNode[]; edges: GraphEdge[] }>('get_graph_data'),

  // Publish (static site)
  exportStatic: (vaultPath: string, options: PublishOptions) =>
    invoke<PublishResult>('export_static', { vaultPath, options }),

  // AI card generation
  aiGenerateCards: (req: AiGenerateCardsRequest) =>
    invoke<AiGenerateCardsResult>('ai_generate_cards', { req }),
}

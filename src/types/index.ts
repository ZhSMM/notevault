// Shared types mirroring the Rust backend DTOs

export interface VaultInfo {
  path: string
  name: string
}

export interface NoteMeta {
  path: string
  title: string
  tags: string[]
  modified: number
  size: number
}

export interface NoteContent {
  path: string
  raw: string
  body: string
  frontmatter: Record<string, unknown>
  links: string[]
  tags: string[]
  title: string
  modified: number
  size: number
}

export interface SearchHit {
  path: string
  title: string
  snippet: string
  rank: number
}

export interface TreeNode {
  name: string
  path: string
  is_dir: boolean
  children: TreeNode[]
}

export interface Block {
  id: string
  note_id: string
  type: 'heading' | 'paragraph' | 'code' | 'list' | 'blockquote'
  level: number | null
  content: string
  content_hash: string
  order_index: number
}

export interface BacklinkHit {
  source_path: string
  source_title: string
  from_block_id: string | null
  to_block_id: string | null
  context: string
  link_type: 'wiki' | 'block_ref' | 'transclusion'
}

export interface DanglingLink {
  from_note: string
  to_alias: string
  context: string
}

export interface Card {
  id: string
  note_path: string
  block_id: string | null
  card_type: 'basic' | 'reverse' | 'cloze'
  question: string
  answer: string
  cloze_text: string | null
  line_index: number
  difficulty: number
  stability: number
  interval_days: number
  reps: number
  lapses: number
  last_review_at: number | null
  next_review_at: number
  state: 'new' | 'learning' | 'review' | 'relearning'
  created_at: number
}

export interface ReviewResult {
  card: Card
  next_due_in_days: number
  correct: boolean
}

export interface CardStats {
  total: number
  due: number
  new_count: number
  learning: number
  review: number
  relearning: number
  total_reviews: number
  total_lapses: number
  avg_difficulty: number
  avg_stability: number
}

export interface GitStatus {
  is_repo: boolean
  head: string | null
  branch: string | null
  modified: string[]
  untracked: string[]
  staged: string[]
  ahead: number
  behind: number
  remote_url: string | null
}

export interface GitLogEntry {
  id: string
  full_id: string
  summary: string
  author: string
  email: string
  timestamp: number
  files_changed: number
}

export interface GraphNode {
  id: string
  label: string
  kind: string
  size: number
  in_degree: number
  out_degree: number
  tags: string[]
}

export interface GraphEdge {
  id: string
  source: string
  target: string
  kind: string
}

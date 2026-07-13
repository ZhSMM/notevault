// Frontmatter helpers - parse/serialize YAML for note frontmatter

import YAML from 'yaml'

export interface FrontmatterResult {
  raw: string
  body: string
  data: Record<string, unknown>
}

const FM_RE = /^---\r?\n([\s\S]*?)\r?\n---\r?\n?/

export function parseFrontmatter(text: string): FrontmatterResult {
  const m = text.match(FM_RE)
  if (!m) {
    return { raw: text, body: text, data: {} }
  }
  let data: Record<string, unknown> = {}
  try {
    const parsed = YAML.parse(m[1] ?? '')
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
      data = parsed as Record<string, unknown>
    }
  } catch (e) {
    console.warn('YAML parse failed:', e)
  }
  return {
    raw: text,
    body: text.slice(m[0].length),
    data,
  }
}

export function serializeFrontmatter(data: Record<string, unknown>, body: string): string {
  if (!data || Object.keys(data).length === 0) {
    return body
  }
  // Use block style for typical maps; YAML lib handles that automatically
  const yaml = YAML.stringify(data, { indent: 2, lineWidth: 0 }).trimEnd()
  return `---\n${yaml}\n---\n${body.startsWith('\n') ? body : '\n' + body}`
}

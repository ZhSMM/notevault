// Mermaid diagram rendering - async, lazy-init

import mermaid from 'mermaid'

let initialized = false
let counter = 0

export function initMermaid() {
  if (initialized) return
  mermaid.initialize({
    startOnLoad: false,
    theme: 'default',
    securityLevel: 'strict',
    fontFamily: 'inherit',
    themeVariables: {
      // Fine-tune to match Notevault palette
      primaryColor: '#6366f1',
      primaryTextColor: '#fff',
      primaryBorderColor: '#4f46e5',
      lineColor: '#525252',
      secondaryColor: '#f5f5f5',
      tertiaryColor: '#fafafa',
    },
  })
  initialized = true
}

/**
 * Render a mermaid diagram source into an SVG string.
 * @param source Mermaid source code
 * @returns SVG string
 */
export async function renderMermaid(source: string): Promise<string> {
  initMermaid()
  const id = `mmd-${Date.now()}-${counter++}`
  try {
    const { svg } = await mermaid.render(id, source.trim())
    return svg
  } catch (e) {
    // On parse error, return a styled error block so the user sees the syntax issue
    const msg = (e as Error)?.message ?? String(e)
    const escapedSource = source
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
    return `<pre class="mermaid-error" data-error="${msg.replace(/"/g, '&quot;')}"><code>${escapedSource}</code></pre>`
  }
}

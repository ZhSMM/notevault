<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { usePublishStore } from '../stores/publish'

const pub = usePublishStore()
const logExpanded = ref(false)

onMounted(() => {
  pub.openPanel()
})

function copyPath() {
  if (pub.lastResult) {
    navigator.clipboard.writeText(pub.lastResult.outputPath).catch(() => {})
  }
}
</script>

<template>
  <div
    v-if="pub.showPanel"
    class="fixed inset-0 z-40 flex bg-black/40 backdrop-blur-sm"
    @click.self="pub.closePanel()"
  >
    <div
      class="ml-auto h-full w-full max-w-[560px] flex flex-col bg-bg border-l border-border shadow-2xl"
    >
      <header class="h-12 px-4 flex items-center gap-2 border-b border-border bg-bg-soft">
        <span class="text-base font-medium">🌐 静态发布</span>
        <span class="text-xs text-fg-subtle">Quartz 风格 · GitHub Pages 友好</span>
        <div class="flex-1" />
        <button class="text-xs hover:text-fg" @click="pub.closePanel()">✕</button>
      </header>

      <div class="flex-1 overflow-y-auto p-4 space-y-4 text-sm">
        <p class="text-fg-subtle">
          把 vault 里的笔记导出成静态 HTML 网站，可以放到任何静态托管（GitHub Pages /
          Cloudflare Pages / Vercel / 本地预览）。
        </p>

        <div>
          <label class="block text-xs text-fg-subtle mb-1">输出目录</label>
          <input
            v-model="pub.outputPath"
            class="w-full bg-bg border border-border rounded px-2 py-1.5 font-mono text-xs"
            placeholder="例如：D:\publish\my-notes"
          />
          <p class="text-[11px] text-fg-subtle mt-1">
            建议填 vault 内 <code>.public</code> 或一个独立目录。已存在的内容会被清空。
          </p>
        </div>

        <div>
          <label class="block text-xs text-fg-subtle mb-1">站点 base URL</label>
          <input
            v-model="pub.baseUrl"
            class="w-full bg-bg border border-border rounded px-2 py-1.5 font-mono text-xs"
            placeholder="/"
          />
          <p class="text-[11px] text-fg-subtle mt-1">
            GitHub Pages 用户项目填 <code>/repo-name/</code>，根域或自定义域名填 <code>/</code>。
          </p>
        </div>

        <button
          class="btn btn-primary w-full"
          :disabled="pub.running"
          @click="pub.run()"
        >
          {{ pub.running ? '⏳ 生成中…' : '🚀 导出' }}
        </button>

        <div
          v-if="pub.lastError"
          class="p-3 rounded bg-red-500/10 text-red-500 text-xs whitespace-pre-wrap break-all"
        >
          <div class="font-medium mb-1">⚠️ 失败</div>
          {{ pub.lastError }}
        </div>

        <div
          v-if="pub.lastResult"
          class="p-3 rounded bg-accent/10 text-sm space-y-1"
        >
          <div class="font-medium text-accent">✓ 已生成</div>
          <div>页面：<strong>{{ pub.lastResult.pages }}</strong> 个</div>
          <div>标签页：<strong>{{ pub.lastResult.tags }}</strong> 个</div>
          <div class="font-mono text-xs break-all">{{ pub.lastResult.outputPath }}</div>
          <div class="flex flex-wrap gap-2 mt-2">
            <button
              class="btn btn-secondary text-xs h-7 px-2"
              @click="logExpanded = !logExpanded"
            >
              {{ logExpanded ? '隐藏日志' : '查看日志' }}
            </button>
            <button
              class="btn btn-secondary text-xs h-7 px-2"
              @click="copyPath"
              title="复制路径"
            >
              📋 复制路径
            </button>
          </div>
          <pre
            v-if="logExpanded"
            class="mt-2 p-2 bg-bg/50 rounded text-[11px] font-mono whitespace-pre-wrap break-all max-h-48 overflow-y-auto"
          >{{ pub.lastResult.log }}</pre>
        </div>

        <div class="text-[11px] text-fg-subtle space-y-1 pt-2 border-t border-border">
          <div><strong>输出包含：</strong></div>
          <ul class="list-disc pl-4 space-y-0.5">
            <li>每篇笔记一个 HTML 文件（保留目录结构）</li>
            <li>首页（按 mtime 排序）</li>
            <li>每个标签一个聚合页（<code>/tags/&lt;tag&gt;.html</code>）</li>
            <li>客户端搜索页（<code>/search.html</code>）</li>
            <li>RSS feed（<code>/rss.xml</code>）</li>
            <li>sitemap（<code>/sitemap.xml</code>）</li>
            <li>404 页</li>
          </ul>
          <div class="pt-2">
            <strong>本地预览：</strong><code>python -m http.server 8000</code> 然后访问 <code>http://localhost:8000</code>。
          </div>
          <div class="pt-1">
            <strong>发布到 GitHub Pages：</strong>把输出目录推到 <code>gh-pages</code> 分支即可（可以单独建一个 repo）。
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

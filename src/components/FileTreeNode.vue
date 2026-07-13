<script setup lang="ts">
import { ref } from 'vue'
import type { TreeNode } from '../types'
import { useNotesStore } from '../stores/notes'

const props = defineProps<{
  node: TreeNode
  depth: number
}>()

const expanded = ref(props.depth < 1) // top level open by default
const notes = useNotesStore()

async function open() {
  if (props.node.is_dir) {
    expanded.value = !expanded.value
  } else {
    try {
      await notes.openNote(props.node.path)
    } catch (e: any) {
      alert('打开失败: ' + (e?.message ?? e))
    }
  }
}
</script>

<template>
  <div>
    <button
      class="w-full text-left flex items-center gap-1 py-0.5 px-1 rounded hover:bg-bg-soft text-sm"
      :style="{ paddingLeft: depth * 12 + 4 + 'px' }"
      @click="open"
    >
      <span class="w-3 text-fg-subtle text-[10px]">
        <template v-if="node.is_dir">
          {{ expanded ? '▾' : '▸' }}
        </template>
      </span>
      <span class="truncate" :class="node.is_dir ? 'text-fg-muted' : 'text-fg'">
        {{ node.is_dir ? '📁' : '📄' }} {{ node.name }}
      </span>
    </button>
    <div v-if="node.is_dir && expanded">
      <FileTreeNode
        v-for="child in node.children"
        :key="child.path"
        :node="child"
        :depth="depth + 1"
      />
    </div>
  </div>
</template>

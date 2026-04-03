<script setup lang="ts">
defineProps<{
  activeTool: string
}>();

const emit = defineEmits<{
  (e: 'select', tool: string): void
}>();

const tools = [
  { id: 'attack', name: '攻击工具' },
  { id: 'filetransfer', name: '文件传输' }
];
</script>

<template>
  <aside class="sidebar">
    <div class="sidebar-header">
      <span class="logo">JiyuTool</span>
    </div>
    <nav class="sidebar-nav">
      <button
        v-for="tool in tools"
        :key="tool.id"
        :class="['nav-item', { active: activeTool === tool.id }]"
        @click="emit('select', tool.id)"
      >
        {{ tool.name }}
      </button>
    </nav>
  </aside>
</template>

<style scoped>
.sidebar {
  width: 200px;
  background: var(--bg-secondary);
  border-right: 1px solid var(--border-color);
  display: flex;
  flex-direction: column;
}

.sidebar-header {
  padding: 20px;
  border-bottom: 1px solid var(--border-color);
}

.logo {
  font-size: 18px;
  font-weight: 600;
  color: var(--accent-blue);
}

.sidebar-nav {
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.nav-item {
  background: transparent;
  border: none;
  border-radius: 0;
  padding: 12px 16px;
  text-align: left;
  color: var(--text-secondary);
  cursor: pointer;
  transition: all 0.2s;
}

.nav-item:hover {
  background: var(--bg-tertiary);
  color: var(--text-primary);
}

.nav-item.active {
  background: var(--accent-blue);
  color: white;
}
</style>

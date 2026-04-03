<script setup lang="ts">
import { ref } from 'vue';
import Sidebar from './components/Sidebar.vue';
import AttackPanel from './components/AttackPanel.vue';
import FileTransferPanel from './components/FileTransferPanel.vue';

const activeTool = ref('attack');
</script>

<template>
  <div class="app-layout">
    <Sidebar :active-tool="activeTool" @select="activeTool = $event" />
    <main class="main-content">
      <AttackPanel v-if="activeTool === 'attack'" />
      <FileTransferPanel v-if="activeTool === 'filetransfer'" />
    </main>
  </div>
</template>

<style scoped>
.app-layout {
  display: flex;
  height: 100%;
}

.main-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
</style>

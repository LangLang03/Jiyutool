<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';

const targetIp = ref('');
const allDevices = ref(false);
const localIpPrefix = ref('');
const port = ref(4705);
const loading = ref(false);
const status = ref('');
const showModal = ref(false);
const modalType = ref('');
const modalInput = ref('');
const showStatusModal = ref(false);

const actualIp = computed(() => {
  if (allDevices.value && localIpPrefix.value) {
    return localIpPrefix.value;
  }
  return targetIp.value;
});

interface ActionItem {
  id: string;
  title: string;
  description: string;
  type: 'default' | 'danger' | 'success';
  needInput?: boolean;
}

const remoteActions: ActionItem[] = [
  {
    id: 'message',
    title: '发送消息',
    description: '向目标设备发送自定义消息',
    type: 'default',
    needInput: true
  },
  {
    id: 'command',
    title: '执行命令',
    description: '在目标设备上执行系统命令',
    type: 'default',
    needInput: true
  },
  {
    id: 'reboot',
    title: '重启',
    description: '远程重启目标设备',
    type: 'danger'
  },
  {
    id: 'shutdown',
    title: '关机',
    description: '远程关闭目标设备',
    type: 'danger'
  }
];

const localActions: ActionItem[] = [
  {
    id: 'reverse_shell',
    title: '反弹Shell',
    description: '启动PowerShell监听，获取目标设备Shell',
    type: 'success'
  },
  {
    id: 'kill_student',
    title: '本地解控',
    description: '终止StudentMain.exe或Student.exe进程',
    type: 'danger'
  }
];

async function getLocalInfo() {
  try {
    const result = await invoke<{ success: boolean; data: { ip: string; ports: number[] } }>('get_local_info');
    if (result.success && result.data?.ip) {
      const parts = result.data.ip.split('.');
      localIpPrefix.value = parts.slice(0, 3).join('.');
      if (result.data.ports?.length > 0) {
        port.value = result.data.ports[0];
      }
    }
  } catch (e) {
    console.error('Failed to get local info:', e);
  }
}

onMounted(() => {
  getLocalInfo();
});

function setStatus(msg: string) {
  status.value = msg;
  showStatusModal.value = true;
}

function closeStatusModal() {
  showStatusModal.value = false;
}

async function executeAction(action: string, params: Record<string, unknown> = {}) {
  if (!actualIp.value && !allDevices.value) {
    setStatus('请输入目标IP');
    return;
  }

  loading.value = true;
  setStatus(`正在执行 ${action}...`);

  try {
    const result = await invoke<{ success: boolean; message: string }>(action, {
      ip: actualIp.value,
      port: port.value,
      ...params
    });
    setStatus(result.message);
  } catch (e) {
    setStatus(`错误: ${e}`);
  } finally {
    loading.value = false;
  }
}

function openModal(type: string) {
  modalType.value = type;
  modalInput.value = '';
  showModal.value = true;
}

function closeModal() {
  showModal.value = false;
  modalType.value = '';
  modalInput.value = '';
}

async function confirmModal() {
  if (!modalInput.value.trim()) {
    closeModal();
    return;
  }

  if (modalType.value === 'message') {
    await executeAction('send_message', { message: modalInput.value });
  } else if (modalType.value === 'command') {
    await executeAction('execute_command', { command: modalInput.value });
  }

  closeModal();
}

async function handleAction(action: ActionItem) {
  if (action.needInput) {
    openModal(action.id);
    return;
  }

  switch (action.id) {
    case 'reboot':
      await executeAction('reboot_target');
      break;
    case 'shutdown':
      await executeAction('shutdown_target');
      break;
    case 'reverse_shell':
      await executeReverseShell();
      break;
    case 'kill_student':
      await executeKillStudent();
      break;
  }
}

async function executeReverseShell() {
  loading.value = true;
  setStatus('正在启动PowerShell监听...');

  try {
    const result = await invoke<{ success: boolean; message: string }>('start_reverse_shell', {
      ip: actualIp.value,
      port: port.value
    });
    setStatus(result.message);
  } catch (e) {
    setStatus(`错误: ${e}`);
  } finally {
    loading.value = false;
  }
}

async function executeKillStudent() {
  loading.value = true;
  setStatus('正在终止进程...');

  try {
    const result = await invoke<{ success: boolean; message: string }>('kill_student_process');
    setStatus(result.message);
  } catch (e) {
    setStatus(`错误: ${e}`);
  } finally {
    loading.value = false;
  }
}

function getButtonClass(type: string): string {
  return type;
}
</script>

<template>
  <div class="attack-panel">
    <div class="panel-header">
      <div class="ip-section">
        <input
          v-model="targetIp"
          type="text"
          placeholder="目标IP (例如: 192.168.1.100)"
          :disabled="allDevices"
          class="ip-input"
        />
        <label class="checkbox-wrapper">
          <input type="checkbox" v-model="allDevices" />
          <span>全部设备 ({{ localIpPrefix || '...' }}.1-255)</span>
        </label>
      </div>
      <div class="port-section">
        <label>端口:</label>
        <input v-model.number="port" type="number" class="port-input" />
      </div>
    </div>

    <div class="panel-content">
      <div class="section">
        <h3 class="section-title">远程操作</h3>
        <div class="action-list">
          <div v-for="action in remoteActions" :key="action.id" class="action-item">
            <div class="action-info">
              <div class="action-title">{{ action.title }}</div>
              <div class="action-desc">{{ action.description }}</div>
            </div>
            <button
              :class="['action-btn', getButtonClass(action.type)]"
              @click="handleAction(action)"
              :disabled="loading"
            >
              执行
            </button>
          </div>
        </div>
      </div>

      <div class="section">
        <h3 class="section-title">本地操作</h3>
        <div class="action-list">
          <div v-for="action in localActions" :key="action.id" class="action-item">
            <div class="action-info">
              <div class="action-title">{{ action.title }}</div>
              <div class="action-desc">{{ action.description }}</div>
            </div>
            <button
              :class="['action-btn', getButtonClass(action.type)]"
              @click="handleAction(action)"
              :disabled="loading || (action.id === 'reverse_shell' && allDevices)"
            >
              执行
            </button>
          </div>
        </div>
      </div>
    </div>

    <div v-if="showModal" class="modal-overlay" @click.self="closeModal">
      <div class="modal">
        <h3>{{ modalType === 'message' ? '发送消息' : '执行命令' }}</h3>
        <textarea
          v-model="modalInput"
          :placeholder="modalType === 'message' ? '请输入消息内容...' : '请输入命令...'"
        ></textarea>
        <div class="modal-actions">
          <button @click="closeModal">取消</button>
          <button class="success" @click="confirmModal">确认</button>
        </div>
      </div>
    </div>

    <div v-if="showStatusModal" class="modal-overlay" @click.self="closeStatusModal">
      <div class="modal">
        <h3>执行结果</h3>
        <div class="status-content">{{ status }}</div>
        <div class="modal-actions">
          <button class="success" @click="closeStatusModal">确定</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.attack-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.panel-header {
  background: var(--bg-secondary);
  border-bottom: 1px solid var(--border-color);
  padding: 20px;
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 20px;
}

.ip-section {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.ip-input {
  width: 100%;
  max-width: 320px;
}

.port-section {
  display: flex;
  align-items: center;
  gap: 10px;
}

.port-input {
  width: 100px;
}

.panel-content {
  flex: 1;
  padding: 20px;
  overflow-y: auto;
}

.section {
  margin-bottom: 24px;
}

.section-title {
  font-size: 12px;
  font-weight: 500;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 1px;
  margin-bottom: 12px;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--border-color);
}

.action-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.action-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  padding: 16px;
  transition: border-color 0.2s;
}

.action-item:hover {
  border-color: var(--accent-blue);
}

.action-info {
  flex: 1;
}

.action-title {
  font-size: 14px;
  font-weight: 500;
  color: var(--text-primary);
  margin-bottom: 4px;
}

.action-desc {
  font-size: 12px;
  color: var(--text-secondary);
}

.action-btn {
  min-width: 80px;
  padding: 8px 20px;
}

.action-btn.success {
  background: var(--accent-cyan);
  border-color: var(--accent-cyan);
  color: #1a1d23;
}

.action-btn.success:hover {
  background: #00c49a;
  border-color: #00c49a;
}

.action-btn.danger {
  background: var(--danger);
  border-color: var(--danger);
}

.action-btn.danger:hover {
  background: #ff5252;
  border-color: #ff5252;
}

.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal {
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  padding: 24px;
  min-width: 400px;
  max-width: 500px;
}

.modal h3 {
  margin-bottom: 16px;
  font-weight: 500;
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
  margin-top: 20px;
}

.modal textarea {
  width: 100%;
  min-height: 100px;
  resize: vertical;
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  padding: 12px;
  color: var(--text-primary);
  outline: none;
}

.modal textarea:focus {
  border-color: var(--accent-blue);
}

.status-content {
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  padding: 16px;
  font-family: 'Consolas', 'Monaco', monospace;
  font-size: 13px;
  white-space: pre-wrap;
  word-break: break-all;
  max-height: 300px;
  overflow-y: auto;
}
</style>

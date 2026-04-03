<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';

interface DeviceInfo {
  uuid: string;
  device_name: string;
  ip: string;
  port: number;
  last_seen: number;
}

interface TransferProgress {
  transfer_id: string;
  file_name: string;
  total_size: number;
  transferred_size: number;
  current_file: string;
  current_file_index: number;
  total_files: number;
  speed: number;
  status: string;
}

interface IncomingFileRequest {
  transfer_id: string;
  from_device: DeviceInfo;
  file_name: string;
  file_size: number;
  md5: string;
  is_folder: boolean;
  total_files: number;
}

const devices = ref<DeviceInfo[]>([]);
const selectedDevice = ref<string>('');
const serviceInfo = ref<{ uuid: string; device_name: string; ip: string; port: number } | null>(null);
const isScanning = ref(false);
const isTransferring = ref(false);
const transferProgress = ref<TransferProgress | null>(null);
const incomingRequest = ref<IncomingFileRequest | null>(null);
const savePath = ref('');
const status = ref('');
const manualIp = ref('');
const manualPort = ref(14706);
const isConnecting = ref(false);
const showResultModal = ref(false);
const resultMessage = ref('');

const formatSize = (bytes: number): string => {
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(2) + ' KB';
  if (bytes < 1024 * 1024 * 1024) return (bytes / 1024 / 1024).toFixed(2) + ' MB';
  return (bytes / 1024 / 1024 / 1024).toFixed(2) + ' GB';
};

const progressPercent = computed(() => {
  if (!transferProgress.value) return 0;
  return Math.round((transferProgress.value.transferred_size / transferProgress.value.total_size) * 100);
});

function showResult(msg: string) {
  resultMessage.value = msg;
  showResultModal.value = true;
}

function closeResultModal() {
  showResultModal.value = false;
}

async function initService() {
  try {
    const result = await invoke<{ success: boolean; data: { uuid: string; device_name: string; ip: string; port: number } }>('init_file_transfer');
    if (result.success && result.data) {
      serviceInfo.value = result.data;
      setStatus(`服务已启动`);
    }
  } catch (e) {
    setStatus(`启动服务失败: ${e}`);
  }
}

async function scanDevices() {
  isScanning.value = true;
  setStatus('正在扫描设备...');
  
  try {
    const result = await invoke<{ success: boolean; data: { devices: DeviceInfo[] } }>('scan_devices');
    if (result.success && result.data) {
      devices.value = result.data.devices;
      setStatus(`发现 ${devices.value.length} 个设备`);
    }
  } catch (e) {
    setStatus(`扫描失败: ${e}`);
  } finally {
    isScanning.value = false;
  }
}

async function connectToDevice() {
  if (!manualIp.value.trim()) {
    showResult('请输入目标IP');
    return;
  }

  isConnecting.value = true;
  setStatus(`正在连接 ${manualIp.value}:${manualPort.value}...`);

  try {
    const result = await invoke<{ success: boolean; data: { device: DeviceInfo } }>('connect_device', {
      ip: manualIp.value,
      port: manualPort.value
    });
    
    if (result.success && result.data) {
      const device = result.data.device;
      const exists = devices.value.find(d => d.uuid === device.uuid);
      if (!exists) {
        devices.value.push(device);
      }
      selectedDevice.value = device.uuid;
      setStatus(`已连接到 ${device.device_name}`);
    }
  } catch (e) {
    showResult(`连接失败: ${e}`);
  } finally {
    isConnecting.value = false;
  }
}

async function sendFile() {
  if (!selectedDevice.value) {
    showResult('请选择目标设备');
    return;
  }

  try {
    const filePath = await open({
      multiple: false,
      title: '选择要发送的文件'
    });
    
    if (!filePath) return;
    
    isTransferring.value = true;
    setStatus('正在发送文件...');

    const result = await invoke<{ success: boolean; message: string }>('send_file', {
      targetUuid: selectedDevice.value,
      filePath: filePath
    });
    
    if (result.success) {
      setStatus('文件发送中...');
    } else {
    showResult(`发送失败: ${result.message}`);
    isTransferring.value = false;
  }
  } catch (e) {
    showResult(`发送失败: ${e}`);
    isTransferring.value = false;
  }
}

async function sendFolder() {
  if (!selectedDevice.value) {
    showResult('请选择目标设备');
    return;
  }

  try {
    const folderPath = await open({
      directory: true,
      title: '选择要发送的文件夹'
    });
    
    if (!folderPath) return;

    isTransferring.value = true;
    setStatus('正在发送文件夹...');

    const result = await invoke<{ success: boolean; message: string }>('send_folder', {
      targetUuid: selectedDevice.value,
      folderPath: folderPath
    });
    
    if (result.success) {
      setStatus('文件夹发送中...');
    } else {
    showResult(`发送失败: ${result.message}`);
    isTransferring.value = false;
  }
  } catch (e) {
    showResult(`发送失败: ${e}`);
    isTransferring.value = false;
  }
}

async function acceptIncoming() {
  if (!incomingRequest.value) return;
  
  const path = savePath.value || '.';
  if (!path) return;

  try {
    await invoke('accept_file', {
      transferId: incomingRequest.value.transfer_id,
      savePath: path
    });
    incomingRequest.value = null;
    savePath.value = '';
    setStatus('已接受文件，开始接收...');
  } catch (e) {
    showResult(`接受失败: ${e}`);
  }
}

async function rejectIncoming() {
  if (!incomingRequest.value) return;

  try {
    await invoke('reject_file', {
      transferId: incomingRequest.value.transfer_id,
      reason: '用户拒绝'
    });
    incomingRequest.value = null;
    setStatus('已拒绝文件');
  } catch (e) {
    showResult(`拒绝失败: ${e}`);
  }
}

function setStatus(msg: string) {
  status.value = msg;
}

onMounted(async () => {
  await initService();

  await listen<TransferProgress>('transfer-progress', (event) => {
    transferProgress.value = event.payload;
    if (event.payload.status === 'Completed') {
      isTransferring.value = false;
      setStatus('传输完成');
    }
  });

  await listen<IncomingFileRequest>('file-request', (event) => {
    incomingRequest.value = event.payload;
  });

  await listen<{ transfer_id: string; success: boolean }>('transfer-complete', (event) => {
    isTransferring.value = false;
    setStatus('传输完成');
  });

  await listen<{ error: string }>('transfer-error', (event) => {
    isTransferring.value = false;
    showResult(`传输失败: ${event.payload.error}`);
  });
});
</script>

<template>
  <div class="file-transfer-panel">
    <div class="panel-header">
      <div class="info-row" v-if="serviceInfo">
        <div class="info-item">
          <span class="label">UUID</span>
          <span class="value">{{ serviceInfo.uuid }}</span>
        </div>
        <div class="info-item">
          <span class="label">设备</span>
          <span class="value">{{ serviceInfo.device_name }}</span>
        </div>
        <div class="info-item">
          <span class="label">地址</span>
          <span class="value">{{ serviceInfo.ip }}:{{ serviceInfo.port }}</span>
        </div>
      </div>
      <div class="connect-row">
        <input 
          v-model="manualIp" 
          type="text" 
          class="ip-input" 
          placeholder="目标IP"
        />
        <input 
          v-model.number="manualPort" 
          type="number" 
          class="port-input" 
          placeholder="端口"
        />
        <button class="btn-connect" @click="connectToDevice" :disabled="isConnecting">
          {{ isConnecting ? '连接中...' : '连接' }}
        </button>
        <button class="btn-refresh" @click="scanDevices" :disabled="isScanning">
          {{ isScanning ? '扫描中...' : '扫描' }}
        </button>
      </div>
    </div>

    <div class="panel-content">
      <div class="device-list" v-if="devices.length > 0">
        <div 
          v-for="device in devices" 
          :key="device.uuid"
          :class="['device-item', { selected: selectedDevice === device.uuid }]"
          @click="selectedDevice = device.uuid"
        >
          <div class="device-main">
            <div class="device-name">{{ device.device_name }}</div>
            <div class="device-ip">{{ device.ip }}:{{ device.port }}</div>
          </div>
          <div class="device-uuid">{{ device.uuid.substring(0, 8) }}</div>
        </div>
      </div>
      <div class="empty-state" v-else>
        无设备
      </div>

      <div class="section" v-if="selectedDevice">
        <h3 class="section-title">发送文件</h3>
        <div class="action-list">
          <button class="btn-action" @click="sendFile" :disabled="isTransferring">
            发送文件
          </button>
          <button class="btn-action" @click="sendFolder" :disabled="isTransferring">
            发送文件夹
          </button>
        </div>
      </div>

      <div class="section" v-if="transferProgress">
        <h3 class="section-title">传输进度</h3>
        <div class="progress-info">
          <div class="progress-name">{{ transferProgress.file_name }}</div>
          <div class="progress-stats">
            {{ formatSize(transferProgress.transferred_size) }} / {{ formatSize(transferProgress.total_size) }} ({{ progressPercent }}%)
          </div>
          <div class="progress-bar">
            <div class="progress-fill" :style="{ width: progressPercent + '%' }"></div>
          </div>
        </div>
      </div>
    </div>

    <div class="modal-overlay" v-if="incomingRequest" @click.self="incomingRequest = null">
      <div class="modal">
        <h3>接收文件请求</h3>
        <div class="request-info">
          <div class="request-row">
            <span class="label">来自:</span>
            <span class="value">{{ incomingRequest.from_device.device_name }}</span>
          </div>
          <div class="request-row">
            <span class="label">文件:</span>
            <span class="value">{{ incomingRequest.file_name }}</span>
          </div>
          <div class="request-row">
            <span class="label">大小:</span>
            <span class="value">{{ formatSize(incomingRequest.file_size) }}</span>
          </div>
        </div>
        <div class="save-input">
          <label>保存路径:</label>
          <input v-model="savePath" type="text" placeholder="默认当前目录" />
        </div>
        <div class="modal-actions">
          <button class="btn-secondary" @click="rejectIncoming">拒绝</button>
          <button class="btn-primary" @click="acceptIncoming">接受</button>
        </div>
      </div>
    </div>

    <div class="modal-overlay" v-if="showResultModal" @click.self="closeResultModal">
      <div class="modal">
        <h3>执行结果</h3>
        <div class="status-content">{{ resultMessage }}</div>
        <div class="modal-actions">
          <button class="btn-primary" @click="closeResultModal">确定</button>
        </div>
      </div>
    </div>

    <div class="status-bar">{{ status }}</div>
  </div>
</template>

<style scoped>
.file-transfer-panel {
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

.info-row {
  display: flex;
  gap: 24px;
}

.info-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.info-item .label {
  font-size: 11px;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.info-item .value {
  font-size: 13px;
  color: var(--text-primary);
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
}

.btn-refresh {
  padding: 6px 12px;
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  color: var(--text-primary);
  font-size: 12px;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-refresh:hover:not(:disabled) {
  border-color: var(--accent-blue);
}

.btn-refresh:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.connect-row {
  display: flex;
  gap: 8px;
  align-items: center;
}

.ip-input {
  width: 140px;
  padding: 6px 10px;
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  color: var(--text-primary);
  font-size: 12px;
}

.ip-input:focus {
  outline: none;
  border-color: var(--accent-blue);
}

.port-input {
  width: 70px;
  padding: 6px 10px;
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  color: var(--text-primary);
  font-size: 12px;
}

.port-input:focus {
  outline: none;
  border-color: var(--accent-blue);
}

.btn-connect {
  padding: 6px 12px;
  background: var(--accent-blue);
  border: 1px solid var(--accent-blue);
  color: white;
  font-size: 12px;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-connect:hover:not(:disabled) {
  background: #0062cc;
}

.btn-connect:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.device-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 24px;
}

.device-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  padding: 12px 16px;
  cursor: pointer;
  transition: border-color 0.2s;
}

.device-item:hover {
  border-color: var(--accent-blue);
}

.device-item.selected {
  border-color: var(--accent-blue);
  background: rgba(0, 122, 255, 0.1);
}

.device-name {
  font-size: 14px;
  font-weight: 500;
  color: var(--text-primary);
}

.device-ip {
  font-size: 12px;
  color: var(--text-secondary);
  margin-top: 2px;
}

.device-uuid {
  font-family: monospace;
  font-size: 11px;
  color: var(--text-secondary);
}

.empty-state {
  text-align: center;
  padding: 60px 20px;
  color: var(--text-secondary);
  font-size: 14px;
  margin-bottom: 24px;
}

.action-list {
  display: flex;
  gap: 12px;
}

.btn-action {
  flex: 1;
  padding: 12px 20px;
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  color: var(--text-primary);
  font-size: 13px;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-action:hover:not(:disabled) {
  border-color: var(--accent-blue);
}

.btn-action:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-primary {
  padding: 10px 20px;
  background: var(--accent-blue);
  border: 1px solid var(--accent-blue);
  color: white;
  font-size: 13px;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-primary:hover:not(:disabled) {
  background: #0062cc;
}

.btn-primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-secondary {
  padding: 10px 20px;
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  color: var(--text-primary);
  font-size: 13px;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-secondary:hover {
  border-color: var(--accent-blue);
}

.progress-info {
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  padding: 16px;
}

.progress-name {
  font-size: 14px;
  color: var(--text-primary);
  margin-bottom: 8px;
}

.progress-stats {
  font-size: 12px;
  color: var(--text-secondary);
  margin-bottom: 12px;
}

.progress-bar {
  height: 6px;
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background: var(--accent-blue);
  transition: width 0.3s;
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
  font-size: 16px;
  font-weight: 500;
}

.request-info {
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  padding: 12px;
  margin-bottom: 16px;
}

.request-row {
  display: flex;
  gap: 8px;
  margin: 4px 0;
  font-size: 13px;
}

.request-row .label {
  color: var(--text-secondary);
}

.request-row .value {
  color: var(--text-primary);
}

.save-input {
  margin-bottom: 16px;
}

.save-input label {
  display: block;
  font-size: 12px;
  color: var(--text-secondary);
  margin-bottom: 6px;
}

.save-input input {
  width: 100%;
  padding: 8px 12px;
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  color: var(--text-primary);
  font-size: 13px;
}

.save-input input:focus {
  outline: none;
  border-color: var(--accent-blue);
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
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

.status-bar {
  background: var(--bg-secondary);
  border-top: 1px solid var(--border-color);
  padding: 12px 20px;
  font-size: 12px;
  color: var(--text-secondary);
}
</style>

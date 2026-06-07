<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

type SyncthingFolder = {
  id: string
  label?: string
  path?: string
  paused?: boolean
  devices?: Array<{ deviceID: string }>
}

type SyncthingDevice = {
  deviceID: string
  name?: string
  addresses?: string[]
  paused?: boolean
}

type SyncthingConfig = {
  folders?: SyncthingFolder[]
  devices?: SyncthingDevice[]
}

type SyncthingSystemStatus = {
  myID?: string
  discoveryEnabled?: boolean
  startTime?: string
  uptime?: number
}

type SyncthingConnection = {
  connected?: boolean
  address?: string
  type?: string
}

type SyncthingConnections = {
  total?: {
    inBytesTotal?: number
    outBytesTotal?: number
  }
  connections?: Record<string, SyncthingConnection>
}

type SyncthingOverview = {
  running: boolean
  ready: boolean
  config: SyncthingConfig
  systemStatus: SyncthingSystemStatus
  connections: SyncthingConnections
  restartRequired: boolean
  error?: string | null
}

type OperationResult = {
  restartRequired: boolean
}

type ModalName = 'folder' | 'device' | null
type NavKey = 'tasks' | 'devices' | 'transfers' | 'settings'

type ConfirmAction = {
  title: string
  message: string
  danger?: boolean
  action: () => Promise<void>
} | null

const isRunning = ref(false)
const isReady = ref(false)
const isLoading = ref(false)
const isMutating = ref(false)
const restartRequired = ref(false)
const lastUpdated = ref('尚未刷新')
const errorMessage = ref('')
const successMessage = ref('')
const activeView = ref<NavKey>('tasks')
const activeModal = ref<ModalName>(null)
const confirmAction = ref<ConfirmAction>(null)
const config = ref<SyncthingConfig>({ folders: [], devices: [] })
const systemStatus = ref<SyncthingSystemStatus>({})
const connections = ref<SyncthingConnections>({ connections: {} })

const folderForm = reactive({
  id: '',
  label: '',
  path: '',
  deviceIds: [] as string[],
})

const deviceForm = reactive({
  deviceId: '',
  name: '',
  addresses: 'dynamic',
  folderIds: [] as string[],
})

const folders = computed(() => config.value.folders ?? [])
const devices = computed(() => config.value.devices ?? [])
const localDeviceId = computed(() => systemStatus.value.myID || '')
const remoteDevices = computed(() => devices.value.filter((device) => device.deviceID !== localDeviceId.value))
const connectedDevices = computed(() => {
  return remoteDevices.value.filter((device) => connections.value.connections?.[device.deviceID]?.connected).length
})
const totalTraffic = computed(() => {
  const total = connections.value.total
  return `${formatBytes(total?.inBytesTotal ?? 0)} ↓ / ${formatBytes(total?.outBytesTotal ?? 0)} ↑`
})
const uptimeText = computed(() => formatDuration(systemStatus.value.uptime ?? 0))
const statusText = computed(() => {
  if (!isRunning.value) return '未连接'
  return isReady.value ? '运行中' : '启动中'
})
const pageTitle = computed(() => {
  const titles: Record<NavKey, string> = {
    tasks: '同步任务',
    devices: '设备管理',
    transfers: '传输记录',
    settings: '设置',
  }
  return titles[activeView.value]
})
const pageDescription = computed(() => {
  const descriptions: Record<NavKey, string> = {
    tasks: '管理本机同步文件夹和共享设备',
    devices: '管理远程设备连接、暂停和删除',
    transfers: '查看当前连接和传输状态',
    settings: '控制 Syncthing 核心和全局操作',
  }
  return descriptions[activeView.value]
})
const connectionEntries = computed(() => {
  return Object.entries(connections.value.connections ?? {}).map(([deviceId, connection]) => ({
    deviceId,
    ...connection,
  }))
})

async function refreshData() {
  isLoading.value = true
  errorMessage.value = ''

  try {
    const overview = await invoke<SyncthingOverview>('get_syncthing_overview')
    isRunning.value = overview.running
    isReady.value = overview.ready
    config.value = overview.config ?? { folders: [], devices: [] }
    systemStatus.value = overview.systemStatus ?? {}
    connections.value = overview.connections ?? { connections: {} }
    restartRequired.value = overview.restartRequired
    lastUpdated.value = new Date().toLocaleTimeString()

    if (overview.error) {
      errorMessage.value = overview.error
    }
  } catch (error) {
    errorMessage.value = String(error)
  } finally {
    isLoading.value = false
  }
}

async function openWebUi() {
  await runAction('打开 Web UI', async () => {
    await invoke('open_syncthing_web')
  }, false)
}

function openFolderModal() {
  resetFolderForm()
  activeModal.value = 'folder'
}

function openDeviceModal() {
  resetDeviceForm()
  activeModal.value = 'device'
}

function closeModal() {
  if (isMutating.value) return
  activeModal.value = null
}

async function submitFolder() {
  if (!folderForm.id.trim()) {
    errorMessage.value = '文件夹 ID 不能为空'
    return
  }
  if (!folderForm.path.trim()) {
    errorMessage.value = '本地路径不能为空'
    return
  }

  await runAction('添加文件夹', async () => {
    const result = await invoke<OperationResult>('add_syncthing_folder', {
      request: {
        id: folderForm.id.trim(),
        label: folderForm.label.trim(),
        path: folderForm.path.trim(),
        deviceIds: folderForm.deviceIds,
      },
    })
    handleOperationResult(result)
    activeModal.value = null
    await refreshData()
  })
}

async function submitDevice() {
  if (!deviceForm.deviceId.trim()) {
    errorMessage.value = '设备 ID 不能为空'
    return
  }

  await runAction('添加设备', async () => {
    const result = await invoke<OperationResult>('add_syncthing_device', {
      request: {
        deviceId: deviceForm.deviceId.trim(),
        name: deviceForm.name.trim(),
        addresses: parseAddresses(deviceForm.addresses),
        folderIds: deviceForm.folderIds,
      },
    })
    handleOperationResult(result)
    activeModal.value = null
    await refreshData()
  })
}

async function pauseFolder(folder: SyncthingFolder) {
  await runAction('暂停文件夹', async () => {
    handleOperationResult(await invoke<OperationResult>('pause_syncthing_folder', { folderId: folder.id }))
    await refreshData()
  })
}

async function resumeFolder(folder: SyncthingFolder) {
  await runAction('恢复文件夹', async () => {
    handleOperationResult(await invoke<OperationResult>('resume_syncthing_folder', { folderId: folder.id }))
    await refreshData()
  })
}

async function rescanFolder(folder: SyncthingFolder) {
  await runAction('扫描文件夹', async () => {
    handleOperationResult(await invoke<OperationResult>('rescan_syncthing_folder', { folderId: folder.id }))
    await refreshData()
  })
}

function confirmRemoveFolder(folder: SyncthingFolder) {
  confirmAction.value = {
    title: '删除同步文件夹',
    message: `确定要从 Syncthing 配置中删除「${folder.label || folder.id}」吗？本操作不会删除磁盘上的实际文件。`,
    danger: true,
    action: async () => {
      await runAction('删除文件夹', async () => {
        handleOperationResult(await invoke<OperationResult>('remove_syncthing_folder', { folderId: folder.id }))
        await refreshData()
      })
    },
  }
}

async function pauseDevice(device: SyncthingDevice) {
  await runAction('暂停设备', async () => {
    handleOperationResult(await invoke<OperationResult>('pause_syncthing_device', { deviceId: device.deviceID }))
    await refreshData()
  })
}

async function resumeDevice(device: SyncthingDevice) {
  await runAction('恢复设备', async () => {
    handleOperationResult(await invoke<OperationResult>('resume_syncthing_device', { deviceId: device.deviceID }))
    await refreshData()
  })
}

function confirmRemoveDevice(device: SyncthingDevice) {
  confirmAction.value = {
    title: '删除远程设备',
    message: `确定要删除设备「${device.name || shortId(device.deviceID)}」吗？它也会从所有共享文件夹里移除。`,
    danger: true,
    action: async () => {
      await runAction('删除设备', async () => {
        handleOperationResult(await invoke<OperationResult>('remove_syncthing_device', { deviceId: device.deviceID }))
        await refreshData()
      })
    },
  }
}

async function startCore() {
  await runAction('启动核心', async () => {
    await invoke('start_syncthing')
    await refreshData()
  })
}

async function shutdownCore() {
  await runAction('停止核心', async () => {
    await invoke('shutdown_syncthing')
    await refreshData()
  })
}

async function restartCore() {
  await runAction('重启核心', async () => {
    await invoke('restart_syncthing')
    await refreshData()
  })
}

async function rescanAllFolders() {
  await runAction('扫描全部文件夹', async () => {
    handleOperationResult(await invoke<OperationResult>('rescan_all_syncthing_folders'))
    await refreshData()
  })
}

async function pauseAllDevices() {
  await runAction('暂停全部设备', async () => {
    handleOperationResult(await invoke<OperationResult>('pause_all_syncthing_devices'))
    await refreshData()
  })
}

async function resumeAllDevices() {
  await runAction('恢复全部设备', async () => {
    handleOperationResult(await invoke<OperationResult>('resume_all_syncthing_devices'))
    await refreshData()
  })
}

async function runConfirmedAction() {
  const current = confirmAction.value
  if (!current) return
  await current.action()
  confirmAction.value = null
}

async function runAction(name: string, action: () => Promise<void>, showSuccess = true) {
  isMutating.value = true
  errorMessage.value = ''
  successMessage.value = ''

  try {
    await action()
    if (showSuccess) {
      successMessage.value = `${name}完成`
    }
  } catch (error) {
    errorMessage.value = String(error)
  } finally {
    isMutating.value = false
  }
}

function handleOperationResult(result?: OperationResult) {
  if (result?.restartRequired) {
    restartRequired.value = true
    successMessage.value = '操作已完成，Syncthing 提示需要重启核心。'
  }
}

function resetFolderForm() {
  folderForm.id = ''
  folderForm.label = ''
  folderForm.path = ''
  folderForm.deviceIds = remoteDevices.value.map((device) => device.deviceID)
}

function resetDeviceForm() {
  deviceForm.deviceId = ''
  deviceForm.name = ''
  deviceForm.addresses = 'dynamic'
  deviceForm.folderIds = folders.value.map((folder) => folder.id)
}

function parseAddresses(value: string) {
  const addresses = value
    .split(/[\n,]/)
    .map((address) => address.trim())
    .filter(Boolean)

  return addresses.length ? addresses : ['dynamic']
}

async function startWindowDrag(event: MouseEvent) {
  if (event.buttons === 1) {
    await invoke('start_window_drag')
  }
}

async function minimizeWindow() {
  await invoke('minimize_window')
}

async function maximizeWindow() {
  await invoke('toggle_maximize_window')
}

async function closeWindow() {
  await invoke('close_window')
}

function shortId(id?: string) {
  if (!id) return '未知设备'
  return id.length > 12 ? `${id.slice(0, 6)}…${id.slice(-6)}` : id
}

function formatBytes(value: number) {
  if (!Number.isFinite(value) || value <= 0) return '0 B'

  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let size = value
  let unitIndex = 0

  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024
    unitIndex += 1
  }

  return `${size.toFixed(size >= 10 || unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`
}

function formatDuration(seconds: number) {
  if (!Number.isFinite(seconds) || seconds <= 0) return isReady.value ? '刚刚启动' : '等待数据'

  const days = Math.floor(seconds / 86400)
  const hours = Math.floor((seconds % 86400) / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)

  if (days > 0) return `${days} 天 ${hours} 小时`
  if (hours > 0) return `${hours} 小时 ${minutes} 分钟`
  return `${minutes} 分钟`
}

onMounted(() => {
  refreshData()
})
</script>

<template>
  <main class="app-shell">
    <div class="custom-titlebar" @mousedown="startWindowDrag">
      <div class="titlebar-title">AeroSync</div>
      <div class="window-controls">
        <button class="titlebar-button" aria-label="最小化" @mousedown.stop @click="minimizeWindow">—</button>
        <button class="titlebar-button" aria-label="最大化" @mousedown.stop @click="maximizeWindow">□</button>
        <button class="titlebar-button close" aria-label="关闭" @mousedown.stop @click="closeWindow">×</button>
      </div>
    </div>

    <aside class="sidebar">
      <div class="sidebar-brand">
        <div class="brand-icon">A</div>
        <div>
          <strong>AeroSync</strong>
          <span>同步管理</span>
        </div>
      </div>

      <nav class="side-nav">
        <button :class="activeView === 'tasks' ? 'nav-item active' : 'nav-item'" @click="activeView = 'tasks'">
          <span class="nav-icon">↔</span>
          <span>同步任务</span>
        </button>
        <button :class="activeView === 'devices' ? 'nav-item active' : 'nav-item'" @click="activeView = 'devices'">
          <span class="nav-icon">▣</span>
          <span>设备管理</span>
        </button>
        <button :class="activeView === 'transfers' ? 'nav-item active' : 'nav-item'" @click="activeView = 'transfers'">
          <span class="nav-icon">◷</span>
          <span>传输记录</span>
        </button>
        <button :class="activeView === 'settings' ? 'nav-item active' : 'nav-item'" @click="activeView = 'settings'">
          <span class="nav-icon">⚙</span>
          <span>设置</span>
        </button>
      </nav>

      <div class="sidebar-footer">
        <span :class="isReady ? 'status-dot online' : 'status-dot'" />
        <span>{{ statusText }}</span>
      </div>
    </aside>

    <section class="main-content">
      <header class="page-header">
        <div>
          <h1>{{ pageTitle }}</h1>
          <p>{{ pageDescription }}</p>
        </div>

        <div class="header-actions">
          <button class="secondary-button" :disabled="isLoading || isMutating" @click="refreshData">
            {{ isLoading ? '刷新中…' : '刷新' }}
          </button>
          <button class="secondary-button" @click="openWebUi">打开 Web UI</button>
          <button v-if="activeView === 'tasks'" class="primary-button" :disabled="!isReady || isMutating" @click="openFolderModal">添加同步任务</button>
          <button v-else-if="activeView === 'devices'" class="primary-button" :disabled="!isReady || isMutating" @click="openDeviceModal">添加设备</button>
          <button v-else-if="activeView === 'transfers'" class="primary-button" :disabled="isLoading || isMutating" @click="refreshData">刷新连接</button>
          <button v-else class="primary-button" :disabled="!isRunning || isMutating" @click="restartCore">重启核心</button>
        </div>
      </header>

      <section class="summary-grid">
        <article class="summary-card">
          <span>核心状态</span>
          <strong :class="isReady ? 'text-green' : 'text-red'">{{ statusText }}</strong>
          <small>最后刷新：{{ lastUpdated }}</small>
        </article>
        <article class="summary-card">
          <span>同步文件夹</span>
          <strong>{{ folders.length }}</strong>
          <small>{{ folders.filter((folder) => !folder.paused).length }} 个正在同步</small>
        </article>
        <article class="summary-card">
          <span>连接设备</span>
          <strong>{{ connectedDevices }} / {{ remoteDevices.length }}</strong>
          <small>运行时长：{{ uptimeText }}</small>
        </article>
        <article class="summary-card">
          <span>传输流量</span>
          <strong>{{ totalTraffic }}</strong>
          <small>API：127.0.0.1:58384</small>
        </article>
      </section>

      <p v-if="restartRequired" class="notice">Syncthing 提示需要重启核心才能完全应用配置。</p>
      <p v-if="successMessage" class="notice success">{{ successMessage }}</p>
      <p v-if="errorMessage" class="notice error">{{ errorMessage }}</p>

      <section class="workspace-grid">
        <section v-if="activeView === 'tasks'" class="panel task-panel">
          <div class="panel-titlebar">
            <div>
              <h2>同步文件夹</h2>
              <p>当前已配置的同步目录</p>
            </div>
            <div class="panel-actions">
              <button class="secondary-button" :disabled="!isReady || isMutating" @click="rescanAllFolders">扫描全部</button>
              <button class="secondary-button" :disabled="!isReady || isMutating" @click="openFolderModal">添加文件夹</button>
            </div>
          </div>

          <div class="table-head folder-head">
            <span>名称</span>
            <span>共享设备</span>
            <span>状态</span>
            <span>操作</span>
          </div>

          <div v-if="folders.length" class="list-table">
            <article v-for="folder in folders" :key="folder.id" class="table-row folder-row">
              <div class="name-cell">
                <strong>{{ folder.label || folder.id }}</strong>
                <span>{{ folder.path || '未设置路径' }}</span>
              </div>
              <span>{{ folder.devices?.filter((device) => device.deviceID !== localDeviceId).length ?? 0 }} 台</span>
              <span :class="folder.paused ? 'pill muted' : 'pill'">
                {{ folder.paused ? '已暂停' : '同步中' }}
              </span>
              <div class="row-actions">
                <button :disabled="isMutating" @click="rescanFolder(folder)">扫描</button>
                <button v-if="folder.paused" :disabled="isMutating" @click="resumeFolder(folder)">恢复</button>
                <button v-else :disabled="isMutating" @click="pauseFolder(folder)">暂停</button>
                <button class="danger-link" :disabled="isMutating" @click="confirmRemoveFolder(folder)">删除</button>
              </div>
            </article>
          </div>
          <div v-else class="empty-state">还没有同步文件夹，点击右上角添加。</div>
        </section>

        <section v-if="activeView === 'devices'" class="panel device-panel">
          <div class="panel-titlebar">
            <div>
              <h2>设备管理</h2>
              <p>远程设备连接状态</p>
            </div>
            <button class="secondary-button" :disabled="!isReady || isMutating" @click="openDeviceModal">添加设备</button>
          </div>

          <div v-if="remoteDevices.length" class="device-list">
            <article v-for="device in remoteDevices" :key="device.deviceID" class="device-card">
              <div class="device-avatar">{{ (device.name || device.deviceID || '?').slice(0, 1) }}</div>
              <div>
                <strong>{{ device.name || shortId(device.deviceID) }}</strong>
                <span>{{ shortId(device.deviceID) }}</span>
              </div>
              <span :class="connections.connections?.[device.deviceID]?.connected ? 'pill' : 'pill muted'">
                {{ connections.connections?.[device.deviceID]?.connected ? '在线' : device.paused ? '已暂停' : '离线' }}
              </span>
              <div class="device-actions">
                <button v-if="device.paused" :disabled="isMutating" @click="resumeDevice(device)">恢复</button>
                <button v-else :disabled="isMutating" @click="pauseDevice(device)">暂停</button>
                <button class="danger-link" :disabled="isMutating" @click="confirmRemoveDevice(device)">删除</button>
              </div>
            </article>
          </div>
          <div v-else class="empty-state small">还没有远程设备。</div>
        </section>

        <section v-if="activeView === 'transfers'" class="panel transfer-panel">
          <div class="panel-titlebar">
            <div>
              <h2>传输记录</h2>
              <p>当前 Syncthing 连接和流量概览</p>
            </div>
            <button class="secondary-button" :disabled="isLoading || isMutating" @click="refreshData">刷新</button>
          </div>

          <div class="table-head transfer-head">
            <span>设备</span>
            <span>地址</span>
            <span>类型</span>
            <span>状态</span>
          </div>
          <div v-if="connectionEntries.length" class="list-table">
            <article v-for="connection in connectionEntries" :key="connection.deviceId" class="table-row transfer-row">
              <div class="name-cell">
                <strong>{{ shortId(connection.deviceId) }}</strong>
                <span>{{ connection.deviceId }}</span>
              </div>
              <span>{{ connection.address || '未知' }}</span>
              <span>{{ connection.type || '未知' }}</span>
              <span :class="connection.connected ? 'pill' : 'pill muted'">
                {{ connection.connected ? '已连接' : '未连接' }}
              </span>
            </article>
          </div>
          <div v-else class="empty-state small">暂无连接记录。</div>
        </section>

        <section v-if="activeView === 'settings'" class="panel system-panel">
          <div class="panel-titlebar">
            <div>
              <h2>系统信息</h2>
              <p>Syncthing 核心详情</p>
            </div>
            <div class="panel-actions">
              <button v-if="!isRunning" class="secondary-button" :disabled="isMutating" @click="startCore">启动核心</button>
              <button v-else class="secondary-button" :disabled="isMutating" @click="restartCore">重启核心</button>
              <button v-if="isRunning" class="danger-button" :disabled="isMutating" @click="shutdownCore">停止核心</button>
              <button class="secondary-button" :disabled="!isReady || isMutating" @click="pauseAllDevices">暂停全部设备</button>
              <button class="secondary-button" :disabled="!isReady || isMutating" @click="resumeAllDevices">恢复全部设备</button>
            </div>
          </div>

          <div class="info-grid">
            <div>
              <span>本机设备 ID</span>
              <strong>{{ shortId(systemStatus.myID) }}</strong>
            </div>
            <div>
              <span>发现服务</span>
              <strong>{{ systemStatus.discoveryEnabled ? '已启用' : '未启用' }}</strong>
            </div>
            <div>
              <span>启动时间</span>
              <strong>{{ systemStatus.startTime || '等待数据' }}</strong>
            </div>
            <div>
              <span>API 端点</span>
              <strong>127.0.0.1:58384</strong>
            </div>
          </div>
        </section>
      </section>
    </section>

    <div v-if="activeModal" class="modal-backdrop" @click.self="closeModal">
      <form v-if="activeModal === 'folder'" class="modal-card" @submit.prevent="submitFolder">
        <div class="modal-heading">
          <h2>添加同步文件夹</h2>
          <button type="button" @click="closeModal">×</button>
        </div>
        <label class="form-field">
          <span>文件夹 ID</span>
          <input v-model="folderForm.id" placeholder="例如 photos" />
        </label>
        <label class="form-field">
          <span>显示名称</span>
          <input v-model="folderForm.label" placeholder="例如 照片同步" />
        </label>
        <label class="form-field">
          <span>本地路径</span>
          <input v-model="folderForm.path" placeholder="例如 /home/user/Pictures" />
        </label>
        <div class="form-field">
          <span>共享设备</span>
          <div v-if="remoteDevices.length" class="checkbox-list">
            <label v-for="device in remoteDevices" :key="device.deviceID">
              <input v-model="folderForm.deviceIds" type="checkbox" :value="device.deviceID" />
              <span>{{ device.name || shortId(device.deviceID) }}</span>
            </label>
          </div>
          <small v-else>暂无远程设备，可稍后添加设备后再共享。</small>
        </div>
        <div class="form-actions">
          <button type="button" class="secondary-button" @click="closeModal">取消</button>
          <button type="submit" class="primary-button" :disabled="isMutating">添加</button>
        </div>
      </form>

      <form v-else class="modal-card" @submit.prevent="submitDevice">
        <div class="modal-heading">
          <h2>添加远程设备</h2>
          <button type="button" @click="closeModal">×</button>
        </div>
        <label class="form-field">
          <span>设备 ID</span>
          <textarea v-model="deviceForm.deviceId" rows="3" placeholder="粘贴 Syncthing 设备 ID" />
        </label>
        <label class="form-field">
          <span>设备名称</span>
          <input v-model="deviceForm.name" placeholder="例如 NAS / Laptop" />
        </label>
        <label class="form-field">
          <span>地址</span>
          <textarea v-model="deviceForm.addresses" rows="3" placeholder="dynamic 或 tcp://host:22000，多个用逗号/换行分隔" />
        </label>
        <div class="form-field">
          <span>共享文件夹</span>
          <div v-if="folders.length" class="checkbox-list">
            <label v-for="folder in folders" :key="folder.id">
              <input v-model="deviceForm.folderIds" type="checkbox" :value="folder.id" />
              <span>{{ folder.label || folder.id }}</span>
            </label>
          </div>
          <small v-else>暂无同步文件夹。</small>
        </div>
        <div class="form-actions">
          <button type="button" class="secondary-button" @click="closeModal">取消</button>
          <button type="submit" class="primary-button" :disabled="isMutating">添加</button>
        </div>
      </form>
    </div>

    <div v-if="confirmAction" class="modal-backdrop" @click.self="confirmAction = null">
      <section class="modal-card confirm-card">
        <div class="modal-heading">
          <h2>{{ confirmAction.title }}</h2>
          <button type="button" @click="confirmAction = null">×</button>
        </div>
        <p>{{ confirmAction.message }}</p>
        <div class="form-actions">
          <button class="secondary-button" @click="confirmAction = null">取消</button>
          <button :class="confirmAction.danger ? 'danger-button' : 'primary-button'" :disabled="isMutating" @click="runConfirmedAction">
            确认
          </button>
        </div>
      </section>
    </div>
  </main>
</template>

<template>
  <div class="h-8 bg-gray-100 dark:bg-gray-800 border-t border-gray-200 dark:border-gray-700 flex items-center justify-between px-4 text-xs">
    <!-- 左侧状态信息 -->
    <div class="flex items-center space-x-6">
      <!-- 代理状态 -->
      <div class="flex items-center space-x-2">
        <div :class="['w-2 h-2 rounded-full', getProxyStatusColor()]"></div>
        <span class="text-gray-600 dark:text-gray-400">
          {{ proxyStatusText }}
        </span>
      </div>
      
      <!-- 代理模式 -->
      <div class="flex items-center space-x-2">
        <Icon name="heroicons:globe-alt" class="w-3 h-3 text-gray-500" />
        <span class="text-gray-600 dark:text-gray-400 select-none">
          {{ proxyModeText }}
        </span>
      </div>
      
      <!-- 当前服务器 -->
      <div v-if="currentServer" class="flex items-center space-x-2">
        <Icon name="heroicons:server" class="w-3 h-3 text-gray-500" />
        <span class="text-gray-600 dark:text-gray-400 max-w-32 truncate" :title="currentServer.name + ' (' + currentServer.ping + 'ms)'">
          {{ currentServer.name }} ({{ currentServer.ping }}ms)
        </span>
      </div>
      
      <!-- TUN模式开关 -->
      <div class="flex items-center space-x-2 cursor-pointer" @click="toggleTunMode">
        <Icon name="heroicons:globe-alt" class="w-3 h-3 text-gray-500" />
        <span class="text-gray-600 dark:text-gray-400 select-none" :class="{ 'text-green-600 dark:text-green-400': tunEnabled }">
          {{ tunModeText }}
        </span>
        <div class="relative inline-flex h-4 w-7 items-center rounded-full transition-colors" :class="tunEnabled ? 'bg-green-600' : 'bg-gray-300 dark:bg-gray-600'">
          <span class="inline-block h-3 w-3 transform rounded-full bg-white transition-transform" :class="tunEnabled ? 'translate-x-3.5' : 'translate-x-0.5'"></span>
        </div>
      </div>
    </div>
    
    <!-- 右侧系统信息 -->
    <div class="flex items-center space-x-6">
      <!-- 网络速率 -->
      <div class="flex items-center space-x-4">
        <div class="flex items-center space-x-1">
          <Icon name="heroicons:arrow-up" class="w-3 h-3 text-green-500" />
          <span class="speed-display text-green-600 dark:text-green-400">
            {{ formatSpeed(uploadSpeed) }}
          </span>
        </div>
        
        <div class="flex items-center space-x-1">
          <Icon name="heroicons:arrow-down" class="w-3 h-3 text-blue-500" />
          <span class="speed-display text-blue-600 dark:text-blue-400">
            {{ formatSpeed(downloadSpeed) }}
          </span>
        </div>
      </div>
      
      <!-- 流量统计 -->
      <div class="flex items-center space-x-2">
        <Icon name="heroicons:chart-bar" class="w-3 h-3 text-gray-500" />
        <span class="text-gray-600 dark:text-gray-400">
          {{ formatBytes(totalTraffic) }}
        </span>
      </div>
      
      <!-- CPU 使用率 -->
      <div class="flex items-center space-x-2">
        <Icon name="heroicons:cpu-chip" class="w-3 h-3 text-gray-500" />
        <span class="text-gray-600 dark:text-gray-400">
          CPU: {{ cpuUsage }}%
        </span>
      </div>
      
      <!-- 内存使用率 -->
      <div class="flex items-center space-x-2">
        <Icon name="heroicons:circle-stack" class="w-3 h-3 text-gray-500" />
        <span class="text-gray-600 dark:text-gray-400">
          内存: {{ formatBytes(memoryUsage) }}
        </span>
      </div>
      
      <!-- 运行时间 -->
      <div class="flex items-center space-x-2">
        <Icon name="heroicons:clock" class="w-3 h-3 text-gray-500" />
        <span class="text-gray-600 dark:text-gray-400">
          {{ uptime }}
        </span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

// Toast 通知
const toast = useToast()

// 接口定义
interface Server {
  name: string
  ping: number
}

interface SystemStats {
  cpu_usage: number
  memory_usage: number
  memory_used: number
  network_upload: number
  network_download: number
}

// 网络速度统计接口
interface NetworkSpeedStats {
  upload_speed: number
  download_speed: number
  total_upload: number
  total_download: number
}

interface ProxyStatus {
  is_running: boolean
  status: string // "connected" | "connecting" | "disconnected"
  current_server: string | null
  proxy_mode: string
  uptime: number
  upload_speed: number
  download_speed: number
  total_upload: number
  total_download: number
}

// 状态
const proxyStatus = ref<'connected' | 'connecting' | 'disconnected'>('disconnected')
const proxyMode = ref<'direct' | 'global' | 'pac'>('direct')
const currentServer = ref<Server | null>(null)
const tunEnabled = ref(false)
const tunStatus = ref<any>(null)

// 网络统计
const uploadSpeed = ref(0) // bytes/s
const downloadSpeed = ref(0) // bytes/s
const totalTraffic = ref(0) // bytes

// 系统资源
const cpuUsage = ref(0) // percentage
const memoryUsage = ref(0) // bytes
const startTime = ref(Date.now())

let updateInterval: NodeJS.Timeout | null = null

// 计算属性
const proxyStatusText = computed(() => {
  const statusMap = {
    connected: '已连接',
    connecting: '连接中',
    disconnected: '未连接'
  }
  return statusMap[proxyStatus.value]
})

const proxyModeText = computed(() => {
  const modeMap = {
    direct: '直连模式',
    global: '全局代理',
    pac: 'PAC 模式'
  }
  return modeMap[proxyMode.value]
})

const tunModeText = computed(() => {
  return tunEnabled.value ? 'TUN模式' : '代理模式'
})

const uptime = computed(() => {
  const now = Date.now()
  const diff = now - startTime.value
  
  const hours = Math.floor(diff / (1000 * 60 * 60))
  const minutes = Math.floor((diff % (1000 * 60 * 60)) / (1000 * 60))
  const seconds = Math.floor((diff % (1000 * 60)) / 1000)
  
  return `${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')}`
})

// 方法
const getProxyStatusColor = () => {
  const colorMap = {
    connected: 'bg-green-500',
    connecting: 'bg-yellow-500 animate-pulse',
    disconnected: 'bg-red-500'
  }
  return colorMap[proxyStatus.value]
}

const formatSpeed = (bytesPerSecond: number) => {
  if (bytesPerSecond === 0) return '0 B/s'
  
  const units = ['B/s', 'KB/s', 'MB/s', 'GB/s']
  const k = 1024
  const i = Math.floor(Math.log(bytesPerSecond) / Math.log(k))
  
  return `${(bytesPerSecond / Math.pow(k, i)).toFixed(1)} ${units[i]}`
}

const formatBytes = (bytes: number) => {
  if (bytes === 0) return '0 B'
  
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const k = 1024
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  
  return `${(bytes / Math.pow(k, i)).toFixed(1)} ${units[i]}`
}

const updateSystemStats = async () => {
  try {
    // 获取基础系统统计（CPU、内存）
    const systemStats = await invoke('get_system_stats') as SystemStats
    cpuUsage.value = Math.round(systemStats.cpu_usage)
    memoryUsage.value = systemStats.memory_used
    
    // 获取网络速度和流量统计
     const networkStats = await invoke('get_network_speed') as NetworkSpeedStats
     uploadSpeed.value = networkStats.upload_speed
     downloadSpeed.value = networkStats.download_speed
     totalTraffic.value = networkStats.total_upload + networkStats.total_download
    
    // 获取TUN模式状态
    try {
      const tunRunning = await invoke('is_tun_running') as boolean
      tunEnabled.value = tunRunning
      if (tunRunning) {
        tunStatus.value = await invoke('get_tun_status')
      }
    } catch (error) {
      console.warn('获取TUN状态失败:', error)
    }
    
    // 获取代理状态信息
    const proxyStatusData = await invoke('get_proxy_status') as ProxyStatus
    
    
    // 更新代理状态 - 使用后端返回的真实状态
    const statusMap: { [key: string]: 'connected' | 'connecting' | 'disconnected' } = {
      'connected': 'connected',
      'connecting': 'connecting',
      'disconnected': 'disconnected'
    }
    proxyStatus.value = statusMap[proxyStatusData.status] || 'disconnected'
    
    // 更新当前服务器信息
    if (proxyStatusData.current_server && proxyStatusData.is_running) {
      currentServer.value = {
        name: proxyStatusData.current_server,
        ping: 0 // 代理状态中没有ping信息，设为0
      }
    } else {
      currentServer.value = null
    }
    
    // 更新代理模式
    const modeMap: { [key: string]: 'direct' | 'global' | 'pac' } = {
      'direct': 'direct',
      'global': 'global',
      'pac': 'pac'
    }
    proxyMode.value = modeMap[proxyStatusData.proxy_mode] || 'direct'
    
  } catch (error) {
    console.error('更新系统统计失败:', error)
    
    // 如果API调用失败，使用默认值
    cpuUsage.value = 0
    memoryUsage.value = 0
    uploadSpeed.value = 0
    downloadSpeed.value = 0
    proxyStatus.value = 'disconnected'
    currentServer.value = null
  }
}

// 监听代理模式变化事件
const handleProxyModeChange = (event: any) => {
  const { proxy_mode } = event.payload
  
  // 立即更新代理模式显示
  const modeMap: { [key: string]: 'direct' | 'global' | 'pac' } = {
    'direct': 'direct',
    'global': 'global',
    'pac': 'pac'
  }
  proxyMode.value = modeMap[proxy_mode] || 'direct'
}

// 生命周期
onMounted(async () => {
  // 每秒更新一次统计信息
  updateInterval = setInterval(updateSystemStats, 1000)
  
  // 初始化数据
  updateSystemStats()
  
  // 监听代理模式变化事件
  try {
    const { listen } = await import('@tauri-apps/api/event')
    const unlisten = await listen('proxy-mode-changed', handleProxyModeChange)
    
    // 组件卸载时清理监听器
    onUnmounted(() => {
      unlisten()
    })
  } catch (error) {
    console.warn('监听代理模式变化事件失败:', error)
  }
})

onUnmounted(() => {
  if (updateInterval) {
    clearInterval(updateInterval)
  }
})

// 暴露方法给父组件
const setProxyStatus = (status: 'connected' | 'connecting' | 'disconnected') => {
  proxyStatus.value = status
}

const setProxyMode = (mode: 'direct' | 'global' | 'pac') => {
  proxyMode.value = mode
}

const setCurrentServer = (server: Server | null) => {
  currentServer.value = server
}

// TUN模式切换
const toggleTunMode = async () => {
  try {
    const newState = !tunEnabled.value
    await invoke('toggle_tun_mode', { enabled: newState })
    tunEnabled.value = newState
    
    // 显示状态提示
    if (newState) {
      toast.add({
        title: 'TUN模式已启用',
        description: '虚拟网卡已成功启动',
        icon: 'i-heroicons-globe-alt',
        color: 'green'
      })
    } else {
      toast.add({
        title: 'TUN模式已禁用',
        description: '虚拟网卡已停止',
        icon: 'i-heroicons-globe-alt',
        color: 'gray'
      })
    }
  } catch (error) {
    console.error('切换TUN模式失败:', error)
    
    const errorMessage = typeof error === 'string' ? error : (error as any)?.message || '切换TUN模式失败'
    
    // 显示错误通知
    toast.add({
      title: 'TUN模式切换失败',
      description: errorMessage,
      icon: 'i-heroicons-exclamation-triangle',
      color: 'red'
    })
  }
}

defineExpose({
  setProxyStatus,
  setProxyMode,
  setCurrentServer,
  toggleTunMode
})
</script>
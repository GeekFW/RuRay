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
      
      <!-- 当前服务器 -->
      <div v-if="currentServer" class="flex items-center space-x-2">
        <Icon name="heroicons:server" class="w-3 h-3 text-gray-500" />
        <span class="text-gray-600 dark:text-gray-400">
          {{ currentServer.name }} ({{ currentServer.ping }}ms)
        </span>
      </div>
      
      <!-- 代理模式 -->
      <div class="flex items-center space-x-2">
        <Icon name="heroicons:globe-alt" class="w-3 h-3 text-gray-500" />
        <span class="text-gray-600 dark:text-gray-400">
          {{ proxyModeText }}
        </span>
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

// 接口定义
interface Server {
  name: string
  ping: number
}

// 状态
const proxyStatus = ref<'connected' | 'connecting' | 'disconnected'>('disconnected')
const proxyMode = ref<'direct' | 'global' | 'pac'>('direct')
const currentServer = ref<Server | null>(null)

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
    // TODO: 从 Rust 后端获取真实的系统统计信息
    // 这里使用模拟数据
    
    // 模拟网络速率变化
    uploadSpeed.value = Math.random() * 1024 * 1024 // 0-1MB/s
    downloadSpeed.value = Math.random() * 10 * 1024 * 1024 // 0-10MB/s
    totalTraffic.value += (uploadSpeed.value + downloadSpeed.value)
    
    // 模拟 CPU 使用率
    cpuUsage.value = Math.floor(Math.random() * 20) + 5 // 5-25%
    
    // 模拟内存使用
    memoryUsage.value = 50 * 1024 * 1024 + Math.random() * 20 * 1024 * 1024 // 50-70MB
    
    // 模拟代理状态变化
    if (Math.random() < 0.1) { // 10% 概率改变状态
      const statuses: Array<'connected' | 'connecting' | 'disconnected'> = ['connected', 'connecting', 'disconnected']
      proxyStatus.value = statuses[Math.floor(Math.random() * statuses.length)]
      
      if (proxyStatus.value === 'connected' && !currentServer.value) {
        currentServer.value = {
          name: '示例服务器',
          ping: Math.floor(Math.random() * 200) + 50
        }
      } else if (proxyStatus.value === 'disconnected') {
        currentServer.value = null
      }
    }
  } catch (error) {
    console.error('更新系统统计失败:', error)
  }
}

// 生命周期
onMounted(() => {
  // 每秒更新一次统计信息
  updateInterval = setInterval(updateSystemStats, 1000)
  
  // 初始化数据
  updateSystemStats()
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

defineExpose({
  setProxyStatus,
  setProxyMode,
  setCurrentServer
})
</script>
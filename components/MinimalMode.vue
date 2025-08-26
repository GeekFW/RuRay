<template>
  <div class="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50">
    <div class="minimal-mode p-6 max-w-md w-full mx-4">
      <!-- 头部 -->
      <div class="flex items-center justify-between mb-6">
        <div class="flex items-center space-x-3">
          <div class="w-8 h-8 bg-gradient-to-br from-green-400 to-green-600 rounded-lg flex items-center justify-center">
            <Icon name="heroicons:bolt" class="w-5 h-5 text-white" />
          </div>
          <h2 class="text-lg font-semibold text-white">RuRay</h2>
        </div>
        
        <UButton
          variant="ghost"
          size="sm"
          @click="$emit('close')"
          class="text-white hover:bg-white/10"
        >
          <Icon name="heroicons:x-mark" class="w-4 h-4" />
        </UButton>
      </div>
      
      <!-- 连接状态 -->
      <div class="text-center mb-6">
        <div class="flex items-center justify-center space-x-3 mb-3">
          <div :class="['w-4 h-4 rounded-full', getStatusColor()]"></div>
          <span class="text-xl font-medium text-white">
            {{ statusText }}
          </span>
        </div>
        
        <div v-if="currentServer" class="text-sm text-gray-300">
          {{ currentServer.name }} • {{ currentServer.ping }}ms
        </div>
      </div>
      
      <!-- 网络速率 -->
      <div class="grid grid-cols-2 gap-4 mb-6">
        <div class="text-center">
          <div class="flex items-center justify-center space-x-1 mb-1">
            <Icon name="heroicons:arrow-up" class="w-4 h-4 text-green-400" />
            <span class="text-sm text-gray-300">上传</span>
          </div>
          <div class="speed-display text-lg font-bold text-green-400">
            {{ formatSpeed(uploadSpeed) }}
          </div>
        </div>
        
        <div class="text-center">
          <div class="flex items-center justify-center space-x-1 mb-1">
            <Icon name="heroicons:arrow-down" class="w-4 h-4 text-blue-400" />
            <span class="text-sm text-gray-300">下载</span>
          </div>
          <div class="speed-display text-lg font-bold text-blue-400">
            {{ formatSpeed(downloadSpeed) }}
          </div>
        </div>
      </div>
      
      <!-- 流量统计 -->
      <div class="text-center mb-6">
        <div class="text-sm text-gray-300 mb-1">总流量</div>
        <div class="text-lg font-medium text-white">
          {{ formatBytes(totalTraffic) }}
        </div>
      </div>
      
      <!-- 快速操作 -->
      <div class="grid grid-cols-2 gap-3">
        <UButton
          :color="isConnected ? 'red' : selectedThemeColor"
          variant="solid"
          block
          @click="toggleConnection"
        >
          <Icon :name="isConnected ? 'heroicons:stop' : 'heroicons:play'" class="w-4 h-4 mr-2" />
          {{ isConnected ? '断开' : '连接' }}
        </UButton>
        
        <UButton
          variant="outline"
          block
          @click="showServerList = true"
          class="border-white/20 text-white hover:bg-white/10"
        >
          <Icon name="heroicons:server" class="w-4 h-4 mr-2" />
          切换服务器
        </UButton>
      </div>
      
      <!-- 代理模式切换 -->
      <div class="mt-4">
        <div class="text-sm text-gray-300 mb-2">代理模式</div>
        <div class="grid grid-cols-3 gap-2">
          <UButton
            v-for="mode in proxyModes"
            :key="mode.value"
            :variant="proxyMode === mode.value ? 'solid' : 'outline'"
            :color="proxyMode === mode.value ? selectedThemeColor : 'gray'"
            size="xs"
            @click="setProxyMode(mode.value)"
            class="text-xs"
          >
            {{ mode.label }}
          </UButton>
        </div>
      </div>
    </div>
    
    <!-- 服务器选择对话框 -->
    <UModal v-model="showServerList">
      <UCard class="max-w-md">
        <template #header>
          <div class="flex items-center space-x-2">
            <Icon name="heroicons:server" class="w-5 h-5 text-green-500" />
            <span>选择服务器</span>
          </div>
        </template>
        
        <div class="space-y-2 max-h-64 overflow-y-auto">
          <div
            v-for="server in servers"
            :key="server.id"
            :class="[
              'p-3 rounded-lg border cursor-pointer transition-colors',
              server.id === currentServerId 
                ? 'border-green-500 bg-green-50 dark:bg-green-900/20' 
                : 'border-gray-200 dark:border-gray-700 hover:border-gray-300'
            ]"
            @click="selectServer(server)"
          >
            <div class="flex items-center justify-between">
              <div>
                <div class="font-medium">{{ server.name }}</div>
                <div class="text-sm text-gray-500">
                  {{ server.address }}:{{ server.port }}
                </div>
              </div>
              
              <div class="text-right">
                <UBadge
                  :color="getProtocolColor(server.protocol)"
                  variant="soft"
                  size="xs"
                >
                  {{ server.protocol.toUpperCase() }}
                </UBadge>
                <div v-if="server.ping" class="text-xs text-gray-500 mt-1">
                  {{ server.ping }}ms
                </div>
              </div>
            </div>
          </div>
        </div>
        
        <template #footer>
          <div class="flex justify-end">
            <UButton variant="ghost" @click="showServerList = false">
              关闭
            </UButton>
          </div>
        </template>
      </UCard>
    </UModal>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'

// 获取应用配置以访问主题色
const appConfig = useAppConfig()
const selectedThemeColor = computed(() => appConfig.ui?.primary || 'green')

// 接口定义
interface Server {
  id: string
  name: string
  address: string
  port: number
  protocol: string
  ping?: number
}

// Props
const props = defineProps<{
  isConnected?: boolean
  currentServer?: { name: string; ping: number } | null
  uploadSpeed?: number
  downloadSpeed?: number
  totalTraffic?: number
  proxyMode?: string
}>()

// Emits
defineEmits<{
  close: []
  toggleConnection: []
  selectServer: [server: Server]
  setProxyMode: [mode: string]
}>()

// 状态
const showServerList = ref(false)
const currentServerId = ref<string | null>(null)

// 模拟数据
const servers = ref<Server[]>([
  {
    id: '1',
    name: '示例服务器 1',
    address: 'example1.com',
    port: 443,
    protocol: 'vmess',
    ping: 120
  },
  {
    id: '2',
    name: '示例服务器 2',
    address: 'example2.com',
    port: 443,
    protocol: 'vless',
    ping: 85
  }
])

const proxyModes = [
  { label: '直连', value: 'direct' },
  { label: '全局', value: 'global' },
  { label: 'PAC', value: 'pac' }
]

// 计算属性
const statusText = computed(() => {
  if (props.isConnected) return '已连接'
  return '未连接'
})

// 方法
const getStatusColor = () => {
  if (props.isConnected) return 'bg-green-500'
  return 'bg-red-500'
}

const getProtocolColor = (protocol: string) => {
  const colors: Record<string, string> = {
    vmess: 'blue',
    vless: 'green',
    trojan: 'purple',
    socks5: 'orange',
    http: 'gray'
  }
  return colors[protocol] || 'gray'
}

const formatSpeed = (bytesPerSecond: number = 0) => {
  if (bytesPerSecond === 0) return '0 B/s'
  
  const units = ['B/s', 'KB/s', 'MB/s', 'GB/s']
  const k = 1024
  const i = Math.floor(Math.log(bytesPerSecond) / Math.log(k))
  
  return `${(bytesPerSecond / Math.pow(k, i)).toFixed(1)} ${units[i]}`
}

const formatBytes = (bytes: number = 0) => {
  if (bytes === 0) return '0 B'
  
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const k = 1024
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  
  return `${(bytes / Math.pow(k, i)).toFixed(1)} ${units[i]}`
}

const toggleConnection = () => {
  // TODO: 实现连接切换逻辑
}

const selectServer = (server: Server) => {
  currentServerId.value = server.id
  showServerList.value = false
  // TODO: 切换到选中的服务器
}

const setProxyMode = (mode: string) => {
  // TODO: 实现代理模式切换
}
</script>
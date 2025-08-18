<template>
  <div class="min-h-screen bg-gray-50 dark:bg-gray-900">
    <!-- 启动加载动画 -->
    <LoadingScreen v-if="isLoading" />
    
    <!-- 主应用界面 -->
    <div v-else-if="!isZenMode" class="h-screen flex flex-col">
      <!-- 顶部菜单栏 -->
      <AppHeader @toggle-zen-mode="toggleZenMode" />
      
      <!-- 主内容区域 -->
      <div class="flex-1 flex overflow-hidden">
        <!-- 左侧服务器列表 -->
        <ServerList class="w-80 border-r border-gray-200 dark:border-gray-700" />
        
        <!-- 右侧日志区域 -->
        <LogViewer class="flex-1" />
      </div>
      
      <!-- 底部状态栏 -->
      <StatusBar />
    </div>
    
    <!-- 极简模式 (Zen Mode) -->
    <Zen 
      v-if="isZenMode"
      :is-connected="appState.isConnected"
      :active-server="appState.activeServer"
      :upload-speed="appState.uploadSpeed"
      :download-speed="appState.downloadSpeed"
      :total-traffic="appState.totalTraffic"
      :uptime="appState.uptime"
      :proxy-mode="appState.proxyMode"
      @close="toggleZenMode"
      @toggle-connection="toggleConnection"
      @switch-server="switchServer"
    />
    
    <!-- 全局通知 -->
    <UNotifications />
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

/**
 * 服务器接口定义
 */
interface Server {
  id: string
  name: string
  address: string
  port: number
  protocol: string
  ping?: number
  isActive?: boolean
  status?: 'connected' | 'connecting' | 'disconnected'
  uuid?: string
  password?: string
  username?: string
  security?: string
  network?: string
  path?: string
  tls?: boolean
  sni?: string
  created_at?: string
  updated_at?: string
}

// 应用状态
const isLoading = ref(true)
const isZenMode = ref(false)

// 服务器列表和当前服务器索引
const servers = ref<Server[]>([])
const currentServerIndex = ref(0)
const runningServerId = ref<string | null>(null)

// 应用数据状态
const appState = reactive({
  isConnected: false,
  activeServer: null as Server | null,
  uploadSpeed: 0,
  downloadSpeed: 0,
  totalTraffic: 0,
  uptime: 0,
  proxyMode: 'global' as 'global' | 'pac' | 'direct'
})

/**
 * 切换极简模式
 */
const toggleZenMode = () => {
  isZenMode.value = !isZenMode.value
}

/**
 * 切换连接状态
 */
const toggleConnection = async () => {
  if (appState.isConnected) {
    // 断开连接
    await stopProxy()
  } else {
    // 开始连接 - 如果没有服务器启动，默认启动第一个服务器
    if (servers.value.length === 0) {
      await loadServers()
    }
    
    if (servers.value.length > 0) {
      // 如果没有运行中的服务器，启动第一个服务器
      if (!runningServerId.value) {
        currentServerIndex.value = 0
        await startProxy(servers.value[0].id)
      } else {
        // 如果已有运行中的服务器，直接连接
        const runningServer = servers.value.find(s => s.id === runningServerId.value)
        if (runningServer) {
          appState.activeServer = runningServer
          appState.isConnected = true
          startNetworkMonitoring()
        }
      }
    } else {
      // 没有服务器配置
      const toast = useToast()
      toast.add({
        title: '无可用服务器',
        description: '请先添加服务器配置',
        icon: 'i-heroicons-exclamation-triangle',
        color: 'orange'
      })
    }
  }
}

/**
 * 切换服务器
 */
const switchServer = async () => {
  if (servers.value.length <= 1) {
    const toast = useToast()
    toast.add({
      title: '无其他服务器',
      description: '当前只有一个服务器配置',
      icon: 'i-heroicons-information-circle',
      color: 'blue'
    })
    return
  }
  
  // 按顺序切换到下一个服务器
  currentServerIndex.value = (currentServerIndex.value + 1) % servers.value.length
  const nextServer = servers.value[currentServerIndex.value]
  
  // 先停止当前服务器
  if (runningServerId.value) {
    await stopProxy()
  }
  
  // 启动新服务器
  await startProxy(nextServer.id)
}

// 网络监控定时器
let networkTimer: NodeJS.Timeout | null = null
let uptimeTimer: NodeJS.Timeout | null = null

/**
 * 加载服务器列表
 */
const loadServers = async () => {
  try {
    const serverList = await invoke('get_servers') as any[]
    servers.value = serverList.map(server => ({
      id: server.id,
      name: server.name,
      protocol: server.protocol,
      address: server.address,
      port: server.port,
      status: 'disconnected' as const,
      uuid: server.config?.uuid || '',
      password: server.config?.password || '',
      username: server.config?.username || '',
      security: server.config?.security || 'auto',
      network: server.config?.network || 'tcp',
      path: server.config?.path || '',
      tls: server.config?.tls || false,
      sni: server.config?.sni || '',
      created_at: server.created_at,
      updated_at: server.updated_at
    }))
  } catch (error) {
    console.error('加载服务器列表失败:', error)
  }
}

/**
 * 启动代理服务器
 */
const startProxy = async (serverId: string) => {
  try {
    await invoke('start_proxy', { serverId })
    
    // 更新状态
    runningServerId.value = serverId
    const server = servers.value.find(s => s.id === serverId)
    if (server) {
      server.status = 'connected'
      appState.activeServer = server
      appState.isConnected = true
      
      // 更新当前服务器索引
      currentServerIndex.value = servers.value.findIndex(s => s.id === serverId)
      
      startNetworkMonitoring()
      
      const toast = useToast()
      toast.add({
        title: '连接成功',
        description: `已连接到服务器 "${server.name}"`,
        icon: 'i-heroicons-check-circle',
        color: 'green'
      })
    }
  } catch (error) {
    console.error('启动代理失败:', error)
    
    const toast = useToast()
    toast.add({
      title: '连接失败',
      description: `无法连接到服务器: ${error}`,
      icon: 'i-heroicons-exclamation-triangle',
      color: 'red'
    })
  }
}

/**
 * 停止代理服务器
 */
const stopProxy = async () => {
  try {
    await invoke('stop_proxy')
    
    // 更新状态
    if (runningServerId.value) {
      const server = servers.value.find(s => s.id === runningServerId.value)
      if (server) {
        server.status = 'disconnected'
      }
    }
    
    runningServerId.value = null
    appState.activeServer = null
    appState.isConnected = false
    
    stopNetworkMonitoring()
    
    const toast = useToast()
    toast.add({
      title: '已断开连接',
      description: '代理服务已停止',
      icon: 'i-heroicons-stop-circle',
      color: 'gray'
    })
  } catch (error) {
    console.error('停止代理失败:', error)
    
    const toast = useToast()
    toast.add({
      title: '断开失败',
      description: `无法停止代理服务: ${error}`,
      icon: 'i-heroicons-exclamation-triangle',
      color: 'red'
    })
  }
}

/**
 * 开始网络监控
 */
const startNetworkMonitoring = () => {
  // 模拟网络速度数据
  networkTimer = setInterval(() => {
    appState.uploadSpeed = Math.random() * 1024 * 1024 // 0-1MB/s
    appState.downloadSpeed = Math.random() * 10 * 1024 * 1024 // 0-10MB/s
    appState.totalTraffic += appState.uploadSpeed + appState.downloadSpeed
  }, 1000)
  
  // 运行时间计时器
  uptimeTimer = setInterval(() => {
    appState.uptime += 1
  }, 1000)
}

/**
 * 停止网络监控
 */
const stopNetworkMonitoring = () => {
  if (networkTimer) {
    clearInterval(networkTimer)
    networkTimer = null
  }
  if (uptimeTimer) {
    clearInterval(uptimeTimer)
    uptimeTimer = null
  }
  
  appState.uploadSpeed = 0
  appState.downloadSpeed = 0
  appState.uptime = 0
}

/**
 * 检查 Xray Core 是否存在
 */
const checkXrayCore = async () => {
  try {
    const exists = await invoke('check_xray_exists') as boolean
    const xrayPath = await invoke('get_xray_path') as string
    
    if (!exists) {
      // 显示警告通知
      const toast = useToast()
      toast.add({
        id: 'xray-warning',
        title: 'Xray Core 未找到',
        description: `在路径 "${xrayPath}" 下未找到 xray-core 可执行文件。请在设置中配置正确的 Xray Core 路径。`,
        icon: 'i-heroicons-exclamation-triangle',
        color: 'orange',
        timeout: 0, // 不自动消失
        actions: [{
          label: '打开设置',
          click: () => {
            // TODO: 打开设置对话框
            console.log('打开设置')
          }
        }, {
          label: '忽略',
          click: () => {
            toast.remove('xray-warning')
          }
        }]
      })
    }
  } catch (error) {
    console.error('检查 Xray Core 失败:', error)
  }
}

/**
 * 初始化代理状态
 */
const initializeProxyStatus = async () => {
  try {
    const status = await invoke('get_proxy_status') as any
    if (status.is_running && status.current_server) {
      runningServerId.value = status.current_server
      
      // 更新对应服务器的状态
      const server = servers.value.find(s => s.id === status.current_server)
      if (server) {
        server.status = 'connected'
        appState.activeServer = server
        appState.isConnected = true
        currentServerIndex.value = servers.value.findIndex(s => s.id === status.current_server)
        startNetworkMonitoring()
      }
    }
  } catch (error) {
    console.error('获取代理状态失败:', error)
  }
}

/**
 * 监听代理状态变化事件
 */
const handleProxyStatusChange = (event: any) => {
  const { is_running, current_server } = event.payload
  
  // 更新运行中的服务器ID
  runningServerId.value = is_running ? current_server : null
  
  // 更新所有服务器的状态
  servers.value.forEach(server => {
    if (server.id === current_server && is_running) {
      server.status = 'connected'
      appState.activeServer = server
      appState.isConnected = true
      currentServerIndex.value = servers.value.findIndex(s => s.id === current_server)
    } else {
      server.status = 'disconnected'
      if (server.id === appState.activeServer?.id) {
        appState.activeServer = null
        appState.isConnected = false
      }
    }
  })
  
  console.log('代理状态已更新:', { is_running, current_server })
}

// 初始化应用
onMounted(async () => {
  // 模拟加载时间
  await new Promise(resolve => setTimeout(resolve, 1000))
  
  // 加载服务器列表
  await loadServers()
  
  // 初始化代理状态
  await initializeProxyStatus()
  
  // 监听代理状态变化事件
  try {
    const { listen } = await import('@tauri-apps/api/event')
    await listen('proxy-status-changed', handleProxyStatusChange)
  } catch (error) {
    console.error('监听代理状态变化失败:', error)
  }
  
  // 检查 Xray Core
  await checkXrayCore()
  
  isLoading.value = false
})

// 设置页面标题
useHead({
  title: 'RuRay - Xray Core Desktop Client'
})

// 组件卸载时清理定时器
onUnmounted(() => {
  stopNetworkMonitoring()
})
</script>

<style>
html, body {
  margin: 0;
  padding: 0;
  overflow: hidden;
}
</style>
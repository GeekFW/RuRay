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
import { ref, reactive, onMounted } from 'vue'
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
}

// 应用状态
const isLoading = ref(true)
const isZenMode = ref(false)

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
const toggleConnection = () => {
  appState.isConnected = !appState.isConnected
  
  if (appState.isConnected) {
    // 模拟连接成功，设置默认服务器
    appState.activeServer = {
      id: '1',
      name: '测试服务器',
      address: '127.0.0.1',
      port: 10086,
      protocol: 'vmess',
      ping: 45,
      isActive: true
    }
    startNetworkMonitoring()
  } else {
    // 断开连接
    appState.activeServer = null
    stopNetworkMonitoring()
  }
}

/**
 * 切换服务器
 */
const switchServer = () => {
  // TODO: 实现服务器切换逻辑
  console.log('切换服务器')
}

// 网络监控定时器
let networkTimer: NodeJS.Timeout | null = null
let uptimeTimer: NodeJS.Timeout | null = null

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

// 初始化应用
onMounted(async () => {
  // 模拟初始化过程
  await new Promise(resolve => setTimeout(resolve, 2000))
  isLoading.value = false
  
  // 检查 Xray Core
  await checkXrayCore()
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
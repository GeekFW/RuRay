<!--
Project: RuRay
Author: Lander
CreateAt: 2024-12-19
-->
<template>
  <div class="fixed inset-0 bg-gradient-to-br from-gray-900 via-gray-800 to-gray-900 flex items-center justify-center z-50">
    <!-- 主容器 -->
    <div class="zen-container bg-black/40 backdrop-blur-xl rounded-2xl p-8 max-w-lg w-full mx-4 border border-white/10">
      <!-- 头部 -->
      <div class="flex items-center justify-between mb-8">
        <div class="flex items-center space-x-3">
          <div class="w-10 h-10 bg-gradient-to-br from-green-400 to-green-600 rounded-xl flex items-center justify-center shadow-lg">
            <Icon name="heroicons:bolt" class="w-6 h-6 text-white" />
          </div>
          <div>
            <h2 class="text-xl font-bold text-white">RuRay</h2>
            <p class="text-xs text-gray-400">极简模式</p>
          </div>
        </div>
        
        <UButton
          variant="ghost"
          size="sm"
          @click="$emit('close')"
          class="text-white hover:bg-white/10 rounded-full p-2"
        >
          <Icon name="heroicons:x-mark" class="w-5 h-5" />
        </UButton>
      </div>
      
      <!-- 连接状态指示器 -->
      <div class="text-center mb-8">
        <div class="relative inline-flex items-center justify-center mb-4">
          <div :class="[
            'w-20 h-20 rounded-full flex items-center justify-center transition-all duration-300',
            isConnected ? 'bg-green-500/20 border-2 border-green-400' : 'bg-red-500/20 border-2 border-red-400'
          ]">
            <Icon 
              :name="isConnected ? 'heroicons:check-circle' : 'heroicons:x-circle'" 
              :class="[
                'w-10 h-10 transition-colors duration-300',
                isConnected ? 'text-green-400' : 'text-red-400'
              ]" 
            />
          </div>
          <!-- 连接状态动画环 -->
          <div v-if="isConnected" class="absolute inset-0 rounded-full border-2 border-green-400 animate-ping opacity-20"></div>
        </div>
        
        <h3 class="text-2xl font-bold text-white mb-2">
          {{ isConnected ? '已连接' : '未连接' }}
        </h3>
        
        <div v-if="activeServer" class="text-gray-300">
          <p class="font-medium">{{ activeServer.name }}</p>
          <p class="text-sm text-gray-400">{{ activeServer.address }}:{{ activeServer.port }}</p>
          <div v-if="activeServer.ping" class="flex items-center justify-center space-x-1 mt-1">
            <div :class="[
              'w-2 h-2 rounded-full',
              getPingColor(activeServer.ping)
            ]"></div>
            <span class="text-xs">{{ activeServer.ping }}ms</span>
          </div>
        </div>
      </div>
      
      <!-- 实时网络速率 -->
      <div class="grid grid-cols-2 gap-6 mb-8">
        <div class="text-center">
          <div class="flex items-center justify-center space-x-2 mb-2">
            <Icon name="heroicons:arrow-up" class="w-5 h-5 text-green-400" />
            <span class="text-sm text-gray-300 font-medium">上传</span>
          </div>
          <div class="text-2xl font-bold text-green-400 font-mono">
            {{ formatSpeed(uploadSpeed) }}
          </div>
        </div>
        
        <div class="text-center">
          <div class="flex items-center justify-center space-x-2 mb-2">
            <Icon name="heroicons:arrow-down" class="w-5 h-5 text-blue-400" />
            <span class="text-sm text-gray-300 font-medium">下载</span>
          </div>
          <div class="text-2xl font-bold text-blue-400 font-mono">
            {{ formatSpeed(downloadSpeed) }}
          </div>
        </div>
      </div>
      
      <!-- 流量统计 -->
      <div class="bg-white/5 rounded-xl p-4 mb-6">
        <div class="grid grid-cols-3 gap-4 text-center">
          <div>
            <p class="text-xs text-gray-400 mb-1">总流量</p>
            <p class="text-lg font-bold text-white">{{ formatBytes(totalTraffic) }}</p>
          </div>
          <div>
            <p class="text-xs text-gray-400 mb-1">运行时间</p>
            <p class="text-lg font-bold text-white">{{ formatDuration(uptime) }}</p>
          </div>
          <div>
            <p class="text-xs text-gray-400 mb-1">代理模式</p>
            <UBadge 
              :color="getProxyModeColor(proxyMode)" 
              variant="soft" 
              size="sm"
              class="font-medium"
            >
              {{ getProxyModeText(proxyMode) }}
            </UBadge>
          </div>
        </div>
      </div>
      
      <!-- 快速操作按钮 -->
      <div class="grid grid-cols-2 gap-4">
        <UButton
          :color="isConnected ? 'red' : 'green'"
          variant="solid"
          size="lg"
          block
          @click="$emit('toggleConnection')"
          class="font-medium"
        >
          <Icon 
            :name="isConnected ? 'heroicons:stop' : 'heroicons:play'" 
            class="w-5 h-5 mr-2" 
          />
          {{ isConnected ? '断开连接' : '开始连接' }}
        </UButton>
        
        <UButton
          variant="outline"
          size="lg"
          block
          @click="$emit('switchServer')"
          class="border-white/20 text-white hover:bg-white/10 font-medium"
        >
          <Icon name="heroicons:arrow-path" class="w-5 h-5 mr-2" />
          切换服务器
        </UButton>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

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

/**
 * 组件属性
 */
const props = withDefaults(defineProps<{
  /** 是否已连接 */
  isConnected?: boolean
  /** 当前激活的服务器 */
  activeServer?: Server | null
  /** 上传速度 (bytes/s) */
  uploadSpeed?: number
  /** 下载速度 (bytes/s) */
  downloadSpeed?: number
  /** 总流量 (bytes) */
  totalTraffic?: number
  /** 运行时间 (seconds) */
  uptime?: number
  /** 代理模式 */
  proxyMode?: 'global' | 'pac' | 'direct'
}>(), {
  isConnected: false,
  activeServer: null,
  uploadSpeed: 0,
  downloadSpeed: 0,
  totalTraffic: 0,
  uptime: 0,
  proxyMode: 'global'
})

/**
 * 组件事件
 */
defineEmits<{
  /** 关闭极简模式 */
  close: []
  /** 切换连接状态 */
  toggleConnection: []
  /** 切换服务器 */
  switchServer: []
}>()

/**
 * 格式化网络速度
 * @param speed 速度 (bytes/s)
 * @returns 格式化后的速度字符串
 */
const formatSpeed = (speed: number): string => {
  if (speed === 0) return '0 B/s'
  
  const units = ['B/s', 'KB/s', 'MB/s', 'GB/s']
  const k = 1024
  const i = Math.floor(Math.log(speed) / Math.log(k))
  
  return `${(speed / Math.pow(k, i)).toFixed(1)} ${units[i]}`
}

/**
 * 格式化字节数
 * @param bytes 字节数
 * @returns 格式化后的字节字符串
 */
const formatBytes = (bytes: number): string => {
  if (bytes === 0) return '0 B'
  
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const k = 1024
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  
  return `${(bytes / Math.pow(k, i)).toFixed(1)} ${units[i]}`
}

/**
 * 格式化持续时间
 * @param seconds 秒数
 * @returns 格式化后的时间字符串
 */
const formatDuration = (seconds: number): string => {
  if (seconds < 60) return `${seconds}s`
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h`
  return `${Math.floor(seconds / 86400)}d`
}

/**
 * 获取延迟颜色
 * @param ping 延迟值
 * @returns CSS 类名
 */
const getPingColor = (ping: number): string => {
  if (ping < 100) return 'bg-green-400'
  if (ping < 300) return 'bg-yellow-400'
  return 'bg-red-400'
}

/**
 * 获取代理模式颜色
 * @param mode 代理模式
 * @returns 颜色名称
 */
const getProxyModeColor = (mode: string): string => {
  switch (mode) {
    case 'global': return 'green'
    case 'pac': return 'blue'
    case 'direct': return 'gray'
    default: return 'gray'
  }
}

/**
 * 获取代理模式文本
 * @param mode 代理模式
 * @returns 显示文本
 */
const getProxyModeText = (mode: string): string => {
  switch (mode) {
    case 'global': return '全局'
    case 'pac': return 'PAC'
    case 'direct': return '直连'
    default: return '未知'
  }
}
</script>

<style scoped>
.zen-container {
  box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.5);
}

/* 数字字体优化 */
.font-mono {
  font-family: 'SF Mono', 'Monaco', 'Inconsolata', 'Roboto Mono', monospace;
}

/* 动画效果 */
@keyframes pulse-glow {
  0%, 100% {
    box-shadow: 0 0 20px rgba(34, 197, 94, 0.3);
  }
  50% {
    box-shadow: 0 0 30px rgba(34, 197, 94, 0.6);
  }
}

.zen-container:hover {
  animation: pulse-glow 2s ease-in-out infinite;
}
</style>
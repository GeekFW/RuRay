<template>
  <div class="h-screen flex flex-col bg-white dark:bg-gray-800">
    <!-- 头部 -->
    <div class="p-4 border-b border-gray-200 dark:border-gray-700 drag-region">
      <div class="flex items-center justify-between">
        <h1 class="text-lg font-semibold text-gray-900 dark:text-white no-drag">
          {{ $t('advancedLogViewer.title') }}
        </h1>
        
        <div class="flex items-center space-x-2 no-drag">
          <!-- 连接状态 -->
          <UBadge
            :color="isConnected ? 'green' : 'red'"
            variant="soft"
            size="sm"
          >
            {{ isConnected ? $t('advancedLogViewer.connected') : $t('advancedLogViewer.disconnected') }}
          </UBadge>
          
          <!-- 暂停/继续 -->
          <UButton
            variant="ghost"
            size="sm"
            @click="togglePause"
            :color="isPaused ? 'orange' : 'green'"
          >
            <Icon :name="isPaused ? 'heroicons:play' : 'heroicons:pause'" class="w-4 h-4 mr-1" />
            {{ isPaused ? $t('common.resume') : $t('common.pause') }}
          </UButton>
          
          <!-- 清空 -->
          <UButton
            variant="ghost"
            size="sm"
            @click="clearLogs"
            color="red"
          >
            <Icon name="heroicons:trash" class="w-4 h-4 mr-1" />
            {{ $t('common.clear') }}
          </UButton>
          
          <!-- 关闭窗口 -->
          <UButton
            variant="ghost"
            size="sm"
            @click="closeWindow"
            color="gray"
          >
            <Icon name="heroicons:x-mark" class="w-4 h-4" />
          </UButton>
        </div>
      </div>
    </div>
    
    <!-- 日志内容 -->
    <div 
      ref="logContainer"
      class="flex-1 overflow-y-auto p-4 font-mono text-sm bg-gray-50 dark:bg-gray-900"
    >
      <div v-if="logs.length === 0" class="text-center py-12">
        <Icon name="heroicons:document-text" class="w-12 h-12 text-gray-400 mx-auto mb-4" />
        <p class="text-gray-500 dark:text-gray-400">
          {{ isConnected ? $t('advancedLogViewer.waitingForLogs') : $t('advancedLogViewer.notConnected') }}
        </p>
      </div>
      
      <div v-else class="space-y-1">
        <div
          v-for="(log, index) in logs"
          :key="index"
          class="p-2 rounded border-l-4 border-blue-300 bg-blue-50 dark:border-blue-600 dark:bg-blue-900/20"
        >
          <div class="flex items-start space-x-3">
            <!-- 时间戳 -->
            <span class="text-xs text-gray-500 dark:text-gray-400 whitespace-nowrap">
              {{ formatTime(log.timestamp) }}
            </span>
            
            <!-- 日志内容 -->
            <div class="flex-1 min-w-0">
              <p class="text-gray-900 dark:text-gray-100 break-words whitespace-pre-wrap">
                {{ log.message }}
              </p>
            </div>
          </div>
        </div>
      </div>
    </div>
    
    <!-- 底部状态 -->
    <div class="p-2 border-t border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900">
      <div class="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
        <span>{{ $t('advancedLogViewer.totalLogs', { count: logs.length }) }}</span>
        <span v-if="isPaused" class="text-orange-500">
          {{ $t('advancedLogViewer.paused') }}
        </span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, nextTick, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'

// 国际化 - 使用 Nuxt 的自动导入
const { t: $t } = useI18n()

// 接口定义
interface XrayLogEntry {
  timestamp: string
  message: string
}

// 状态
const logContainer = ref<HTMLElement>()
const logs = ref<XrayLogEntry[]>([])
const isConnected = ref(false)
const isPaused = ref(false)

// 定时器
let logFetchInterval: NodeJS.Timeout | null = null

// 方法
const formatTime = (timestamp: string) => {
  try {
    const date = new Date(timestamp)
    return date.toLocaleTimeString('zh-CN', {
      hour12: false,
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
      fractionalSecondDigits: 3
    })
  } catch {
    return timestamp
  }
}

const scrollToBottom = () => {
  if (logContainer.value) {
    nextTick(() => {
      logContainer.value!.scrollTop = logContainer.value!.scrollHeight
    })
  }
}

const fetchXrayLogs = async () => {
  if (isPaused.value) return
  
  try {
    // 检查日志流是否可用
    const isAvailable = await invoke('is_log_stream_available') as boolean
    if (!isAvailable) {
      isConnected.value = false
      return
    }
    
    // 获取日志流缓冲区
    const newLogs = await invoke('get_log_stream_buffer') as any[]
    if (newLogs && newLogs.length > 0) {
      // 转换日志格式
      const formattedLogs = newLogs.map(log => ({
        timestamp: log.timestamp,
        message: log.message
      })) as XrayLogEntry[]
      
      // 只添加新的日志条目（避免重复）
      const currentLogCount = logs.value.length
      if (formattedLogs.length > currentLogCount) {
        const newEntries = formattedLogs.slice(currentLogCount)
        logs.value.push(...newEntries)
        
        // 限制日志数量，避免内存溢出
        if (logs.value.length > 5000) {
          logs.value = logs.value.slice(-2500)
        }
        
        scrollToBottom()
      }
    }
    
    isConnected.value = true
  } catch (error) {
    console.error('Failed to fetch Xray logs:', error)
    isConnected.value = false
  }
}

const togglePause = () => {
  isPaused.value = !isPaused.value
}

const clearLogs = async () => {
  logs.value = []
  // 清空后端日志流缓冲区
  try {
    await invoke('clear_log_stream_buffer')
  } catch (error) {
    console.error('Failed to clear log stream buffer:', error)
  }
}

const closeWindow = async () => {
  try {
    const currentWindow = getCurrentWindow()
    await currentWindow.close()
  } catch (error) {
    console.error('Failed to close window:', error)
  }
}

// 生命周期钩子
onMounted(async () => {
  // 初始获取日志
  await fetchXrayLogs()
  
  // 每500ms获取一次新日志，实现实时更新
  logFetchInterval = setInterval(fetchXrayLogs, 500)
})

onUnmounted(() => {
  if (logFetchInterval) {
    clearInterval(logFetchInterval)
  }
})
</script>

<style scoped>
/* 确保窗口可拖拽 */
.drag-region {
  -webkit-app-region: drag;
}

.no-drag {
  -webkit-app-region: no-drag;
}
</style>
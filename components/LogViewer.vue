<template>
  <div class="h-full flex flex-col bg-white dark:bg-gray-800">
    <!-- 头部 -->
    <div class="p-4 border-b border-gray-200 dark:border-gray-700">
      <div class="flex items-center justify-between">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">{{ $t('logViewer.title') }}</h2>
        
        <div class="flex items-center space-x-2">
          <!-- 日志级别过滤 -->
          <USelectMenu
            class="w-24"
            v-model="logLevel"
            value-attribute="value"
            :options="logLevelOptions"
            size="sm"
          />
          
          <!-- 自动滚动 -->
          <UButton
            variant="ghost"
            size="sm"
            @click="autoScroll = !autoScroll"
            :color="autoScroll ? selectedThemeColor : 'gray'"
          >
            <Icon name="heroicons:arrow-down" class="w-4 h-4 mr-1" />
            {{ $t('logViewer.autoScroll') }}
          </UButton>
          
          <!-- 清空日志 -->
          <UButton
            variant="ghost"
            size="sm"
            @click="clearLogs"
            :color="selectedThemeColor"
          >
            <Icon name="heroicons:trash" class="w-4 h-4 mr-1" />
            {{ $t('common.clear') }}
          </UButton>
          
          <!-- 导出日志 -->
          <UButton
            variant="ghost"
            size="sm"
            @click="exportLogs"
            :color="selectedThemeColor"
          >
            <Icon name="heroicons:arrow-down-tray" class="w-4 h-4 mr-1" />
            {{ $t('common.export') }}
          </UButton>
          
          <!-- 高级日志查看器 -->
          <UButton
            variant="ghost"
            size="sm"
            @click="openAdvancedLogViewer"
            :color="selectedThemeColor"
          >
            <Icon name="heroicons:rectangle-stack" class="w-4 h-4 mr-1" />
            {{ $t('logViewer.advancedViewer') }}
          </UButton>
        </div>
      </div>
    </div>
    
    <!-- 日志内容 -->
    <div 
      ref="logContainer"
      class="flex-1 overflow-y-auto p-4 font-mono text-sm bg-gray-50 dark:bg-gray-900"
    >
      <div v-if="filteredLogs.length === 0" class="text-center py-12">
        <Icon name="heroicons:document-text" class="w-12 h-12 text-gray-400 mx-auto mb-4" />
        <p class="text-gray-500 dark:text-gray-400">{{ $t('logViewer.noLogs') }}</p>
      </div>
      
      <div v-else class="space-y-1">
        <div
          v-for="(log, index) in filteredLogs"
          :key="index"
          :class="[
            'p-2 rounded border-l-4 transition-colors',
            getLogLevelClass(log.level)
          ]"
        >
          <div class="flex items-start space-x-3">
            <!-- 时间戳 -->
            <span class="text-xs text-gray-500 dark:text-gray-400 whitespace-nowrap">
              {{ formatTime(log.timestamp) }}
            </span>
            
            <!-- 日志级别 -->
            <UBadge
              :color="getLogLevelColor(log.level)"
              variant="soft"
              size="xs"
              class="whitespace-nowrap"
            >
              {{ log.level.toUpperCase() }}
            </UBadge>
            
            <!-- 日志内容 -->
            <div class="flex-1 min-w-0">
              <p class="text-gray-900 dark:text-gray-100 break-words">
                {{ log.message }}
              </p>
              
              <!-- 详细信息 -->
              <div v-if="log.details" class="mt-1 text-xs text-gray-600 dark:text-gray-400">
                {{ log.details }}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
    
    <!-- 底部状态 -->
    <div class="p-2 border-t border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900">
      <div class="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
        <span>{{ $t('logViewer.totalLogs', { count: logs.length }) }}</span>
        <span v-if="filteredLogs.length !== logs.length">
          {{ $t('logViewer.showingLogs', { count: filteredLogs.length }) }}
        </span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, nextTick, watch, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from 'vue-i18n'

// 国际化
const { t: $t } = useI18n()

// 获取应用配置以访问主题色
const appConfig = useAppConfig()
const selectedThemeColor = computed(() => appConfig.ui?.primary || 'green')

// 接口定义
interface LogEntry {
  timestamp: string
  level: 'debug' | 'info' | 'warn' | 'error'
  message: string
  details?: string
}

// 状态
const logContainer = ref<HTMLElement>()
const logs = ref<LogEntry[]>([])

const logLevel = ref('info')
const autoScroll = ref(true)

// 选项
const logLevelOptions = computed(() => [
  { label: $t('logViewer.levels.all'), value: 'debug' },
  { label: $t('logViewer.levels.info'), value: 'info' },
  { label: $t('logViewer.levels.warn'), value: 'warn' },
  { label: $t('logViewer.levels.error'), value: 'error' }
])

// 计算属性
const filteredLogs = computed(() => {
  const levels = ['debug', 'info', 'warn', 'error']
  const minLevelIndex = levels.indexOf(logLevel.value)
  
  return logs.value.filter(log => {
    const logLevelIndex = levels.indexOf(log.level)
    return logLevelIndex >= minLevelIndex
  })
})

// 方法
const getLogLevelClass = (level: string) => {
  const classes = {
    debug: 'border-gray-300 bg-gray-50 dark:border-gray-600 dark:bg-gray-800',
    info: 'border-blue-300 bg-blue-50 dark:border-blue-600 dark:bg-blue-900/20',
    warn: 'border-yellow-300 bg-yellow-50 dark:border-yellow-600 dark:bg-yellow-900/20',
    error: 'border-red-300 bg-red-50 dark:border-red-600 dark:bg-red-900/20'
  }
  return classes[level as keyof typeof classes] || classes.info
}

const getLogLevelColor = (level: string) => {
  const colors = {
    debug: 'gray',
    info: 'blue',
    warn: 'yellow',
    error: 'red'
  }
  return colors[level as keyof typeof colors] || 'blue'
}

const formatTime = (timestamp: string) => {
  try {
    const date = new Date(timestamp)
    return date.toLocaleTimeString('zh-CN', {
      hour12: false,
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit'
    })
  } catch {
    return timestamp
  }
}

const scrollToBottom = () => {
  if (autoScroll.value && logContainer.value) {
    nextTick(() => {
      logContainer.value!.scrollTop = logContainer.value!.scrollHeight
    })
  }
}

// 从后端获取日志
const fetchLogs = async () => {
  try {
    const backendLogs = await invoke('get_logs', { limit: 1000 }) as LogEntry[]
    logs.value = backendLogs
    // 只有在开启自动滚动时才滚动到底部
    if (autoScroll.value) {
      scrollToBottom()
    }
  } catch (error) {
    console.error($t('logViewer.fetchLogsFailed'), error)
  }
}

const clearLogs = () => {
  logs.value = []
}

const exportLogs = async () => {
  try {
    const { save } = await import('@tauri-apps/plugin-dialog')
    const { writeTextFile } = await import('@tauri-apps/plugin-fs')
    
    const logContent = logs.value
      .map(log => `[${formatTime(log.timestamp)}] [${log.level.toUpperCase()}] ${log.message}${log.details ? '\n  ' + log.details : ''}`)
      .join('\n')
    
    const filePath = await save({
      filters: [{
        name: 'Log Files',
        extensions: ['log', 'txt']
      }],
      defaultPath: `ruray-logs-${new Date().toISOString().split('T')[0]}.log`
    })
    
    if (filePath) {
      await writeTextFile(filePath, logContent)
      // TODO: 显示成功通知
    }
  } catch (error) {
    console.error($t('logViewer.exportLogsFailed'), error)
    // TODO: 显示错误通知
  }
}

// 打开高级日志查看器窗口
const openAdvancedLogViewer = async () => {
  try {
    await invoke('open_advanced_log_window')
  } catch (error) {
    console.error('Failed to open advanced log viewer:', error)
  }
}

// 添加新日志的方法（保留用于兼容性）
const addLog = (level: LogEntry['level'], message: string, details?: string) => {
  const newLog: LogEntry = {
    timestamp: new Date().toISOString(),
    level,
    message,
    details
  }
  
  logs.value.push(newLog)
  
  // 限制日志数量，避免内存溢出
  if (logs.value.length > 1000) {
    logs.value = logs.value.slice(-500)
  }
  
  scrollToBottom()
}

// 监听日志变化，自动滚动
watch(() => logs.value.length, () => {
  if (autoScroll.value) {
    scrollToBottom()
  }
})

// 定时获取日志的定时器
let logUpdateInterval: NodeJS.Timeout | null = null

// 生命周期钩子
onMounted(async () => {
  // 初始加载日志
  await fetchLogs()
  
  // 每5秒更新一次日志
  logUpdateInterval = setInterval(fetchLogs, 5000)
})

onUnmounted(() => {
  if (logUpdateInterval) {
    clearInterval(logUpdateInterval)
  }
})

// 暴露方法给父组件
defineExpose({
  addLog
})


</script>
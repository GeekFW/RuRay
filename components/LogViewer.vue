<template>
  <div class="h-full flex flex-col bg-white dark:bg-gray-800">
    <!-- 头部 -->
    <div class="p-4 border-b border-gray-200 dark:border-gray-700">
      <div class="flex items-center justify-between">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">运行日志</h2>
        
        <div class="flex items-center space-x-2">
          <!-- 日志级别过滤 -->
          <USelectMenu
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
            :color="autoScroll ? 'green' : 'gray'"
          >
            <Icon name="heroicons:arrow-down" class="w-4 h-4 mr-1" />
            自动滚动
          </UButton>
          
          <!-- 清空日志 -->
          <UButton
            variant="ghost"
            size="sm"
            @click="clearLogs"
          >
            <Icon name="heroicons:trash" class="w-4 h-4 mr-1" />
            清空
          </UButton>
          
          <!-- 导出日志 -->
          <UButton
            variant="ghost"
            size="sm"
            @click="exportLogs"
          >
            <Icon name="heroicons:arrow-down-tray" class="w-4 h-4 mr-1" />
            导出
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
        <p class="text-gray-500 dark:text-gray-400">暂无日志信息</p>
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
        <span>共 {{ logs.length }} 条日志</span>
        <span v-if="filteredLogs.length !== logs.length">
          显示 {{ filteredLogs.length }} 条
        </span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, nextTick, watch } from 'vue'

// 接口定义
interface LogEntry {
  timestamp: Date
  level: 'debug' | 'info' | 'warn' | 'error'
  message: string
  details?: string
}

// 状态
const logContainer = ref<HTMLElement>()
const logs = ref<LogEntry[]>([
  {
    timestamp: new Date(),
    level: 'info',
    message: 'RuRay 启动成功',
    details: '版本: 1.0.0'
  },
  {
    timestamp: new Date(Date.now() - 1000),
    level: 'info',
    message: '正在初始化 Xray Core...'
  },
  {
    timestamp: new Date(Date.now() - 2000),
    level: 'debug',
    message: '加载配置文件: config.json'
  }
])

const logLevel = ref('info')
const autoScroll = ref(true)

// 选项
const logLevelOptions = [
  { label: '全部', value: 'debug' },
  { label: '信息', value: 'info' },
  { label: '警告', value: 'warn' },
  { label: '错误', value: 'error' }
]

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

const formatTime = (timestamp: Date) => {
  return timestamp.toLocaleTimeString('zh-CN', {
    hour12: false,
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit'
  })
}

const scrollToBottom = () => {
  if (autoScroll.value && logContainer.value) {
    nextTick(() => {
      logContainer.value!.scrollTop = logContainer.value!.scrollHeight
    })
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
    console.error('导出日志失败:', error)
    // TODO: 显示错误通知
  }
}

// 添加新日志的方法
const addLog = (level: LogEntry['level'], message: string, details?: string) => {
  logs.value.push({
    timestamp: new Date(),
    level,
    message,
    details
  })
  
  // 限制日志数量，避免内存溢出
  if (logs.value.length > 1000) {
    logs.value = logs.value.slice(-500)
  }
  
  scrollToBottom()
}

// 监听日志变化，自动滚动
watch(() => logs.value.length, () => {
  scrollToBottom()
})

// 暴露方法给父组件
defineExpose({
  addLog
})

// 模拟日志生成（开发时使用）
if (process.dev) {
  const simulateLogs = () => {
    const messages = [
      { level: 'info', message: '连接服务器成功' },
      { level: 'debug', message: '发送心跳包' },
      { level: 'warn', message: '连接延迟较高', details: '当前延迟: 500ms' },
      { level: 'error', message: '连接失败', details: '网络超时' },
      { level: 'info', message: '重新连接中...' }
    ]
    
    setInterval(() => {
      const randomMessage = messages[Math.floor(Math.random() * messages.length)]
      addLog(
        randomMessage.level as LogEntry['level'],
        randomMessage.message,
        randomMessage.details
      )
    }, 3000)
  }
  
  setTimeout(simulateLogs, 2000)
}
</script>
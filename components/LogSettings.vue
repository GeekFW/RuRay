<template>
  <div class="h-full flex flex-col bg-white dark:bg-gray-800">
    <!-- 头部 -->
    <div class="p-4 border-b border-gray-200 dark:border-gray-700">
      <div class="flex items-center justify-between mb-3">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">日志设置</h2>
        <UButton
          size="sm"
          @click="refreshLogInfo"
          :color="selectedThemeColor"
          variant="outline"
          :loading="loading"
        >
          <Icon name="heroicons:arrow-path" class="w-4 h-4 mr-1" />
          刷新
        </UButton>
      </div>
    </div>
    
    <!-- 配置内容 -->
    <div class="flex-1 overflow-y-auto p-4">
      <div class="space-y-6">
        <!-- 日志文件信息 -->
        <UCard>
          <template #header>
            <h3 class="text-base font-semibold text-gray-900 dark:text-white">日志文件信息</h3>
          </template>
          
          <div class="space-y-4">
            <!-- 日志文件路径 -->
            <UFormGroup label="日志文件路径" name="logPath" help="当前日志文件的存储路径">
              <div class="flex items-center space-x-2">
                <UInput
                  v-model="logPath"
                  readonly
                  class="flex-1"
                  placeholder="获取中..."
                />
                <UButton
                  size="sm"
                  @click="openLogDirectory"
                  :color="selectedThemeColor"
                  variant="outline"
                  :disabled="!logPath"
                >
                  <Icon name="heroicons:folder-open" class="w-4 h-4 mr-1" />
                  浏览
                </UButton>
              </div>
            </UFormGroup>
            
            <!-- 日志文件大小 -->
            <UFormGroup label="文件大小" name="logSize" help="当前日志文件的大小">
              <UInput
                v-model="logSizeDisplay"
                readonly
                placeholder="计算中..."
              />
            </UFormGroup>
            
            <!-- 日志条目数量 -->
            <UFormGroup label="日志条目" name="logCount" help="当前日志文件中的条目数量">
              <UInput
                v-model="logCountDisplay"
                readonly
                placeholder="统计中..."
              />
            </UFormGroup>
          </div>
        </UCard>
        
        <!-- 日志管理操作 -->
        <UCard>
          <template #header>
            <h3 class="text-base font-semibold text-gray-900 dark:text-white">日志管理</h3>
          </template>
          
          <div class="space-y-4">
            <!-- 清理日志 -->
            <div class="flex items-center justify-between p-4 bg-yellow-50 dark:bg-yellow-900/20 rounded-lg border border-yellow-200 dark:border-yellow-800">
              <div>
                <h4 class="text-sm font-medium text-yellow-800 dark:text-yellow-200">清理日志文件</h4>
                <p class="text-xs text-yellow-600 dark:text-yellow-400 mt-1">
                  清空现有的日志内容，不影响后续日志写入
                </p>
              </div>
              <UButton
                @click="showClearConfirm = true"
                :color="selectedThemeColor"
                variant="outline"
                size="sm"
                :disabled="!logPath || logSize === 0"
              >
                <Icon name="heroicons:trash" class="w-4 h-4 mr-1" />
                清理日志
              </UButton>
            </div>
            
            <!-- 导出日志 -->
            <div class="flex items-center justify-between p-4 bg-blue-50 dark:bg-blue-900/20 rounded-lg border border-blue-200 dark:border-blue-800">
              <div>
                <h4 class="text-sm font-medium text-blue-800 dark:text-blue-200">导出日志文件</h4>
                <p class="text-xs text-blue-600 dark:text-blue-400 mt-1">
                  将当前日志文件导出到指定位置
                </p>
              </div>
              <UButton
                @click="exportLogFile"
                :color="selectedThemeColor"
                variant="outline"
                size="sm"
                :disabled="!logPath || logSize === 0"
                :loading="exporting"
              >
                <Icon name="heroicons:arrow-down-tray" class="w-4 h-4 mr-1" />
                导出日志
              </UButton>
            </div>
          </div>
        </UCard>
      </div>
    </div>
    
    <!-- 清理确认对话框 -->
    <UModal v-model="showClearConfirm">
      <UCard>
        <template #header>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">确认清理日志</h3>
        </template>
        
        <div class="space-y-4">
          <div class="flex items-start space-x-3">
            <Icon name="heroicons:exclamation-triangle" class="w-6 h-6 text-yellow-500 flex-shrink-0 mt-0.5" />
            <div>
              <p class="text-gray-900 dark:text-white font-medium">您确定要清理日志文件吗？</p>
              <p class="text-sm text-gray-600 dark:text-gray-400 mt-1">
                此操作将清空所有现有的日志内容，但不会影响后续的日志写入。此操作不可撤销。
              </p>
            </div>
          </div>
          
          <div class="bg-gray-50 dark:bg-gray-800 p-3 rounded-lg">
            <div class="text-sm text-gray-600 dark:text-gray-400">
              <p><strong>文件路径：</strong>{{ logPath }}</p>
              <p><strong>当前大小：</strong>{{ logSizeDisplay }}</p>
              <p><strong>日志条目：</strong>{{ logCountDisplay }}</p>
            </div>
          </div>
        </div>
        
        <template #footer>
          <div class="flex justify-end space-x-2">
            <UButton
              variant="ghost"
              @click="showClearConfirm = false"
              :disabled="clearing"
            >
              取消
            </UButton>
            <UButton
              :color="selectedThemeColor"
              @click="clearLogFile"
              :loading="clearing"
            >
              确认清理
            </UButton>
          </div>
        </template>
      </UCard>
    </UModal>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { save } from '@tauri-apps/plugin-dialog'
import { copyFile } from '@tauri-apps/plugin-fs'

// Toast 通知
const toast = useToast()

// 获取应用配置以访问主题色
const appConfig = useAppConfig()
const selectedThemeColor = computed(() => appConfig.ui?.primary || 'green')

// 响应式数据
const loading = ref(false)
const clearing = ref(false)
const exporting = ref(false)
const showClearConfirm = ref(false)

const logPath = ref('')
const logSize = ref(0)
const logCount = ref(0)

// 计算属性
const logSizeDisplay = computed(() => {
  if (logSize.value === 0) return '0 B'
  
  const units = ['B', 'KB', 'MB', 'GB']
  let size = logSize.value
  let unitIndex = 0
  
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024
    unitIndex++
  }
  
  return `${size.toFixed(1)} ${units[unitIndex]}`
})

const logCountDisplay = computed(() => {
  return logCount.value.toLocaleString() + ' 条'
})

// 方法
/**
 * 获取日志文件信息
 */
const getLogInfo = async () => {
  try {
    const result = await invoke('get_log_info') as {
      path: string
      size: number
      count: number
    }
    
    logPath.value = result.path
    logSize.value = result.size
    logCount.value = result.count
  } catch (error) {
    console.error('获取日志信息失败:', error)
    toast.add({
      title: '获取日志信息失败',
      description: error instanceof Error ? error.message : '未知错误',
      color: 'red'
    })
  }
}

/**
 * 刷新日志信息
 */
const refreshLogInfo = async () => {
  loading.value = true
  try {
    await getLogInfo()
    toast.add({
      title: '刷新成功',
      description: '日志信息已更新',
      color: 'green'
    })
  } finally {
    loading.value = false
  }
}

/**
 * 打开日志文件所在目录
 */
const openLogDirectory = async () => {
  if (!logPath.value) return
  
  try {
    // 获取目录路径
    const dirPath = logPath.value.substring(0, logPath.value.lastIndexOf('\\'))
    // 使用invoke调用后端命令打开目录
    await invoke('open_file_directory', { filePath: dirPath })
  } catch (error) {
    console.error('打开目录失败:', error)
    toast.add({
      title: '打开目录失败',
      description: error instanceof Error ? error.message : '未知错误',
      color: 'red'
    })
  }
}

/**
 * 清理日志文件
 */
const clearLogFile = async () => {
  clearing.value = true
  try {
    await invoke('clear_log_file')
    
    // 更新日志信息
    await getLogInfo()
    
    showClearConfirm.value = false
    toast.add({
      title: '清理成功',
      description: '日志文件已清空',
      color: 'green'
    })
  } catch (error) {
    console.error('清理日志失败:', error)
    toast.add({
      title: '清理日志失败',
      description: error instanceof Error ? error.message : '未知错误',
      color: 'red'
    })
  } finally {
    clearing.value = false
  }
}

/**
 * 导出日志文件
 */
const exportLogFile = async () => {
  if (!logPath.value) return
  
  exporting.value = true
  try {
    // 选择保存位置
    const savePath = await save({
      filters: [{
        name: 'Log Files',
        extensions: ['log', 'txt']
      }],
      defaultPath: `ruray-logs-${new Date().toISOString().split('T')[0]}.log`
    })
    
    if (savePath) {
      // 复制日志文件
      await copyFile(logPath.value, savePath)
      
      toast.add({
        title: '导出成功',
        description: `日志文件已保存到: ${savePath}`,
        color: 'green'
      })
    }
  } catch (error) {
    console.error('导出日志失败:', error)
    toast.add({
      title: '导出日志失败',
      description: error instanceof Error ? error.message : '未知错误',
      color: 'red'
    })
  } finally {
    exporting.value = false
  }
}

// 生命周期钩子
onMounted(async () => {
  await getLogInfo()
})
</script>
<template>
  <div class="h-full flex flex-col bg-white dark:bg-gray-800">
    <!-- 头部 -->
    <div class="p-4 border-b border-gray-200 dark:border-gray-700">
      <div class="flex items-center justify-between mb-3">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">{{ $t('logSettings.title') }}</h2>
        <UButton
          size="sm"
          @click="refreshLogInfo"
          :color="selectedThemeColor"
          variant="outline"
          :loading="loading"
        >
          <Icon name="heroicons:arrow-path" class="w-4 h-4 mr-1" />
          {{ $t('common.refresh') }}
        </UButton>
      </div>
    </div>
    
    <!-- 配置内容 -->
    <div class="flex-1 overflow-y-auto p-4">
      <div class="space-y-6">
        <!-- 日志文件信息 -->
        <UCard>
          <template #header>
            <h3 class="text-base font-semibold text-gray-900 dark:text-white">{{ $t('logSettings.fileInfo.title') }}</h3>
          </template>
          
          <div class="space-y-4">
            <!-- 日志文件路径 -->
            <UFormGroup :label="$t('logSettings.fileInfo.logPath')" name="logPath" :help="$t('logSettings.fileInfo.logPathHelp')">
              <div class="flex items-center space-x-2">
                <UInput
                  v-model="logPath"
                  readonly
                  class="flex-1"
                  :placeholder="$t('logSettings.fileInfo.loading')"
                />
                <UButton
                  size="sm"
                  @click="openLogDirectory"
                  :color="selectedThemeColor"
                  variant="outline"
                  :disabled="!logPath"
                >
                  <Icon name="heroicons:folder-open" class="w-4 h-4 mr-1" />
                  {{ $t('common.browse') }}
                </UButton>
              </div>
            </UFormGroup>
            
            <!-- 日志文件大小 -->
            <UFormGroup :label="$t('logSettings.fileInfo.fileSize')" name="logSize" :help="$t('logSettings.fileInfo.fileSizeHelp')">
              <UInput
                v-model="logSizeDisplay"
                readonly
                :placeholder="$t('logSettings.fileInfo.calculating')"
              />
            </UFormGroup>
            
            <!-- 日志条目数量 -->
            <UFormGroup :label="$t('logSettings.fileInfo.logCount')" name="logCount" :help="$t('logSettings.fileInfo.logCountHelp')">
              <UInput
                v-model="logCountDisplay"
                readonly
                :placeholder="$t('logSettings.fileInfo.counting')"
              />
            </UFormGroup>
          </div>
        </UCard>
        
        <!-- 日志管理操作 -->
        <UCard>
          <template #header>
            <h3 class="text-base font-semibold text-gray-900 dark:text-white">{{ $t('logSettings.management.title') }}</h3>
          </template>
          
          <div class="space-y-4">
            <!-- TUN设备日志开关 -->
            <div class="flex items-center justify-between p-4 bg-green-50 dark:bg-green-900/20 rounded-lg border border-green-200 dark:border-green-800">
              <div>
                <h4 class="text-sm font-medium text-green-800 dark:text-green-200">{{ $t('logSettings.management.tunLogTitle') }}</h4>
                <p class="text-xs text-green-600 dark:text-green-400 mt-1">
                  {{ $t('logSettings.management.tunLogDesc') }}
                </p>
              </div>
              <UToggle
                v-model="tunLogEnabled"
                @change="updateTunLogSetting"
                :color="selectedThemeColor"
                size="md"
              />
            </div>
            <!-- 清理日志 -->
            <div class="flex items-center justify-between p-4 bg-yellow-50 dark:bg-yellow-900/20 rounded-lg border border-yellow-200 dark:border-yellow-800">
              <div>
                <h4 class="text-sm font-medium text-yellow-800 dark:text-yellow-200">{{ $t('logSettings.management.clearTitle') }}</h4>
                <p class="text-xs text-yellow-600 dark:text-yellow-400 mt-1">
                  {{ $t('logSettings.management.clearDesc') }}
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
                {{ $t('logSettings.management.clearButton') }}
              </UButton>
            </div>
            
            <!-- 导出日志 -->
            <div class="flex items-center justify-between p-4 bg-blue-50 dark:bg-blue-900/20 rounded-lg border border-blue-200 dark:border-blue-800">
              <div>
                <h4 class="text-sm font-medium text-blue-800 dark:text-blue-200">{{ $t('logSettings.management.exportTitle') }}</h4>
                <p class="text-xs text-blue-600 dark:text-blue-400 mt-1">
                  {{ $t('logSettings.management.exportDesc') }}
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
                {{ $t('logSettings.management.exportButton') }}
              </UButton>
            </div>
            
            <!-- 删除TUN日志 -->
            <div class="flex items-center justify-between p-4 bg-red-50 dark:bg-red-900/20 rounded-lg border border-red-200 dark:border-red-800">
              <div>
                <h4 class="text-sm font-medium text-red-800 dark:text-red-200">{{ $t('logSettings.management.clearTunTitle') }}</h4>
                <p class="text-xs text-red-600 dark:text-red-400 mt-1">
                  {{ $t('logSettings.management.clearTunDesc') }}
                </p>
              </div>
              <UButton
                @click="showClearTunConfirm = true"
                color="red"
                variant="outline"
                size="sm"
              >
                <Icon name="heroicons:trash" class="w-4 h-4 mr-1" />
                {{ $t('logSettings.management.clearTunButton') }}
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
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">{{ $t('logSettings.confirmClear.title') }}</h3>
        </template>
        
        <div class="space-y-4">
          <div class="flex items-start space-x-3">
            <Icon name="heroicons:exclamation-triangle" class="w-6 h-6 text-yellow-500 flex-shrink-0 mt-0.5" />
            <div>
              <p class="text-gray-900 dark:text-white font-medium">{{ $t('logSettings.confirmClear.question') }}</p>
              <p class="text-sm text-gray-600 dark:text-gray-400 mt-1">
                {{ $t('logSettings.confirmClear.warning') }}
              </p>
            </div>
          </div>
          
          <div class="bg-gray-50 dark:bg-gray-800 p-3 rounded-lg">
            <div class="text-sm text-gray-600 dark:text-gray-400">
              <p><strong>{{ $t('logSettings.confirmClear.filePath') }}：</strong>{{ logPath }}</p>
              <p><strong>{{ $t('logSettings.confirmClear.currentSize') }}：</strong>{{ logSizeDisplay }}</p>
              <p><strong>{{ $t('logSettings.confirmClear.logEntries') }}：</strong>{{ logCountDisplay }}</p>
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
              {{ $t('common.cancel') }}
            </UButton>
            <UButton
              :color="selectedThemeColor"
              @click="clearLogFile"
              :loading="clearing"
            >
              {{ $t('logSettings.confirmClear.confirmButton') }}
            </UButton>
          </div>
        </template>
      </UCard>
    </UModal>
    
    <!-- 清理TUN日志确认对话框 -->
    <UModal v-model="showClearTunConfirm">
      <UCard>
        <template #header>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">{{ $t('logSettings.confirmClearTun.title') }}</h3>
        </template>
        
        <div class="space-y-4">
          <div class="flex items-start space-x-3">
            <Icon name="heroicons:exclamation-triangle" class="w-6 h-6 text-red-500 flex-shrink-0 mt-0.5" />
            <div>
              <p class="text-gray-900 dark:text-white font-medium">{{ $t('logSettings.confirmClearTun.question') }}</p>
              <p class="text-sm text-gray-600 dark:text-gray-400 mt-1">
                {{ $t('logSettings.confirmClearTun.warning') }}
              </p>
            </div>
          </div>
          
          <div class="bg-gray-50 dark:bg-gray-800 p-3 rounded-lg">
            <div class="text-sm text-gray-600 dark:text-gray-400">
              <p><strong>{{ $t('logSettings.confirmClearTun.description') }}</strong></p>
              <p class="mt-2">{{ $t('logSettings.confirmClearTun.fileLocation') }}</p>
            </div>
          </div>
        </div>
        
        <template #footer>
          <div class="flex justify-end space-x-2">
            <UButton
              variant="ghost"
              @click="showClearTunConfirm = false"
              :disabled="clearingTun"
            >
              {{ $t('common.cancel') }}
            </UButton>
            <UButton
              color="red"
              @click="clearTunLogFile"
              :loading="clearingTun"
            >
              {{ $t('logSettings.confirmClearTun.confirmButton') }}
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
import { useI18n } from 'vue-i18n'

// 国际化
const { t: $t } = useI18n()

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
const showClearTunConfirm = ref(false)
const clearingTun = ref(false)
const tunLogEnabled = ref(false)

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
  return logCount.value.toLocaleString() + ' ' + $t('logSettings.fileInfo.entries')
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
      title: $t('logSettings.messages.getInfoFailed'),
      description: error instanceof Error ? error.message : $t('common.unknownError'),
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
      title: $t('logSettings.messages.refreshSuccess'),
      description: $t('logSettings.messages.refreshSuccessDesc'),
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
      title: $t('logSettings.messages.openDirFailed'),
      description: error instanceof Error ? error.message : $t('common.unknownError'),
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
      title: $t('logSettings.messages.clearSuccess'),
      description: $t('logSettings.messages.clearSuccessDesc'),
      color: 'green'
    })
  } catch (error) {
    console.error('清理日志失败:', error)
    toast.add({
      title: $t('logSettings.messages.clearFailed'),
      description: error instanceof Error ? error.message : $t('common.unknownError'),
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
        title: $t('logSettings.messages.exportSuccess'),
        description: $t('logSettings.messages.exportSuccessDesc', { path: savePath }),
        color: 'green'
      })
    }
  } catch (error) {
    console.error('导出日志失败:', error)
    toast.add({
      title: $t('logSettings.messages.exportFailed'),
      description: error instanceof Error ? error.message : $t('common.unknownError'),
      color: 'red'
    })
  } finally {
    exporting.value = false
  }
}

/**
 * 清理TUN日志文件
 */
const clearTunLogFile = async () => {
  clearingTun.value = true
  try {
    await invoke('clear_tun_log_file')
    
    showClearTunConfirm.value = false
    toast.add({
      title: $t('logSettings.messages.clearTunSuccess'),
      description: $t('logSettings.messages.clearTunSuccessDesc'),
      color: 'green'
    })
  } catch (error) {
    console.error('清理TUN日志失败:', error)
    toast.add({
      title: $t('logSettings.messages.clearTunFailed'),
      description: error instanceof Error ? error.message : $t('common.unknownError'),
      color: 'red'
    })
  } finally {
    clearingTun.value = false
  }
}

/**
 * 获取TUN日志配置
 */
const getTunLogConfig = async () => {
  try {
    const config = await invoke('get_config') as { tun_log_enabled: boolean }
    tunLogEnabled.value = config.tun_log_enabled
  } catch (error) {
    console.error('获取TUN日志配置失败:', error)
  }
}

/**
 * 更新TUN日志设置
 */
const updateTunLogSetting = async (enabled: boolean) => {
  try {
    await invoke('update_tun_log_setting', { enabled })
    
    toast.add({
      title: $t('logSettings.messages.tunLogUpdateSuccess'),
      description: enabled 
        ? $t('logSettings.messages.tunLogEnabledDesc')
        : $t('logSettings.messages.tunLogDisabledDesc'),
      color: 'green'
    })
  } catch (error) {
    console.error('更新TUN日志设置失败:', error)
    // 恢复原来的状态
    tunLogEnabled.value = !enabled
    toast.add({
      title: $t('logSettings.messages.tunLogUpdateFailed'),
      description: error instanceof Error ? error.message : $t('common.unknownError'),
      color: 'red'
    })
  }
}

// 生命周期钩子
onMounted(async () => {
  await getLogInfo()
  await getTunLogConfig()
})
</script>
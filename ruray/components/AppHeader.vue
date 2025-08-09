<template>
  <div class="h-12 bg-white dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between px-4 drag-region">
    <!-- 左侧 Logo 和标题 -->
    <div class="flex items-center space-x-3 no-drag">
      <div class="w-8 h-8 bg-gradient-to-br from-green-400 to-green-600 rounded-lg flex items-center justify-center">
        <Icon name="heroicons:bolt" class="w-5 h-5 text-white" />
      </div>
      <h1 class="text-lg font-semibold text-gray-900 dark:text-white">RuRay</h1>
    </div>
    
    <!-- 中间菜单 -->
    <div class="flex items-center space-x-1 no-drag">
      <UButton
        variant="ghost"
        size="sm"
        @click="showUpdateDialog = true"
      >
        <Icon name="heroicons:arrow-down-tray" class="w-4 h-4 mr-1" />
        更新 Xray Core
      </UButton>
      
      <UButton
        variant="ghost"
        size="sm"
        @click="toggleMinimalMode"
      >
        <Icon name="heroicons:minus" class="w-4 h-4 mr-1" />
        极简模式
      </UButton>
    </div>
    
    <!-- 右侧控制按钮 -->
    <div class="flex items-center space-x-2 no-drag">
      <!-- 主题切换 -->
      <UButton
        variant="ghost"
        size="sm"
        @click="toggleColorMode"
      >
        <Icon :name="colorMode.value === 'dark' ? 'heroicons:sun' : 'heroicons:moon'" class="w-4 h-4" />
      </UButton>
      
      <!-- 设置 -->
      <UButton
        variant="ghost"
        size="sm"
        @click="showSettings = true"
      >
        <Icon name="heroicons:cog-6-tooth" class="w-4 h-4" />
      </UButton>
      
      <!-- 窗口控制按钮 -->
      <div class="flex items-center space-x-1 ml-2">
        <UButton
          variant="ghost"
          size="sm"
          @click="minimizeWindow"
        >
          <Icon name="heroicons:minus" class="w-3 h-3" />
        </UButton>
        
        <UButton
          variant="ghost"
          size="sm"
          @click="toggleMaximize"
        >
          <Icon name="heroicons:stop" class="w-3 h-3" />
        </UButton>
        
        <UButton
          variant="ghost"
          size="sm"
          color="red"
          @click="closeWindow"
        >
          <Icon name="heroicons:x-mark" class="w-3 h-3" />
        </UButton>
      </div>
    </div>
    
    <!-- 更新对话框 -->
    <UModal v-model="showUpdateDialog">
      <UCard>
        <template #header>
          <div class="flex items-center space-x-2">
            <Icon name="heroicons:arrow-down-tray" class="w-5 h-5 text-green-500" />
            <span>更新 Xray Core</span>
          </div>
        </template>
        
        <div class="space-y-4">
          <p class="text-gray-600 dark:text-gray-400">
            检查并下载最新版本的 Xray Core
          </p>
          
          <div v-if="updateStatus" class="p-3 bg-blue-50 dark:bg-blue-900/20 rounded-lg">
            <p class="text-sm text-blue-700 dark:text-blue-300">{{ updateStatus }}</p>
          </div>
          
          <div v-if="updateProgress > 0" class="space-y-2">
            <div class="flex justify-between text-sm">
              <span>下载进度</span>
              <span>{{ updateProgress }}%</span>
            </div>
            <UProgress :value="updateProgress" />
          </div>
        </div>
        
        <template #footer>
          <div class="flex justify-end space-x-2">
            <UButton variant="ghost" @click="showUpdateDialog = false" :disabled="isUpdating">
              取消
            </UButton>
            <UButton @click="startUpdate" :loading="isUpdating">
              {{ isUpdating ? '更新中...' : '开始更新' }}
            </UButton>
          </div>
        </template>
      </UCard>
    </UModal>
    
    <!-- 设置对话框 -->
    <UModal v-model="showSettings">
      <UCard>
        <template #header>
          <div class="flex items-center space-x-2">
            <Icon name="heroicons:cog-6-tooth" class="w-5 h-5 text-green-500" />
            <span>设置</span>
          </div>
        </template>
        
        <div class="space-y-6">
          <!-- 主题设置 -->
          <div>
            <h3 class="text-lg font-medium mb-3">主题设置</h3>
            <div class="space-y-3">
              <div class="flex items-center justify-between">
                <span>颜色模式</span>
                <USelectMenu
                  v-model="selectedColorModeOption"
                  :options="colorModeOptions"
                  @change="setColorMode"
                />
              </div>
              
              <div class="flex items-center justify-between">
                <span>主题色</span>
                <div class="flex space-x-2">
                  <button
                    v-for="color in themeColors"
                    :key="color.name"
                    :class="[
                      'w-6 h-6 rounded-full border-2',
                      selectedThemeColor === color.name ? 'border-gray-400' : 'border-transparent'
                    ]"
                    :style="{ backgroundColor: color.value }"
                    @click="setThemeColor(color.name)"
                  />
                </div>
              </div>
            </div>
          </div>
          
          <!-- 代理设置 -->
          <div>
            <h3 class="text-lg font-medium mb-3">代理设置</h3>
            <div class="space-y-3">
              <div class="flex items-center justify-between">
                <span>HTTP 代理端口</span>
                <UInput
                  v-model="httpPort"
                  type="number"
                  placeholder="10086"
                  class="w-24"
                />
              </div>
              
              <div class="flex items-center justify-between">
                <span>SOCKS 代理端口</span>
                <UInput
                  v-model="socksPort"
                  type="number"
                  placeholder="10087"
                  class="w-24"
                />
              </div>
              
              <div class="flex items-center justify-between">
                <span>启动时自动连接</span>
                <UToggle v-model="autoConnect" />
              </div>
              
              <div class="border-t pt-3 mt-3">
                <h4 class="text-sm font-medium mb-2">Inbound 高级设置</h4>
                
                <div class="flex items-center justify-between">
                  <span class="text-sm">启用流量嗅探</span>
                  <UToggle v-model="inboundSniffingEnabled" />
                </div>
                
                <div class="flex items-center justify-between">
                  <span class="text-sm">启用 UDP 转发</span>
                  <UToggle v-model="inboundUdpEnabled" />
                </div>
                
                <div class="flex items-center justify-between">
                  <span class="text-sm">允许透明代理</span>
                  <UToggle v-model="inboundAllowTransparent" />
                </div>
                
                <div class="flex items-center justify-between">
                  <span class="text-sm">认证方式</span>
                  <USelect
                    v-model="inboundAuthMethod"
                    :options="authMethodOptions"
                    class="w-24"
                  />
                </div>
              </div>
            </div>
          </div>

          <!-- Xray 设置 -->
          <div>
            <h3 class="text-lg font-medium mb-3">Xray Core 设置</h3>
            <div class="space-y-3">
              <div class="space-y-2">
                <div class="flex items-center justify-between">
                  <span>Xray Core 路径</span>
                  <UButton
                    variant="outline"
                    size="xs"
                    @click="selectXrayPath"
                    :disabled="isSelectingPath"
                  >
                    {{ isSelectingPath ? '选择中...' : '浏览' }}
                  </UButton>
                </div>
                <UInput
                  v-model="xrayPath"
                  placeholder="留空使用默认路径"
                  readonly
                  class="text-xs"
                />
                <div class="flex items-center space-x-2 text-xs">
                  <div
                    :class="[
                      'w-2 h-2 rounded-full',
                      xrayExists ? 'bg-green-500' : 'bg-red-500'
                    ]"
                  ></div>
                  <span :class="xrayExists ? 'text-green-600 dark:text-green-400' : 'text-red-600 dark:text-red-400'">
                    {{ xrayExists ? 'Xray Core 已找到' : 'Xray Core 未找到' }}
                  </span>
                  <UButton
                    variant="ghost"
                    size="xs"
                    @click="checkXrayExists"
                    :loading="isCheckingXray"
                  >
                    重新检查
                  </UButton>
                </div>
              </div>
            </div>
          </div>
        </div>
        
        <template #footer>
          <div class="flex justify-end space-x-2">
            <UButton variant="ghost" @click="showSettings = false">
              取消
            </UButton>
            <UButton @click="saveSettings">
              保存
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
import { open } from '@tauri-apps/plugin-dialog'

const colorMode = useColorMode()
const appConfig = useAppConfig()

// 对话框状态
const showUpdateDialog = ref(false)
const showSettings = ref(false)

// 更新状态
const isUpdating = ref(false)
const updateStatus = ref('')
const updateProgress = ref(0)

// 设置选项
const selectedColorMode = ref(colorMode.preference)
const selectedThemeColor = ref(appConfig.ui?.primary || 'green')
const httpPort = ref(10086)
const socksPort = ref(10087)
const autoConnect = ref(false)

// Inbound 高级设置
const inboundSniffingEnabled = ref(false)
const inboundUdpEnabled = ref(false)
const inboundAllowTransparent = ref(false)
const inboundAuthMethod = ref('noauth')

// Xray 相关状态
const xrayPath = ref('')
const xrayExists = ref(false)
const isSelectingPath = ref(false)
const isCheckingXray = ref(false)

const colorModeOptions = [
  { label: '跟随系统', value: 'system' },
  { label: '浅色模式', value: 'light' },
  { label: '深色模式', value: 'dark' }
]

const themeColors = [
  { name: 'green', value: '#22c55e' },
  { name: 'blue', value: '#3b82f6' },
  { name: 'purple', value: '#8b5cf6' },
  { name: 'pink', value: '#ec4899' },
  { name: 'orange', value: '#f97316' }
]

const authMethodOptions = [
  { label: '无认证', value: 'noauth' },
  { label: '用户名密码', value: 'password' }
]

// 计算属性：处理颜色模式选项的双向绑定
const selectedColorModeOption = computed({
  get() {
    const colorModeValue = colorModeOptions.find(option => option.value === selectedColorMode.value) || colorModeOptions[0]
    switch (colorModeValue.value) {
      case 'system':
        return '跟随系统'
      case 'light':
        return '浅色模式'
      case 'dark':
        return '深色模式'
      default:
        return '跟随系统'
    }
  },
  set(option) {
    const mode = typeof option === 'string' ? option : option.value
    colorMode.preference = mode
    selectedColorMode.value = mode
  }
})

// 方法
const toggleColorMode = () => {
  colorMode.preference = colorMode.value === 'dark' ? 'light' : 'dark'
}

// 颜色模式映射
const colorModeMapValue = (colorMode: string): string => {
  switch (colorMode) {
    case 'system':
      return '跟随系统'
    case 'light':
      return '浅色模式'
    case 'dark':
      return '深色模式'
    default:
      return '跟随系统'
  }
}

const setColorMode = (option: any) => {
  // 通过计算属性的 setter 处理
  selectedColorModeOption.value = option
}

const setThemeColor = (color: string) => {
  selectedThemeColor.value = color
  // 动态更新应用配置中的主题色
  updateAppConfig({
    ui: {
      primary: color
    }
  })
}

// 定义事件
const emit = defineEmits<{
  'toggle-zen-mode': []
}>()

const toggleMinimalMode = () => {
  emit('toggle-zen-mode')
}

const minimizeWindow = async () => {
  const { getCurrentWindow } = await import('@tauri-apps/api/window')
  await getCurrentWindow().minimize()
}

const toggleMaximize = async () => {
  const { getCurrentWindow } = await import('@tauri-apps/api/window')
  const window = getCurrentWindow()
  const isMaximized = await window.isMaximized()
  
  if (isMaximized) {
    await window.unmaximize()
  } else {
    await window.maximize()
  }
}

const closeWindow = async () => {
  const { getCurrentWindow } = await import('@tauri-apps/api/window')
  await getCurrentWindow().close()
}

const startUpdate = async () => {
  isUpdating.value = true
  updateStatus.value = '正在检查最新版本...'
  updateProgress.value = 0
  
  try {
    // TODO: 实现 Xray Core 更新逻辑
    // 模拟更新过程
    for (let i = 0; i <= 100; i += 10) {
      updateProgress.value = i
      updateStatus.value = `下载中... ${i}%`
      await new Promise(resolve => setTimeout(resolve, 200))
    }
    
    updateStatus.value = '更新完成！'
    setTimeout(() => {
      showUpdateDialog.value = false
      isUpdating.value = false
      updateProgress.value = 0
      updateStatus.value = ''
    }, 1000)
  } catch (error) {
    updateStatus.value = '更新失败，请稍后重试'
    isUpdating.value = false
  }
}

const saveSettings = async () => {
  try {
    // 获取当前配置
    const config = await invoke('get_app_config') as any
    
    // 更新配置
    config.xray_path = xrayPath.value || null
    config.http_port = parseInt(httpPort.value.toString()) || 10086
    config.socks_port = parseInt(socksPort.value.toString()) || 10087
    config.theme_color = selectedThemeColor.value || 'green'
    
    // 更新 inbound 配置
    config.inbound_sniffing_enabled = inboundSniffingEnabled.value
    config.inbound_udp_enabled = inboundUdpEnabled.value
    config.inbound_allow_transparent = inboundAllowTransparent.value
    config.inbound_auth_method = inboundAuthMethod.value
    
    // 保存配置
    await invoke('save_app_config', { config })
    
    showSettings.value = false
  } catch (error) {
    console.error('保存设置失败:', error)
  }
}

// Xray 相关方法
const selectXrayPath = async () => {
  try {
    isSelectingPath.value = true
    
    const selected = await open({
      title: '选择 Xray Core 可执行文件',
      filters: [{
        name: 'Executable',
        extensions: ['exe']
      }]
    })
    
    if (selected && typeof selected === 'string') {
      xrayPath.value = selected
      await checkXrayExists()
    }
  } catch (error) {
    console.error('选择文件失败:', error)
  } finally {
    isSelectingPath.value = false
  }
}

const checkXrayExists = async () => {
  try {
    isCheckingXray.value = true
    xrayExists.value = await invoke('check_xray_exists')
  } catch (error) {
    console.error('检查 Xray Core 失败:', error)
    xrayExists.value = false
  } finally {
    isCheckingXray.value = false
  }
}

const loadSettings = async () => {
  try {
    // 加载应用配置
    const config = await invoke('get_app_config') as any
    xrayPath.value = config.xray_path || ''
    httpPort.value = config.http_port || 10086
    socksPort.value = config.socks_port || 10087
    selectedThemeColor.value = config.theme_color || 'green'
    
    // 加载 inbound 配置
    inboundSniffingEnabled.value = config.inbound_sniffing_enabled || false
    inboundUdpEnabled.value = config.inbound_udp_enabled || false
    inboundAllowTransparent.value = config.inbound_allow_transparent || false
    inboundAuthMethod.value = config.inbound_auth_method || 'noauth'
    
    // 应用主题色
    setThemeColor(selectedThemeColor.value)
    
    // 检查 Xray Core 是否存在
    await checkXrayExists()
  } catch (error) {
    console.error('加载设置失败:', error)
  }
}

// 组件挂载时加载设置
onMounted(() => {
  loadSettings()
})
</script>
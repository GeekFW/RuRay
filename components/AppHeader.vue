<template>
  <div
    class="h-12 bg-white dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between px-4 drag-region">
    <!-- 左侧 Logo 和标题 -->
    <div class="flex items-center space-x-3 no-drag">
      <div class="w-8 h-8 bg-gradient-to-br from-green-400 to-green-600 rounded-lg flex items-center justify-center">
        <Icon name="heroicons:bolt" class="w-5 h-5 text-white" />
      </div>
      <h1 class="text-lg font-semibold text-gray-900 dark:text-white">RuRay</h1>
    </div>

    <!-- 中间菜单 -->
    <div class="flex items-center space-x-1 no-drag">
      <UButton variant="ghost" size="sm" @click="showUpdateDialog = true">
        <Icon name="heroicons:arrow-down-tray" class="w-4 h-4 mr-1" />
        更新 Xray Core
      </UButton>

      <UButton variant="ghost" size="sm" @click="toggleMinimalMode">
        <Icon name="heroicons:minus" class="w-4 h-4 mr-1" />
        极简模式
      </UButton>
    </div>

    <!-- 右侧控制按钮 -->
    <div class="flex items-center space-x-2 no-drag">
      <!-- 主题切换 -->
      <UButton variant="ghost" size="sm" @click="toggleColorMode">
        <Icon :name="colorMode.value === 'dark' ? 'heroicons:sun' : 'heroicons:moon'" class="w-4 h-4" />
      </UButton>

      <!-- 设置 -->
      <UButton variant="ghost" size="sm" @click="openSettings">
        <Icon name="heroicons:cog-6-tooth" class="w-4 h-4" />
      </UButton>

      <!-- 窗口控制按钮 -->
      <div class="flex items-center space-x-1 ml-2">
        <UButton variant="ghost" size="sm" @click="minimizeWindow">
          <Icon name="heroicons:minus" class="w-3 h-3" />
        </UButton>

        <UButton variant="ghost" size="sm" @click="toggleMaximize">
          <Icon name="heroicons:stop" class="w-3 h-3" />
        </UButton>

        <UButton variant="ghost" size="sm" color="red" @click="closeWindow">
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
    <UModal v-model="showSettings" :ui="{
      container: 'flex min-h-full items-center justify-center p-4',
      wrapper: 'flex items-center justify-center min-h-full',
      inner: 'flex items-center justify-center min-h-full',
      base: 'relative text-left rtl:text-right flex flex-col bg-white dark:bg-gray-900 shadow-xl w-full sm:max-w-xl rounded-lg sm:my-8'
    }">
      <div class="flex items-center justify-center min-h-full w-full">
        <UCard class="h-[90vh] max-h-[90vh] overflow-hidden flex flex-col w-[800px]">
          <template #header>
            <div class="flex items-center justify-between">
              <div class="flex items-center space-x-2">
                <Icon name="heroicons:cog-6-tooth" class="w-5 h-5 text-green-500" />
                <span>设置</span>
              </div>
              <UButton variant="ghost" size="sm" icon="i-heroicons-x-mark" @click="showSettings = false"
                class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300" />
            </div>
          </template>

          <div class="flex-1 min-h-0 flex flex-col">
            <!-- Tab 导航 -->
            <div class="border-b border-gray-200 dark:border-gray-700 flex-shrink-0">
              <nav class="flex space-x-8 px-6" aria-label="Tabs">
                <button v-for="(tab, index) in settingsTabOptions" :key="tab.value"
                  @click="activeSettingsTab = tab.value" :class="[
                    'whitespace-nowrap py-4 px-1 border-b-2 font-medium text-sm transition-colors',
                    activeSettingsTab === tab.value
                      ? 'border-green-500 text-green-600 dark:text-green-400'
                      : 'border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300 dark:text-gray-400 dark:hover:text-gray-300'
                  ]">
                  {{ tab.label }}
                </button>
              </nav>
            </div>

            <!-- Tab 内容 -->
            <div class="flex-1 min-h-0 max-h-[calc(90vh-240px)] overflow-y-auto px-6 py-6">
              <!-- 基础设置 Tab -->
              <div v-show="activeSettingsTab === 0" class="space-y-6">
                <!-- 主题设置 -->
                <div>
                  <h3 class="text-lg font-medium mb-2">主题设置</h3>
                  <div class="flex items-center justify-between mb-4">
                    <span>颜色模式</span>
                    <USelectMenu v-model="selectedColorModeOption" value-attribute="value" :options="colorModeOptions"
                      @change="setColorMode" />
                  </div>

                  <div class="flex items-center justify-between mb-4">
                    <span>主题色</span>
                    <div class="flex space-x-2">
                      <button v-for="color in themeColors" :key="color.name" :class="[
                        'w-6 h-6 rounded-full border-2',
                        selectedThemeColor === color.name ? 'border-gray-400' : 'border-transparent'
                      ]" :style="{ backgroundColor: color.value }" @click="setThemeColor(color.name)" />
                    </div>
                  </div>
                </div>
                <!-- 代理设置 -->
                <div>
                  <h3 class="text-lg font-medium mb-2">代理设置</h3>
                  <div class="space-y-3">
                    <div class="flex items-center justify-between">
                      <span>HTTP 代理端口</span>
                      <UInput v-model="httpPort" type="number" placeholder="10086" class="w-24" />
                    </div>

                    <div class="flex items-center justify-between">
                      <span>SOCKS 代理端口</span>
                      <UInput v-model="socksPort" type="number" placeholder="10087" class="w-24" />
                    </div>

                    <div class="flex items-center justify-between">
                      <span>启动时自动连接</span>
                      <UToggle v-model="autoConnect" />
                    </div>

                    <div class="border-t pt-3 mt-3">
                      <h4 class="text-sm font-medium mb-3">Inbound 高级设置</h4>

                      <div class="space-y-4">
                        <div class="flex items-center justify-between py-1">
                          <span class="text-sm">启用流量嗅探</span>
                          <UToggle v-model="inboundSniffingEnabled" />
                        </div>

                        <div class="flex items-center justify-between py-1">
                          <span class="text-sm">启用 UDP 转发</span>
                          <UToggle v-model="inboundUdpEnabled" />
                        </div>

                        <div class="flex items-center justify-between py-1">
                          <span class="text-sm">允许透明代理</span>
                          <UToggle v-model="inboundAllowTransparent" />
                        </div>

                        <div class="flex items-center justify-between py-1">
                          <span class="text-sm">认证方式</span>
                          <USelect v-model="inboundAuthMethod" :options="authMethodOptions" class="w-24" />
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              </div>

              <!-- 路由设置 Tab -->
              <div v-show="activeSettingsTab === 1" class="space-y-6">
                <div>
                  <h3 class="text-lg font-medium mb-3">路由设置</h3>
                  <div class="space-y-4">
                    <div class="flex items-center justify-between">
                      <span>域名策略</span>
                      <USelectMenu class="w-32" v-model="routingDomainStrategy" value-attribute="value"
                        :options="domainStrategyOptions" />
                    </div>

                    <div class="border-t pt-4">
                      <div class="flex items-center justify-between mb-3">
                        <h4 class="text-sm font-medium">路由规则</h4>
                        <UButton variant="outline" size="xs" @click="addRoutingRule" icon="i-heroicons-plus">
                          添加规则
                        </UButton>
                      </div>

                      <div class="space-y-3 max-h-60 overflow-y-auto">
                        <div v-for="(rule, index) in routingRules" :key="index" class="border rounded-lg p-3 space-y-3">
                          <div class="flex items-center justify-between">
                            <span class="text-sm font-medium">规则 {{ index + 1 }}</span>
                            <UButton variant="ghost" size="xs" color="red" @click="removeRoutingRule(index)"
                              icon="i-heroicons-trash" />
                          </div>

                          <div class="grid grid-cols-2 gap-3">
                            <div>
                              <label class="text-xs text-gray-500 dark:text-gray-400">出站标签</label>
                              <USelectMenu v-model="rule.outboundTag" value-attribute="value"
                                :options="outboundTagOptions" class="mt-1" />
                            </div>
                            <div>
                              <label class="text-xs text-gray-500 dark:text-gray-400">规则类型</label>
                              <UInput v-model="rule.type" placeholder="field" class="mt-1" readonly />
                            </div>
                          </div>

                          <div>
                            <label class="text-xs text-gray-500 dark:text-gray-400">IP 规则</label>
                            <div class="mt-1 space-y-2">
                              <div class="flex space-x-2">
                                <USelectMenu v-model="newIpRule[index]" value-attribute="value"
                                  :options="ipPresetOptions" placeholder="选择预设或手动输入" class="flex-1" />
                                <UButton variant="outline" size="xs" @click="addIpRule(index)"
                                  :disabled="!newIpRule[index]">
                                  添加
                                </UButton>
                              </div>
                              <UInput v-model="customIpRule[index]" placeholder="或手动输入 IP 规则" class="text-xs"
                                @keyup.enter="addCustomIpRule(index)" />
                              <div class="flex flex-wrap gap-1">
                                <UBadge v-for="(ip, ipIndex) in rule.ip" :key="ipIndex" variant="soft" size="sm"
                                  class="cursor-pointer" @click="removeIpRule(index, ipIndex)">
                                  {{ ip }}
                                  <Icon name="i-heroicons-x-mark" class="w-3 h-3 ml-1" />
                                </UBadge>
                              </div>
                            </div>
                          </div>

                          <div>
                            <label class="text-xs text-gray-500 dark:text-gray-400">域名规则</label>
                            <div class="mt-1 space-y-2">
                              <div class="flex space-x-2">
                                <USelectMenu v-model="newDomainRule[index]" value-attribute="value"
                                  :options="domainPresetOptions" placeholder="选择预设或手动输入" class="flex-1" />
                                <UButton variant="outline" size="xs" @click="addDomainRule(index)"
                                  :disabled="!newDomainRule[index]">
                                  添加
                                </UButton>
                              </div>
                              <UInput v-model="customDomainRule[index]" placeholder="或手动输入域名规则" class="text-xs"
                                @keyup.enter="addCustomDomainRule(index)" />
                              <div class="flex flex-wrap gap-1">
                                <UBadge v-for="(domain, domainIndex) in rule.domain" :key="domainIndex" variant="soft"
                                  size="sm" class="cursor-pointer" @click="removeDomainRule(index, domainIndex)">
                                  {{ domain }}
                                  <Icon name="i-heroicons-x-mark" class="w-3 h-3 ml-1" />
                                </UBadge>
                              </div>
                            </div>
                          </div>
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              </div>

              <!-- Core设置 Tab -->
              <div v-show="activeSettingsTab === 2" class="space-y-6">
                <div>
                  <h3 class="text-lg font-medium mb-3">Xray Core 设置</h3>
                  <div class="space-y-2">
                    <div class="flex items-center justify-between">
                      <span>Xray Core 路径</span>
                      <div class="flex space-x-2">
                        <UButton variant="outline" size="xs" @click="selectXrayPath" :disabled="isSelectingPath">
                          {{ isSelectingPath ? '选择中...' : '浏览' }}
                        </UButton>
                        <UButton variant="ghost" size="xs" @click="clearXrayPath" :disabled="!xrayPath" color="red">
                          清空
                        </UButton>
                      </div>
                    </div>
                    <UInput v-model="xrayPath" :placeholder="defaultXrayPathPlaceholder" class="text-xs" />
                    <div class="text-xs text-gray-500 dark:text-gray-400">
                      <span v-if="!xrayPath">
                        默认路径: {{ defaultXrayPath }}
                      </span>
                      <span v-else>
                        自定义路径: {{ xrayPath }}
                      </span>
                    </div>
                    <div class="space-y-2">
                      <div class="flex items-center space-x-2 text-xs">
                        <div :class="[
                          'w-2 h-2 rounded-full',
                          xrayExists ? 'bg-green-500' : 'bg-red-500'
                        ]"></div>
                        <span
                          :class="xrayExists ? 'text-green-600 dark:text-green-400' : 'text-red-600 dark:text-red-400'">
                          {{ xrayExists ? 'Xray Core 已找到' : 'Xray Core 未找到' }}
                        </span>
                        <UButton variant="ghost" size="xs" @click="checkXrayExists" :loading="isCheckingXray">
                          重新检查
                        </UButton>
                      </div>

                      <div class="flex items-center space-x-2 text-xs">
                        <div :class="[
                          'w-2 h-2 rounded-full',
                          geoFilesExist ? 'bg-green-500' : 'bg-red-500'
                        ]"></div>
                        <span
                          :class="geoFilesExist ? 'text-green-600 dark:text-green-400' : 'text-red-600 dark:text-red-400'">
                          {{ geoFilesExist ? '地理位置数据文件已找到' : '地理位置数据文件缺失' }}
                        </span>
                        <UButton variant="ghost" size="xs" @click="downloadGeoFiles" :loading="isDownloadingGeoFiles"
                          v-if="!geoFilesExist">
                          下载
                        </UButton>
                      </div>

                      <div v-if="!xrayExists || !geoFilesExist" class="mt-2">
                        <UButton variant="outline" size="sm" @click="ensureAllXrayFiles" :loading="isEnsuring"
                          :color="selectedThemeColor">
                          {{ isEnsuring ? '正在设置...' : '一键设置 Xray' }}
                        </UButton>
                      </div>

                      <div v-if="setupProgress > 0" class="space-y-2 mt-2">
                        <div class="flex justify-between text-xs">
                          <span>{{ setupStatus }}</span>
                          <span>{{ setupProgress }}%</span>
                        </div>
                        <UProgress :value="setupProgress" size="xs" />
                      </div>
                    </div>
                  </div>
                </div>
              </div>

              <!-- TUN配置 Tab -->
              <div v-show="activeSettingsTab === 3" class="space-y-6">
                <TunConfig />
              </div>
              
              <!-- 日志设置 Tab -->
              <div v-show="activeSettingsTab === 4" class="space-y-6">
                <LogSettings />
              </div>
            </div>
          </div>

          <template #footer>
            <div class="absolute bottom-4 left-0 right-0 px-6">
              <div class="flex justify-end space-x-2">
                <UButton variant="ghost" @click="showSettings = false">
                  取消
                </UButton>
                <UButton @click="saveSettings">
                  保存
                </UButton>
              </div>
            </div>
          </template>
        </UCard>
      </div>
    </UModal>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'

const toast = useToast()

const colorMode = useColorMode()
const appConfig = useAppConfig()

// 对话框状态
const showUpdateDialog = ref(false)
const showSettings = ref(false)
const activeSettingsTab = ref(0)

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

// 路由配置
const routingDomainStrategy = ref('AsIs')
const routingRules = ref([
  {
    type: 'field',
    ip: ['geoip:private'],
    domain: [],
    outboundTag: 'direct'
  }
])

// 路由规则编辑相关
const newIpRule = ref({})
const newDomainRule = ref({})
const customIpRule = ref({})
const customDomainRule = ref({})

// Xray 相关状态
const xrayPath = ref('')
const xrayExists = ref(false)
const isSelectingPath = ref(false)
const isCheckingXray = ref(false)
const defaultXrayPath = ref('')
const defaultXrayPathPlaceholder = computed(() =>
  defaultXrayPath.value ? `留空使用默认路径: ${defaultXrayPath.value}` : '留空使用默认路径'
)

// 地理位置数据文件相关状态
const geoFilesExist = ref(false)
const isDownloadingGeoFiles = ref(false)
const isEnsuring = ref(false)
const setupProgress = ref(0)
const setupStatus = ref('')

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

const domainStrategyOptions = [
  { label: 'AsIs', value: 'AsIs' },
  { label: 'IPIfNonMatch', value: 'IPIfNonMatch' },
  { label: 'IPOnDemand', value: 'IPOnDemand' }
]

const outboundTagOptions = [
  { label: '代理', value: 'proxy' },
  { label: '直连', value: 'direct' },
  { label: '阻断', value: 'block' }
]

const ipPresetOptions = [
  { label: '私有地址', value: 'geoip:private' },
  { label: '中国大陆', value: 'geoip:cn' },
  { label: '非中国大陆', value: 'geoip:!cn' }
]

const domainPresetOptions = [
  { label: '中国大陆网站', value: 'geosite:cn' },
  { label: '私有域名', value: 'geosite:private' },
  { label: '非中国大陆网站', value: 'geosite:!cn' }
]

const settingsTabOptions = [
  { label: '基础设置', value: 0 },
  { label: '路由设置', value: 1 },
  { label: 'Core设置', value: 2 },
  { label: 'TUN配置', value: 3 },
  { label: '日志设置', value: 4 }
]

// 计算属性：处理颜色模式选项的双向绑定
const selectedColorModeOption = computed({
  get() {
    return selectedColorMode.value ? selectedColorMode.value : colorModeOptions[0]
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

// 路由配置方法
const addRoutingRule = () => {
  const newIndex = routingRules.value.length
  routingRules.value.push({
    type: 'field',
    ip: [],
    domain: [],
    outboundTag: 'proxy'
  })

  // 初始化新规则的编辑状态
  newIpRule.value[newIndex] = ''
  newDomainRule.value[newIndex] = ''
  customIpRule.value[newIndex] = ''
  customDomainRule.value[newIndex] = ''
}

const removeRoutingRule = (index: number) => {
  routingRules.value.splice(index, 1)

  // 清理编辑状态
  delete newIpRule.value[index]
  delete newDomainRule.value[index]
  delete customIpRule.value[index]
  delete customDomainRule.value[index]
}

const addIpRule = (ruleIndex: number) => {
  const rule = newIpRule.value[ruleIndex]
  if (rule && !routingRules.value[ruleIndex].ip.includes(rule)) {
    routingRules.value[ruleIndex].ip.push(rule)
    newIpRule.value[ruleIndex] = ''
  }
}

const addCustomIpRule = (ruleIndex: number) => {
  const rule = customIpRule.value[ruleIndex]
  if (rule && !routingRules.value[ruleIndex].ip.includes(rule)) {
    routingRules.value[ruleIndex].ip.push(rule)
    customIpRule.value[ruleIndex] = ''
  }
}

const removeIpRule = (ruleIndex: number, ipIndex: number) => {
  routingRules.value[ruleIndex].ip.splice(ipIndex, 1)
}

const addDomainRule = (ruleIndex: number) => {
  const rule = newDomainRule.value[ruleIndex]
  if (rule && !routingRules.value[ruleIndex].domain.includes(rule)) {
    routingRules.value[ruleIndex].domain.push(rule)
    newDomainRule.value[ruleIndex] = ''
  }
}

const addCustomDomainRule = (ruleIndex: number) => {
  const rule = customDomainRule.value[ruleIndex]
  if (rule && !routingRules.value[ruleIndex].domain.includes(rule)) {
    routingRules.value[ruleIndex].domain.push(rule)
    customDomainRule.value[ruleIndex] = ''
  }
}

const removeDomainRule = (ruleIndex: number, domainIndex: number) => {
  routingRules.value[ruleIndex].domain.splice(domainIndex, 1)
}

const startUpdate = async () => {
  isUpdating.value = true
  updateProgress.value = 0
  updateStatus.value = ''

  try {
    // 检查是否有可用更新
    updateStatus.value = '检查更新中...'
    const availableUpdate = await invoke('check_xray_update')

    if (!availableUpdate) {
      updateStatus.value = '已是最新版本'
      setTimeout(() => {
        showUpdateDialog.value = false
        isUpdating.value = false
        updateProgress.value = 0
        updateStatus.value = ''
      }, 2000)
      return
    }

    // 监听下载进度事件
    const unlisten = await listen('xray-download-progress', (event: any) => {
      const { progress, message } = event.payload
      updateProgress.value = progress
      updateStatus.value = message
    })

    // 开始下载更新
    await invoke('download_xray_update_with_progress', {
      version: availableUpdate
    })

    // 清理事件监听器
    unlisten()

    // 下载完成后的处理
    setTimeout(() => {
      showUpdateDialog.value = false
      isUpdating.value = false
      updateProgress.value = 0
      updateStatus.value = ''

      // 重新检查 Xray 状态
      checkXrayExists()
    }, 2000)

  } catch (error) {
    console.error('更新失败:', error)
    updateStatus.value = `更新失败: ${error}`
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

    // 更新路由配置
    config.routing_config = {
      domain_strategy: routingDomainStrategy.value,
      rules: routingRules.value.map(rule => ({
        rule_type: rule.type,
        ip: rule.ip,
        domain: rule.domain,
        outbound_tag: rule.outboundTag
      }))
    }

    // 保存配置
    await invoke('save_app_config', { config })
    toast.add({
      title: '保存设置成功',
      description: `保存设置成功`,
      icon: 'i-heroicons-pencil',
      color: 'green'
    })

    showSettings.value = false
  } catch (error) {
    toast.add({
      title: '保存设置失败',
      description: `保存设置失败: ${error}`,
      icon: 'i-heroicons-pencil',
      color: 'red'
    })
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

const clearXrayPath = async () => {
  xrayPath.value = ''
  await checkXrayExists()
}

const getDefaultXrayPath = async () => {
  try {
    defaultXrayPath.value = await invoke('get_xray_path')
  } catch (error) {
    console.error('获取默认 Xray 路径失败:', error)
    defaultXrayPath.value = ''
  }
}

const openSettings = async () => {
  showSettings.value = true
  await loadSettings()
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

const checkGeoFilesExist = async () => {
  try {
    geoFilesExist.value = await invoke('check_geo_files_exist')
  } catch (error) {
    console.error('检查地理位置数据文件失败:', error)
    geoFilesExist.value = false
  }
}

const downloadGeoFiles = async () => {
  try {
    isDownloadingGeoFiles.value = true
    setupProgress.value = 0
    setupStatus.value = ''

    // 监听下载进度事件
    const unlisten = await listen('geo-download-progress', (event: any) => {
      const { progress, message } = event.payload
      setupProgress.value = progress
      setupStatus.value = message
    })

    // 开始下载地理位置数据文件
    await invoke('download_geo_files')

    // 清理事件监听器
    unlisten()

    // 重新检查文件状态
    await checkGeoFilesExist()

    setupProgress.value = 0
    setupStatus.value = ''

  } catch (error) {
    console.error('下载地理位置数据文件失败:', error)
    setupStatus.value = `下载失败: ${error}`
  } finally {
    isDownloadingGeoFiles.value = false
  }
}

const ensureAllXrayFiles = async () => {
  try {
    isEnsuring.value = true
    setupProgress.value = 0
    setupStatus.value = ''

    // 监听设置进度事件
    const unlisten = await listen('xray-setup-progress', (event: any) => {
      const { progress, message } = event.payload
      setupProgress.value = progress
      setupStatus.value = message
    })

    // 确保所有 Xray 文件都存在
    await invoke('ensure_xray_files')

    // 清理事件监听器
    unlisten()

    // 重新检查所有状态
    await checkXrayExists()
    await checkGeoFilesExist()

    setupProgress.value = 0
    setupStatus.value = ''

  } catch (error) {
    console.error('设置 Xray 文件失败:', error)
    setupStatus.value = `设置失败: ${error}`
  } finally {
    isEnsuring.value = false
  }
}

const loadSettings = async () => {
  try {
    // 加载应用配置
    const config = await invoke('get_app_config') as any
    xrayPath.value = config.xray_path || ''
    httpPort.value = config.http_port || 10086
    socksPort.value = config.socks_port || 10087
    autoConnect.value = config.auto_connect || false
    selectedThemeColor.value = config.theme_color || 'green'

    // 加载 inbound 配置
    inboundSniffingEnabled.value = config.inbound_sniffing_enabled || false
    inboundUdpEnabled.value = config.inbound_udp_enabled || false
    inboundAllowTransparent.value = config.inbound_allow_transparent || false
    inboundAuthMethod.value = config.inbound_auth_method || 'noauth'

    // 加载路由配置
    if (config.routing_config) {
      routingDomainStrategy.value = config.routing_config.domain_strategy || 'AsIs'
      routingRules.value = config.routing_config.rules?.map(rule => ({
        type: rule.rule_type || 'field',
        ip: rule.ip || [],
        domain: rule.domain || [],
        outboundTag: rule.outbound_tag || 'proxy'
      })) || [{
        type: 'field',
        ip: ['geoip:private'],
        domain: [],
        outboundTag: 'direct'
      }]
    }

    // 应用主题色
    setThemeColor(selectedThemeColor.value)

    // 获取默认 Xray 路径
    await getDefaultXrayPath()

    // 检查 Xray Core 是否存在
    await checkXrayExists()

    // 检查地理位置数据文件是否存在
    await checkGeoFilesExist()
  } catch (error) {
    console.error('加载设置失败:', error)
  }
}

// 组件挂载时加载设置
onMounted(() => {
  loadSettings()
})
</script>
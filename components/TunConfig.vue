<template>
  <div class="h-full flex flex-col bg-white dark:bg-gray-800">
    <!-- 头部 -->
    <div class="p-4 border-b border-gray-200 dark:border-gray-700">
      <div class="flex items-center justify-between mb-3">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">TUN配置</h2>
        <UButton
          size="sm"
          @click="resetToDefault"
          :color="selectedThemeColor"
          variant="outline"
        >
          <Icon name="heroicons:arrow-path" class="w-4 h-4 mr-1" />
          重置默认
        </UButton>
      </div>
      
      <!-- TUN状态显示 -->
      <div class="mb-4 p-3 bg-gray-50 dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-3">
            <UIcon 
              :name="tunStatus.is_running ? 'i-heroicons-signal' : 'i-heroicons-signal-slash'" 
              :class="tunStatus.is_running ? 'text-green-500' : 'text-gray-400'"
              class="w-5 h-5"
            />
            <div>
              <h3 class="text-sm font-medium text-gray-900 dark:text-white">TUN设备状态</h3>
              <p class="text-xs text-gray-500 dark:text-gray-400">
                {{ tunStatus.is_running ? '运行中' : '未运行' }}
                <span v-if="tunStatus.is_running && tunStatus.device_name">
                  - {{ tunStatus.device_name }}
                </span>
                <span v-if="tunStatus.is_running && tunStatus.ip_address">
                  ({{ tunStatus.ip_address }})
                </span>
              </p>
            </div>
          </div>
          <div class="flex items-center gap-2">
            <UBadge 
              :color="tunStatus.is_running ? 'green' : 'gray'"
              variant="soft"
              size="xs"
            >
              {{ tunStatus.is_running ? '运行' : '停止' }}
            </UBadge>
            <UButton
              icon="i-heroicons-arrow-path"
              size="xs"
              :color="selectedThemeColor"
              variant="ghost"
              @click="refreshTunStatus"
              :loading="statusLoading"
            />
          </div>
        </div>
      </div>
    </div>
    
    <!-- 配置表单 -->
    <div class="flex-1 overflow-y-auto p-4">
      <UForm :state="tunConfig" @submit="saveTunConfig" class="space-y-6">
        <!-- 基础配置 -->
        <UCard>
          <template #header>
            <h3 class="text-base font-semibold text-gray-900 dark:text-white">基础配置</h3>
          </template>
          
          <div class="space-y-4">
            <!-- 设备名称 -->
            <UFormGroup label="设备名称" name="name" help="TUN虚拟网卡的名称">
              <UInput
                v-model="tunConfig.name"
                placeholder="ruray-tun"
                :disabled="tunStatus.is_running"
              />
            </UFormGroup>
            
            <!-- IP地址 -->
            <UFormGroup label="IP地址" name="address" help="TUN设备的IP地址" required>
              <UInput
                v-model="tunConfig.address"
                placeholder="192.168.55.10"
                :disabled="tunStatus.is_running"
                @blur="validateIpAddress"
                :error="addressError"
              />
              <template #error v-if="addressError">
                <span class="text-red-500 text-xs">{{ addressError }}</span>
              </template>
            </UFormGroup>
            
            <!-- 子网掩码 -->
            <UFormGroup label="子网掩码" name="netmask" help="TUN设备的子网掩码" required>
              <UInput
                v-model="tunConfig.netmask"
                placeholder="255.255.255.0"
                :disabled="tunStatus.is_running"
                @blur="validateNetmask"
                :error="netmaskError"
              />
              <template #error v-if="netmaskError">
                <span class="text-red-500 text-xs">{{ netmaskError }}</span>
              </template>
            </UFormGroup>
            
            <!-- MTU大小 -->
            <UFormGroup label="MTU大小" name="mtu" help="最大传输单元大小">
              <UInput
                v-model.number="tunConfig.mtu"
                type="number"
                placeholder="1500"
                :min="576"
                :max="9000"
                :disabled="tunStatus.is_running"
              />
            </UFormGroup>
          </div>
        </UCard>
        
        <!-- 路由配置 -->
        <UCard>
          <template #header>
            <h3 class="text-base font-semibold text-gray-900 dark:text-white">路由配置</h3>
          </template>
          
          <div class="space-y-4">
            <!-- 网关地址 -->
            <UFormGroup label="网关地址" name="gateway" help="默认路由的网关地址">
              <UInput
                v-model="tunConfig.gateway"
                placeholder="192.168.55.1"
                :disabled="tunStatus.is_running"
                @blur="validateGateway"
                :error="gatewayError"
              />
              <template #error v-if="gatewayError">
                <span class="text-red-500 text-xs">{{ gatewayError }}</span>
              </template>
            </UFormGroup>
            
            <!-- 路由优先级 -->
            <UFormGroup label="路由优先级" name="metric" help="路由表中的优先级，数值越小优先级越高">
              <UInput
                v-model.number="tunConfig.metric"
                type="number"
                placeholder="1"
                :min="1"
                :max="9999"
                :disabled="tunStatus.is_running"
              />
            </UFormGroup>
          </div>
        </UCard>
        
        <!-- 操作按钮 -->
        <div class="flex justify-end space-x-3 pt-4 border-t border-gray-200 dark:border-gray-700">
          <UButton
            type="button"
            :color="selectedThemeColor"
            variant="outline"
            @click="loadTunConfig"
            :disabled="configLoading"
          >
            重新加载
          </UButton>
          <UButton
            type="submit"
            :color="selectedThemeColor"
            :loading="configLoading"
            :disabled="tunStatus.is_running || !isConfigValid"
          >
            保存配置
          </UButton>
        </div>
      </UForm>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

// 获取应用配置以访问主题色
const appConfig = useAppConfig()
const selectedThemeColor = computed(() => appConfig.ui?.primary || 'green')

/**
 * TUN配置接口
 */
interface TunConfig {
  name: string
  address: string
  netmask: string
  mtu: number
  gateway: string
  metric: number
  enabled: boolean
}

/**
 * TUN状态接口
 */
interface TunStatus {
  is_running: boolean
  device_name: string
  ip_address: string
  bytes_received: number
  bytes_sent: number
  error?: string
}

// 响应式数据
const tunConfig = reactive<TunConfig>({
  name: 'ruray-tun',
  address: '192.168.55.10',
  netmask: '255.255.255.0',
  mtu: 1500,
  gateway: '192.168.55.1',
  metric: 1,
  enabled: false
})

const tunStatus = reactive<TunStatus>({
  is_running: false,
  device_name: '',
  ip_address: '',
  bytes_received: 0,
  bytes_sent: 0
})

// 加载状态
const configLoading = ref(false)
const statusLoading = ref(false)

// 验证错误
const addressError = ref('')
const netmaskError = ref('')
const gatewayError = ref('')

/**
 * 验证IP地址格式
 */
const validateIpAddress = () => {
  const ipRegex = /^(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$/
  if (!tunConfig.address) {
    addressError.value = 'IP地址不能为空'
  } else if (!ipRegex.test(tunConfig.address)) {
    addressError.value = '请输入有效的IP地址'
  } else {
    addressError.value = ''
  }
}

/**
 * 验证子网掩码格式
 */
const validateNetmask = () => {
  const netmaskRegex = /^(?:(?:255\.){3}(?:255|254|252|248|240|224|192|128|0))|(?:(?:255\.){2}(?:255|254|252|248|240|224|192|128|0)\.0)|(?:(?:255\.){1}(?:255|254|252|248|240|224|192|128|0)\.0\.0)|(?:(?:255|254|252|248|240|224|192|128|0)\.0\.0\.0)$/
  if (!tunConfig.netmask) {
    netmaskError.value = '子网掩码不能为空'
  } else if (!netmaskRegex.test(tunConfig.netmask)) {
    netmaskError.value = '请输入有效的子网掩码'
  } else {
    netmaskError.value = ''
  }
}

/**
 * 验证网关地址格式
 */
const validateGateway = () => {
  const ipRegex = /^(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$/
  if (!tunConfig.gateway) {
    gatewayError.value = '网关地址不能为空'
  } else if (!ipRegex.test(tunConfig.gateway)) {
    gatewayError.value = '请输入有效的网关地址'
  } else {
    gatewayError.value = ''
  }
}

/**
 * 检查配置是否有效
 */
const isConfigValid = computed(() => {
  return tunConfig.address && 
         tunConfig.netmask && 
         tunConfig.gateway &&
         !addressError.value && 
         !netmaskError.value && 
         !gatewayError.value
})

/**
 * 加载TUN配置
 */
const loadTunConfig = async () => {
  configLoading.value = true
  try {
    const config = await invoke('get_tun_config') as TunConfig
    Object.assign(tunConfig, config)
  } catch (error) {
    console.error('加载TUN配置失败:', error)
    const toast = useToast()
    toast.add({
      title: '加载失败',
      description: `无法加载TUN配置: ${error}`,
      icon: 'i-heroicons-exclamation-triangle',
      color: 'red'
    })
  } finally {
    configLoading.value = false
  }
}

/**
 * 保存TUN配置
 */
const saveTunConfig = async () => {
  // 验证所有字段
  validateIpAddress()
  validateNetmask()
  validateGateway()
  
  if (!isConfigValid.value) {
    const toast = useToast()
    toast.add({
      title: '配置无效',
      description: '请检查并修正配置错误',
      icon: 'i-heroicons-exclamation-triangle',
      color: 'orange'
    })
    return
  }
  
  configLoading.value = true
  try {
    await invoke('save_tun_config', { config: tunConfig })
    
    const toast = useToast()
    toast.add({
      title: '保存成功',
      description: 'TUN配置已保存',
      icon: 'i-heroicons-check-circle',
      color: 'green'
    })
  } catch (error) {
    console.error('保存TUN配置失败:', error)
    const toast = useToast()
    toast.add({
      title: '保存失败',
      description: `无法保存TUN配置: ${error}`,
      icon: 'i-heroicons-exclamation-triangle',
      color: 'red'
    })
  } finally {
    configLoading.value = false
  }
}

/**
 * 刷新TUN状态
 */
const refreshTunStatus = async () => {
  statusLoading.value = true
  try {
    const status = await invoke('get_tun_status') as TunStatus
    Object.assign(tunStatus, status)
  } catch (error) {
    console.error('获取TUN状态失败:', error)
  } finally {
    statusLoading.value = false
  }
}

/**
 * 重置为默认配置
 */
const resetToDefault = () => {
  Object.assign(tunConfig, {
    name: 'ruray-tun',
    address: '192.168.55.10',
    netmask: '255.255.255.0',
    mtu: 1500,
    gateway: '192.168.55.1',
    metric: 1,
    enabled: false
  })
  
  // 清除验证错误
  addressError.value = ''
  netmaskError.value = ''
  gatewayError.value = ''
}

// 组件挂载时加载配置和状态
onMounted(async () => {
  await loadTunConfig()
  await refreshTunStatus()
})
</script>

<style scoped>
.server-item {
  transition: all 0.2s ease-in-out;
}

.server-item:hover {
  transform: translateY(-1px);
}

.status-indicator {
  position: relative;
}

.status-indicator::before {
  content: '';
  position: absolute;
  left: -12px;
  top: 50%;
  transform: translateY(-50%);
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background-color: currentColor;
}
</style>
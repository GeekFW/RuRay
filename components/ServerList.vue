<template>
  <div class="h-full flex flex-col bg-white dark:bg-gray-800">
    <!-- 头部 -->
    <div class="p-4 border-b border-gray-200 dark:border-gray-700">
      <div class="flex items-center justify-between mb-3">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">服务器列表</h2>
        <UButton
          size="sm"
          @click="showAddServer = true"
          :color="selectedThemeColor"
        >
          <Icon name="heroicons:plus" class="w-4 h-4 mr-1" />
          添加
        </UButton>
      </div>
      
      <!-- 系统代理状态 -->
      <div class="mb-4 p-3 bg-gray-50 dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-3">
            <UIcon 
              :name="systemProxyStatus.enabled ? 'i-heroicons-globe-alt' : 'i-heroicons-globe-alt'" 
              :class="systemProxyStatus.enabled ? 'text-green-500' : 'text-gray-400'"
              class="w-5 h-5"
            />
            <div>
              <h3 class="text-sm font-medium text-gray-900 dark:text-white">系统代理状态</h3>
              <p class="text-xs text-gray-500 dark:text-gray-400">
                {{ systemProxyStatus.enabled ? '已启用' : '未启用' }}
                <span v-if="systemProxyStatus.enabled && systemProxyStatus.server">
                  - {{ systemProxyStatus.server }}
                </span>
                <span v-if="systemProxyStatus.enabled && systemProxyStatus.type">
                  ({{ systemProxyStatus.type.toUpperCase() }})
                </span>
              </p>
            </div>
          </div>
          <div class="flex items-center gap-2">
            <UBadge 
              :color="systemProxyStatus.enabled ? 'green' : 'gray'"
              variant="soft"
              size="xs"
            >
              {{ systemProxyStatus.enabled ? '启用' : '禁用' }}
            </UBadge>
            <UButton
              icon="i-heroicons-arrow-path"
              size="xs"
              :color="selectedThemeColor"
              variant="ghost"
              @click="refreshSystemProxyStatus"
              :loading="systemProxyStatusLoading"
            />
          </div>
        </div>
      </div>
      
      <!-- 代理模式切换 -->
      <div class="space-y-2">
        <label class="text-sm font-medium text-gray-700 dark:text-gray-300">代理模式</label>
        <USelectMenu
          v-model="proxyMode"
          value-attribute="value"
          :options="proxyModeOptions"
          @change="changeProxyMode"
        />
      </div>
    </div>
    
    <!-- 服务器列表 -->
    <div class="flex-1 overflow-y-auto p-4 space-y-3">
      <div
        v-for="(server, index) in servers"
        :key="server.id"
        :class="[
          'server-item p-4 rounded-lg border cursor-pointer transition-all',
          server.id === runningServerId 
            ? 'border-green-500 bg-green-50 dark:bg-green-900/20 shadow-md' 
            : server.id === activeServerId
            ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/20'
            : 'border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600'
        ]"
        @click="selectServer(server.id)"
        draggable="true"
        @dragstart="handleDragStart(index)"
        @dragover.prevent
        @drop="handleDrop(index)"
      >
        <!-- 服务器信息 -->
        <div class="space-y-1">
          <!-- 第一行：服务器名称和协议类型 -->
          <div class="flex items-center space-x-3 mb-1">
            <div :class="['status-indicator', getStatusClass(server.status)]" class="flex-1 min-w-0">
              <span 
                class="text-sm font-medium truncate block max-w-[150px] sm:max-w-[200px] md:max-w-[250px]" 
                :title="server.name"
              >
                {{ server.name }}
              </span>
              <!-- 运行状态指示器 -->
              <span v-if="server.id === runningServerId" class="ml-2 inline-flex items-center">
                <span class="w-2 h-2 bg-green-500 rounded-full animate-pulse"></span>
                <span class="ml-1 text-xs text-green-600 dark:text-green-400 font-medium">运行中</span>
              </span>
            </div>
            <UBadge
              :color="getProtocolColor(server.protocol)"
              variant="soft"
              size="xs"
            >
              {{ server.protocol.toUpperCase() }}
            </UBadge>
          </div>
          
          <!-- 第二行：操作按钮 -->
          <div class="flex items-center space-x-0.5 mb-1">
            <!-- 激活/停止按钮 -->
            <UButton
              variant="ghost"
              size="xs"
              :color="getActivationButtonColor(server.id)"
              @click.stop="toggleServerActivation(server.id)"
              :loading="server.activating"
              :title="getActivationButtonTitle(server.id)"
            >
              <Icon v-if="!server.activating" :name="getActivationButtonIcon(server.id)" class="w-4 h-4" />
            </UButton>
            
            <!-- 测试连接 -->
            <UButton
              variant="ghost"
              size="xs"
              @click.stop="testConnection(server.id)"
              :loading="server.testing"
              title="测试连接"
            >
              <Icon name="heroicons:signal" class="w-4 h-4" />
            </UButton>
            
            <!-- 测试配置 -->
            <UButton
              variant="ghost"
              size="xs"
              @click.stop="testConfig(server.id)"
              :loading="server.configTesting"
              title="测试配置"
            >
              <Icon name="heroicons:cog-6-tooth" class="w-4 h-4" />
            </UButton>
            
            <!-- 编辑 -->
            <UButton
              variant="ghost"
              size="xs"
              @click.stop="editServer(server)"
              title="编辑服务器"
            >
              <Icon name="heroicons:pencil" class="w-4 h-4" />
            </UButton>
            
            <!-- 打开配置文件 -->
            <UButton
              variant="ghost"
              size="xs"
              @click.stop="openConfigFile(server)"
              title="打开配置文件"
            >
              <Icon name="heroicons:folder-open" class="w-4 h-4" />
            </UButton>
            
            <!-- 刷新配置 -->
            <UButton
              variant="ghost"
              size="xs"
              @click.stop="regenerateConfig(server?.id)"
              :loading="server.regenerating"
              title="重新生成配置文件"
            >
              <Icon name="heroicons:arrow-path" class="w-4 h-4" />
            </UButton>
            
            <!-- 删除 -->
            <UButton
              variant="ghost"
              size="xs"
              color="red"
              @click.stop="deleteServer(server.id)"
              title="删除服务器"
            >
              <Icon name="heroicons:trash" class="w-4 h-4" />
            </UButton>
          </div>
        </div>
        
        <!-- 服务器详情 -->
        <div class="text-sm text-gray-600 dark:text-gray-400 space-y-1">
          <div class="flex justify-between">
            <span>地址:</span>
            <span class="font-mono">{{ server.address }}:{{ server.port }}</span>
          </div>
          <div v-if="server.ping" class="flex justify-between">
            <span>延迟:</span>
            <span :class="getPingColor(server.ping)">{{ server.ping }}ms</span>
          </div>
        </div>
      </div>
      
      <!-- 空状态 -->
      <div v-if="servers.length === 0" class="text-center py-12">
        <Icon name="heroicons:server" class="w-12 h-12 text-gray-400 mx-auto mb-4" />
        <p class="text-gray-500 dark:text-gray-400 mb-4">暂无服务器配置</p>
        <UButton @click="showAddServer = true" :color="selectedThemeColor">
          <Icon name="heroicons:plus" class="w-4 h-4 mr-1" />
          添加第一个服务器
        </UButton>
      </div>
    </div>
    
    <!-- 添加/编辑服务器对话框 -->
    <UModal v-model="showAddServer">
      <UCard>
        <template #header>
          <div class="flex items-center space-x-2">
            <Icon name="heroicons:server" class="w-5 h-5 text-green-500" />
            <span>{{ editingServer ? '编辑服务器' : '添加服务器' }}</span>
          </div>
        </template>
        
        <UForm :state="serverForm" @submit="saveServer" class="space-y-4">
          <UFormGroup label="服务器名称" required>
            <UInput v-model="serverForm.name" placeholder="输入服务器名称" />
          </UFormGroup>
          
          <UFormGroup label="协议类型" required>
            <USelectMenu
              v-model="serverForm.protocol"
              value-attribute="value"
              :options="protocolOptions"
              @change="onProtocolChange"
            />
          </UFormGroup>
          
          <div class="grid grid-cols-2 gap-4">
            <UFormGroup label="服务器地址" required>
              <UInput v-model="serverForm.address" placeholder="example.com" />
            </UFormGroup>
            
            <UFormGroup label="端口" required>
              <UInput v-model="serverForm.port" type="number" placeholder="443" />
            </UFormGroup>
          </div>
          
          <!-- VMESS/VLESS 配置 -->
          <template v-if="['vmess', 'vless'].includes(serverForm.protocol)">
            <UFormGroup label="用户 ID" required>
              <UInput v-model="serverForm.uuid" placeholder="UUID" />
            </UFormGroup>
            
            <div class="grid grid-cols-2 gap-4">
              <UFormGroup label="加密方式">
                <USelectMenu
                  v-model="serverForm.security"
                  value-attribute="value"
                  :options="securityOptions"
                />
              </UFormGroup>
              
              <UFormGroup label="传输协议">
                <USelectMenu
                  v-model="serverForm.network"
                  value-attribute="value"
                  :options="networkOptions"
                />
              </UFormGroup>
            </div>
            
            <UFormGroup v-if="serverForm.network === 'ws'" label="WebSocket 路径">
              <UInput v-model="serverForm.path" placeholder="/path" />
            </UFormGroup>
          </template>
          
          <!-- Trojan 配置 -->
          <template v-if="serverForm.protocol === 'trojan'">
            <UFormGroup label="密码" required>
              <UInput v-model="serverForm.password" type="password" placeholder="密码" />
            </UFormGroup>
            
            <!-- 网络类型 -->
            <UFormGroup label="网络类型">
              <USelect v-model="serverForm.network" :options="networkOptions" />
            </UFormGroup>
            
            <!-- WebSocket 路径 -->
            <UFormGroup v-if="serverForm.network === 'ws'" label="WebSocket 路径">
              <UInput v-model="serverForm.path" placeholder="/path" />
            </UFormGroup>
            
            <!-- HTTP/2 路径 -->
            <UFormGroup v-if="serverForm.network === 'h2'" label="HTTP/2 路径">
              <UInput v-model="serverForm.path" placeholder="/path" />
            </UFormGroup>
            
            <!-- gRPC 服务名 -->
            <UFormGroup v-if="serverForm.network === 'grpc'" label="gRPC 服务名">
              <UInput v-model="serverForm.serviceName" placeholder="serviceName" />
            </UFormGroup>
            
            <!-- Host 头 -->
            <UFormGroup v-if="['ws', 'h2'].includes(serverForm.network)" label="Host 头">
              <UInput v-model="serverForm.host" placeholder="example.com" />
            </UFormGroup>
          </template>
          
          <!-- Socks5/HTTP 配置 -->
          <template v-if="['socks5', 'http'].includes(serverForm.protocol)">
            <div class="grid grid-cols-2 gap-4">
              <UFormGroup label="用户名">
                <UInput v-model="serverForm.username" placeholder="用户名（可选）" />
              </UFormGroup>
              
              <UFormGroup label="密码">
                <UInput v-model="serverForm.password" type="password" placeholder="密码（可选）" />
              </UFormGroup>
            </div>
          </template>
          
          <!-- TLS 设置 -->
          <UFormGroup v-if="['vmess', 'vless', 'trojan'].includes(serverForm.protocol)">
            <div class="flex items-center space-x-2">
              <UCheckbox v-model="serverForm.tls" />
              <label>启用 TLS</label>
            </div>
          </UFormGroup>
          
          <!-- TLS 高级设置 -->
          <template v-if="serverForm.tls">
            <UFormGroup label="SNI (Server Name Indication)">
              <UInput v-model="serverForm.sni" placeholder="example.com" />
            </UFormGroup>
            
            <UFormGroup label="ALPN">
              <UInput v-model="serverForm.alpnString" placeholder="h2,http/1.1" />
              <template #help>
                <span class="text-xs text-gray-500">多个值用逗号分隔，如: h2,http/1.1</span>
              </template>
            </UFormGroup>
            
            <UFormGroup label="Fingerprint">
              <USelect v-model="serverForm.fingerprint" :options="fingerprintOptions" />
            </UFormGroup>
          </template>
          
          <!-- Mux 设置 -->
          <UFormGroup v-if="['vmess', 'vless', 'trojan'].includes(serverForm.protocol)">
            <div class="flex items-center space-x-2">
              <UCheckbox v-model="serverForm.mux" />
              <label>启用 Mux 多路复用</label>
            </div>
          </UFormGroup>
        </UForm>
        
        <template #footer>
          <div class="flex justify-end space-x-2">
            <UButton variant="ghost" @click="cancelEdit">
              取消
            </UButton>
            <UButton @click="saveServer" :color="selectedThemeColor">
              {{ editingServer ? '保存' : '添加' }}
            </UButton>
          </div>
        </template>
      </UCard>
    </UModal>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

// 全局 composables
const toast = useToast()

// 获取应用配置以访问主题色
const appConfig = useAppConfig()
const selectedThemeColor = computed(() => appConfig.ui?.primary || 'green')

// 接口定义
interface Server {
  id: string
  name: string
  protocol: string
  address: string
  port: number
  uuid?: string
  password?: string
  username?: string
  security?: string
  network?: string
  path?: string
  host?: string
  serviceName?: string
  tls?: boolean
  sni?: string
  alpnString?: string
  fingerprint?: string
  mux?: boolean
  status: 'connected' | 'connecting' | 'disconnected'
  ping?: number
  testing?: boolean
  configTesting?: boolean
  activating?: boolean
  regenerating?: boolean
}

// 状态
const servers = ref<Server[]>([])

const activeServerId = ref<string | null>(null)
const runningServerId = ref<string | null>(null) // 当前运行的服务器ID
const proxyMode = ref('direct')
const showAddServer = ref(false)
const editingServer = ref<Server | null>(null)
const draggedIndex = ref<number | null>(null)

// 系统代理状态
const systemProxyStatus = ref({
  enabled: false,
  server: '',
  type: ''
})
const systemProxyStatusLoading = ref(false)

// 表单数据
const serverForm = reactive({
  name: '',
  protocol: 'vmess',
  address: '',
  port: 443,
  uuid: '',
  password: '',
  username: '',
  security: 'auto',
  network: 'tcp',
  path: '',
  host: '',
  serviceName: '',
  tls: false,
  sni: '',
  alpnString: 'h2,http/1.1',
  fingerprint: 'chrome',
  mux: false
})

// 选项
const proxyModeOptions = [
  { label: '直连模式', value: 'direct' },
  { label: '全局代理', value: 'global' },
  { label: 'PAC 模式', value: 'pac' }
]

const protocolOptions = [
  { label: 'VMess', value: 'vmess' },
  { label: 'VLESS', value: 'vless' },
  { label: 'Trojan', value: 'trojan' },
  { label: 'Socks5', value: 'socks5' },
  { label: 'HTTP', value: 'http' }
]

const securityOptions = [
  { label: 'Auto', value: 'auto' },
  { label: 'AES-128-GCM', value: 'aes-128-gcm' },
  { label: 'ChaCha20-Poly1305', value: 'chacha20-poly1305' },
  { label: 'None', value: 'none' }
]

const networkOptions = [
  { label: 'TCP', value: 'tcp' },
  { label: 'WebSocket', value: 'ws' },
  { label: 'HTTP/2', value: 'h2' },
  { label: 'gRPC', value: 'grpc' }
]

const fingerprintOptions = [
  { label: 'Chrome', value: 'chrome' },
  { label: 'Firefox', value: 'firefox' },
  { label: 'Safari', value: 'safari' },
  { label: 'iOS', value: 'ios' },
  { label: 'Android', value: 'android' },
  { label: 'Edge', value: 'edge' },
  { label: '360', value: '360' },
  { label: 'QQ', value: 'qq' },
  { label: 'Random', value: 'random' },
  { label: 'Randomized', value: 'randomized' }
]

// 方法
const getStatusClass = (status: string) => {
  switch (status) {
    case 'connected': return 'connected'
    case 'connecting': return 'connecting'
    default: return 'disconnected'
  }
}

const getProtocolColor = (protocol: string) => {
  const colors: Record<string, string> = {
    vmess: 'blue',
    vless: 'green',
    trojan: 'purple',
    socks5: 'orange',
    http: 'gray'
  }
  return colors[protocol] || 'gray'
}

const getPingColor = (ping: number) => {
  if (ping < 100) return 'text-green-500'
  if (ping < 300) return 'text-yellow-500'
  return 'text-red-500'
}

const selectServer = (serverId: string) => {
  activeServerId.value = serverId
  // TODO: 连接到选中的服务器
}

const testConnection = async (serverId: string) => {
  const server = servers.value.find(s => s.id === serverId)
  if (!server) return
  
  server.testing = true
  try {
    const result = await invoke('test_server_connection', { serverId: serverId }) as any
    server.ping = result.ping || 0
    
    // 显示测试结果通知
    if (result.success) {
      toast.add({
        title: '连接测试成功',
        description: `服务器 "${server.name}" 延迟: ${result.ping}ms`,
        icon: 'i-heroicons-signal',
        color: 'green'
      })
    } else {
      toast.add({
        title: '连接测试失败',
        description: `服务器 "${server.name}" 连接失败`,
        icon: 'i-heroicons-signal-slash',
        color: 'red'
      })
    }
  } catch (error) {
    console.error('测试连接失败:', error)
    
    // 显示错误通知
    toast.add({
      title: '测试失败',
      description: `无法测试服务器连接: ${error}`,
      icon: 'i-heroicons-exclamation-triangle',
      color: 'red'
    })
  } finally {
    server.testing = false
  }
}

const testConfig = async (serverId: string) => {
  const server = servers.value.find(s => s.id === serverId)
  if (!server) return
  
  server.configTesting = true
  try {
    const result = await invoke('test_xray_config', { serverId: serverId }) as string
    
    // 显示测试结果通知 - 如果没有抛出异常，说明配置有效
    toast.add({
      title: '配置测试成功',
      description: `服务器 "${server.name}" 配置有效`,
      icon: 'i-heroicons-check-circle',
      color: 'green'
    })
  } catch (error) {
    console.error('测试配置失败:', error)
    
    // 显示错误通知
    toast.add({
      title: '配置测试失败',
      description: `服务器 "${server.name}" 配置错误: ${error}`,
      icon: 'i-heroicons-exclamation-triangle',
      color: 'red'
    })
  } finally {
    server.configTesting = false
  }
}

const editServer = (server: Server) => {
  editingServer.value = server
  Object.assign(serverForm, server)
  showAddServer.value = true
}

const deleteServer = async (serverId: string) => {
  try {
    await invoke('delete_server', { serverId: serverId })
    
    const index = servers.value.findIndex(s => s.id === serverId)
    if (index > -1) {
      const serverName = servers.value[index].name
      servers.value.splice(index, 1)
      
      if (activeServerId.value === serverId) {
        activeServerId.value = null
      }
      
      if (runningServerId.value === serverId) {
        runningServerId.value = null
      }
      
      // 显示成功通知
      toast.add({
        title: '服务器已删除',
        description: `服务器 "${serverName}" 已成功删除`,
        icon: 'i-heroicons-trash',
        color: 'orange'
      })
    }
  } catch (error) {
    console.error('删除服务器失败:', error)
    
    // 显示错误通知
    toast.add({
      title: '删除失败',
      description: `无法删除服务器: ${error}`,
      icon: 'i-heroicons-exclamation-triangle',
      color: 'red'
    })
  }
}

// 打开配置文件
const openConfigFile = async (server: Server) => {
  try {
    await invoke('open_server_config_file', { 
      serverId: server.id,
      serverName: server.name 
    })
    
    // 显示成功通知
    toast.add({
      title: '配置文件已打开',
      description: `已打开服务器 "${server.name}" 的配置文件`,
      icon: 'i-heroicons-folder-open',
      color: 'green'
    })
  } catch (error) {
    console.error('打开配置文件失败:', error)
    
    // 显示错误通知
    toast.add({
      title: '打开失败',
      description: `无法打开配置文件: ${error}`,
      icon: 'i-heroicons-exclamation-triangle',
      color: 'red'
    })
  }
}

const onProtocolChange = (e: any) => {
  console.log('Protocol change event:', e);
  
  // 处理不同的事件格式
  let protocol: string;
  if (typeof e === 'string') {
    protocol = e;
  } else if (e && typeof e === 'object' && e.value) {
    protocol = e.value;
  } else {
    console.error('Unexpected protocol change event format:', e);
    return;
  }
  
  console.log('Setting protocol to:', protocol);
  serverForm.protocol = protocol;
  
  // 根据协议类型设置默认端口
  const defaultPorts: Record<string, number> = {
    vmess: 443,
    vless: 443,
    trojan: 443,
    socks5: 1080,
    http: 8080
  }
  
  // 只在当前端口是默认端口或为空时才更新端口
  const currentDefaultPort = Object.values(defaultPorts).includes(serverForm.port) || !serverForm.port
  if (currentDefaultPort) {
    serverForm.port = defaultPorts[serverForm.protocol] || 443
  }
  
  // 根据协议类型智能重置相关字段
  if (serverForm.protocol === 'vmess' || serverForm.protocol === 'vless') {
    // VMess/VLESS: 保留 UUID、security、network 等字段，清空不相关的字段
    serverForm.password = ''
    serverForm.username = ''
    // 保持 uuid, security, network, path, tls, sni 不变
    if (!serverForm.uuid) serverForm.uuid = ''
    if (!serverForm.security) serverForm.security = 'auto'
    if (!serverForm.network) serverForm.network = 'tcp'
  } else if (serverForm.protocol === 'trojan') {
    // Trojan: 保留密码字段和高级设置，清空不相关的字段
    serverForm.username = ''
    serverForm.uuid = ''
    // 保持 password, network, path, host, serviceName, tls, sni, alpnString, fingerprint, mux 不变
    if (!serverForm.password) serverForm.password = ''
    if (!serverForm.network) serverForm.network = 'tcp'
    if (!serverForm.alpnString) serverForm.alpnString = 'h2,http/1.1'
    if (!serverForm.fingerprint) serverForm.fingerprint = 'chrome'
    // Trojan 不需要 security 设置
    serverForm.security = 'none'
  } else if (serverForm.protocol === 'socks5' || serverForm.protocol === 'http') {
    // Socks5/HTTP: 保留用户名和密码，清空不相关的字段
    serverForm.uuid = ''
    // 保持 username, password 不变
    if (!serverForm.username) serverForm.username = ''
    if (!serverForm.password) serverForm.password = ''
    // 这些协议不支持 TLS 和其他高级设置
    serverForm.security = 'none'
    serverForm.network = 'tcp'
    serverForm.path = ''
    serverForm.tls = false
    serverForm.sni = ''
  }
}

const saveServer = async () => {
  try {
    // 构造配置对象
    const config: Record<string, any> = {}
    
    // 根据协议添加相应的配置字段
    if (serverForm.protocol === 'vmess' || serverForm.protocol === 'vless') {
      if (serverForm.uuid) config.uuid = serverForm.uuid
    }
    
    if (serverForm.protocol === 'trojan' || serverForm.protocol === 'shadowsocks') {
      if (serverForm.password) config.password = serverForm.password
    }
    
    if (serverForm.protocol === 'socks5' || serverForm.protocol === 'http') {
      if (serverForm.username) config.username = serverForm.username
      if (serverForm.password) config.password = serverForm.password
    }
    
    // 通用配置
    if (serverForm.security) config.security = serverForm.security
    if (serverForm.network) config.network = serverForm.network
    if (serverForm.path) config.path = serverForm.path
    if (serverForm.host) config.host = serverForm.host
    if (serverForm.serviceName) config.serviceName = serverForm.serviceName
    if (serverForm.sni) config.sni = serverForm.sni
    if (serverForm.fingerprint) config.fingerprint = serverForm.fingerprint
    config.tls = serverForm.tls || false
    config.mux = serverForm.mux || false
    
    // 处理 ALPN 字符串
    if (serverForm.alpnString && serverForm.tls) {
      const alpnArray = serverForm.alpnString.split(',').map(s => s.trim()).filter(s => s.length > 0)
      if (alpnArray.length > 0) {
        config.alpn = alpnArray
      }
    }
    
    if (editingServer.value) {
      // 编辑现有服务器
      const serverData = {
        id: editingServer.value.id,
        name: serverForm.name,
        protocol: serverForm.protocol,
        address: serverForm.address,
        port: serverForm.port,
        config: config,
        created_at: editingServer.value.created_at || new Date().toISOString(),
        updated_at: new Date().toISOString()
      }
      
      await invoke('update_server', { server: serverData })
      
      // 更新本地数据
      Object.assign(editingServer.value, {
        name: serverForm.name,
        protocol: serverForm.protocol,
        address: serverForm.address,
        port: serverForm.port,
        ...config
      })
      
      // 显示成功通知
      toast.add({
        title: '服务器已更新',
        description: `服务器 "${serverForm.name}" 已成功更新`,
        icon: 'i-heroicons-check-circle',
        color: 'green'
      })
    } else {
      // 添加新服务器
      const serverData = {
        id: '', // 后端会自动生成 UUID
        name: serverForm.name,
        protocol: serverForm.protocol,
        address: serverForm.address,
        port: serverForm.port,
        config: config,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString()
      }
      
      const newServerId = await invoke('add_server', { server: serverData }) as string
      
      // 添加到本地列表
      const newServer: Server = {
        id: newServerId,
        name: serverForm.name,
        protocol: serverForm.protocol,
        address: serverForm.address,
        port: serverForm.port,
        status: 'disconnected',
        ...config
      }
      servers.value.push(newServer)
      
      // 显示成功通知
      toast.add({
        title: '服务器已添加',
        description: `服务器 "${serverForm.name}" 已成功添加`,
        icon: 'i-heroicons-plus-circle',
        color: 'green'
      })
    }
    
    cancelEdit()
  } catch (error) {
    console.error('保存服务器失败:', error)
    
    // 显示错误通知
    toast.add({
      title: '保存失败',
      description: `无法保存服务器: ${error}`,
      icon: 'i-heroicons-exclamation-triangle',
      color: 'red'
    })
  }
}

const cancelEdit = () => {
  showAddServer.value = false
  editingServer.value = null
  Object.assign(serverForm, {
    name: '',
    protocol: 'vmess',
    address: '',
    port: 443,
    uuid: '',
    password: '',
    username: '',
    security: 'auto',
    network: 'tcp',
    path: '',
    host: '',
    serviceName: '',
    tls: false,
    sni: '',
    alpnString: 'h2,http/1.1',
    fingerprint: 'chrome',
    mux: false
  })
}

const changeProxyMode = async (mode: string) => {
  try {
    // 调用后端API设置代理模式
    await invoke('set_proxy_mode', { mode })
    
    // 更新本地状态
    proxyMode.value = mode
    
    // 如果当前有代理在运行，需要重新应用代理设置
    if (runningServerId.value) {
      // 重新启动代理以应用新的模式设置
      await invoke('stop_proxy')
      await invoke('start_proxy', { serverId: runningServerId.value })
    }
    
    // 刷新系统代理状态
    await refreshSystemProxyStatus()
    
    // 发射代理模式变化事件，通知其他组件更新
    try {
      const { emit } = await import('@tauri-apps/api/event')
      await emit('proxy-mode-changed', {
        proxy_mode: mode,
        timestamp: Date.now()
      })
    } catch (error) {
      console.warn('发射代理模式变化事件失败:', error)
    }
    
    toast.add({
      title: '代理模式已切换',
      description: `代理模式已切换至: ${proxyModeOptions.find(opt => opt.value === mode)?.label || mode}`,
      icon: 'i-heroicons-arrow-path',
      color: 'green'
    })
  } catch (error) {
    console.error('切换代理模式失败:', error)
    toast.add({
      title: '切换代理模式失败',
      description: `无法切换代理模式: ${error.message || error}`,
      icon: 'i-heroicons-exclamation-triangle',
      color: 'red'
    })
  }
}

// 刷新系统代理状态
const refreshSystemProxyStatus = async () => {
  systemProxyStatusLoading.value = true
  try {
    const status = await invoke('get_system_proxy_status') as any
    systemProxyStatus.value = {
      enabled: status.enabled || false,
      server: status.server || '',
      type: status.type || ''
    }
  } catch (error) {
    console.error('获取系统代理状态失败:', error)
    systemProxyStatus.value = {
      enabled: false,
      server: '',
      type: ''
    }
  } finally {
    systemProxyStatusLoading.value = false
  }
}

// 拖拽功能
const handleDragStart = (index: number) => {
  draggedIndex.value = index
}

const handleDrop = (dropIndex: number) => {
  if (draggedIndex.value === null) return
  
  const draggedItem = servers.value[draggedIndex.value]
  servers.value.splice(draggedIndex.value, 1)
  servers.value.splice(dropIndex, 0, draggedItem)
  
  draggedIndex.value = null
}

// 激活按钮相关方法
const getActivationButtonIcon = (serverId: string) => {
  if (runningServerId.value === serverId) {
    return 'heroicons:stop-circle'
  }
  return 'heroicons:play-circle'
}

const getActivationButtonColor = (serverId: string) => {
  if (runningServerId.value === serverId) {
    return 'red'
  }
  return 'green'
}

const getActivationButtonTitle = (serverId: string) => {
  if (runningServerId.value === serverId) {
    return '停止代理'
  }
  return '激活服务器'
}

const toggleServerActivation = async (serverId: string) => {
  const server = servers.value.find(s => s.id === serverId)
  if (!server) return
  
  server.activating = true
  
  try {
    if (runningServerId.value === serverId) {
      // 停止当前运行的服务器
      await invoke('stop_proxy')
      runningServerId.value = null
      server.status = 'disconnected'
      
      // 刷新系统代理状态
      await refreshSystemProxyStatus()
      
      // 显示成功通知
      toast.add({
        title: '代理已停止',
        description: `服务器 "${server.name}" 已停止运行`,
        icon: 'i-heroicons-stop-circle',
        color: 'orange'
      })
    } else {
      // 如果有其他服务器在运行，先停止它
      if (runningServerId.value) {
        await invoke('stop_proxy')
        const prevServer = servers.value.find(s => s.id === runningServerId.value)
        if (prevServer) {
          prevServer.status = 'disconnected'
        }
      }
      
      // 启动新的服务器
      await invoke('start_proxy', { serverId: serverId })
      runningServerId.value = serverId
      server.status = 'connected'
      
      // 刷新系统代理状态
      await refreshSystemProxyStatus()
      
      // 显示成功通知
      toast.add({
        title: '代理已启动',
        description: `服务器 "${server.name}" 已成功启动`,
        icon: 'i-heroicons-play-circle',
        color: 'green'
      })
    }
  } catch (error) {
    console.error('切换服务器状态失败:', error)
    
    // 显示错误通知
    toast.add({
      title: '操作失败',
      description: `无法切换服务器状态: ${error}`,
      icon: 'i-heroicons-exclamation-triangle',
      color: 'red'
    })
  } finally {
     server.activating = false
   }
 }

// 重新生成配置文件
const regenerateConfig = async (serverId: string) => {
  const server = servers.value.find(s => s.id === serverId)
  console.log('server', servers.value);
  
  if (!server) return
  
  server.regenerating = true
  
  try {
    await invoke('regenerate_server_config', { serverId: serverId })
    
    // 显示成功通知
    toast.add({
      title: '配置已刷新',
      description: `服务器 "${server.name}" 的配置文件已重新生成`,
      icon: 'i-heroicons-arrow-path',
      color: 'green'
    })
  } catch (error) {
    console.error('重新生成配置失败:', error)
    
    // 显示错误通知
    toast.add({
      title: '刷新失败',
      description: `无法重新生成配置文件: ${error}`,
      icon: 'i-heroicons-exclamation-triangle',
      color: 'red'
    })
  } finally {
    server.regenerating = false
  }
}

 // 加载服务器列表
  const loadServers = async () => {
    try {
      const serverList = await invoke('get_servers') as any[]
      servers.value = serverList.map(server => ({
        id: server.id,
        name: server.name,
        protocol: server.protocol,
        address: server.address,
        port: server.port,
        status: 'disconnected' as const,
        testing: false,
        configTesting: false,
        activating: false,
        regenerating: false,
        // 从 config 对象中提取配置字段
        uuid: server.config?.uuid || '',
        password: server.config?.password || '',
        username: server.config?.username || '',
        security: server.config?.security || 'auto',
        network: server.config?.network || 'tcp',
        path: server.config?.path || '',
        tls: server.config?.tls || false,
        sni: server.config?.sni || '',
        created_at: server.created_at,
        updated_at: server.updated_at
      }))
    } catch (error) {
      console.error('加载服务器列表失败:', error)
      
      // 显示错误通知
      toast.add({
        title: '加载失败',
        description: `无法加载服务器列表: ${error}`,
        icon: 'i-heroicons-exclamation-triangle',
        color: 'red'
      })
    }
  }

  // 初始化代理状态
  const initializeProxyStatus = async () => {
    try {
      const status = await invoke('get_proxy_status') as any
      if (status.is_running && status.current_server) {
        runningServerId.value = status.current_server
        // 更新对应服务器的状态
        const server = servers.value.find(s => s.id === status.current_server)
        if (server) {
          server.status = 'connected'
        }
      }
    } catch (error) {
      console.error('获取代理状态失败:', error)
    }
  }

  // 监听代理状态变化事件
  const handleProxyStatusChange = (event: any) => {
    const { is_running, current_server } = event.payload
    
    // 更新运行中的服务器ID
    runningServerId.value = is_running ? current_server : null
    
    // 更新所有服务器的状态
    servers.value.forEach(server => {
      if (server.id === current_server && is_running) {
        server.status = 'connected'
      } else {
        server.status = 'disconnected'
      }
    })
    
    console.log('代理状态已更新:', { is_running, current_server })
  }

  // 加载代理模式配置
  const loadProxyMode = async () => {
    try {
      const config = await invoke('get_app_config') as any
      if (config && config.proxy_mode) {
        proxyMode.value = config.proxy_mode
      }
    } catch (error) {
      console.error('加载代理模式失败:', error)
      toast.add({
        title: '加载代理模式失败',
        description: `无法加载当前代理模式设置: ${error.message || error}`,
        icon: 'i-heroicons-exclamation-triangle',
        color: 'red'
      })
      // 使用默认值
      proxyMode.value = 'direct'
    }
  }

  // 组件挂载时初始化
  onMounted(async () => {
    await loadServers()
    await initializeProxyStatus()
    await loadProxyMode()
    await refreshSystemProxyStatus()
    
    // 监听代理状态变化事件
    const { listen } = await import('@tauri-apps/api/event')
    const unlisten = await listen('proxy-status-changed', handleProxyStatusChange)
    
    // 组件卸载时清理监听器
    onUnmounted(() => {
      unlisten()
    })
  })
 </script>
<template>
  <div class="fixed inset-0 bg-white dark:bg-gray-900 flex items-center justify-center z-50">
    <div class="text-center">
      <!-- Logo 区域 -->
      <div class="mb-8">
        <div class="w-24 h-24 mx-auto bg-gradient-to-br from-green-400 to-green-600 rounded-2xl flex items-center justify-center shadow-2xl">
          <Icon name="heroicons:bolt" class="w-12 h-12 text-white" />
        </div>
      </div>
      
      <!-- 应用名称 -->
      <h1 class="text-4xl font-bold text-gray-900 dark:text-white mb-2">
        RuRay
      </h1>
      <p class="text-lg text-gray-600 dark:text-gray-400 mb-8">
        Xray Core Desktop Client
      </p>
      
      <!-- 加载动画 -->
      <div class="flex items-center justify-center space-x-2 mb-4">
        <div class="w-3 h-3 bg-green-500 rounded-full animate-bounce" style="animation-delay: 0ms"></div>
        <div class="w-3 h-3 bg-green-500 rounded-full animate-bounce" style="animation-delay: 150ms"></div>
        <div class="w-3 h-3 bg-green-500 rounded-full animate-bounce" style="animation-delay: 300ms"></div>
      </div>
      
      <!-- 加载文本 -->
      <p class="text-sm text-gray-500 dark:text-gray-400 animate-pulse">
        {{ loadingText }}
      </p>
      
      <!-- 进度条 -->
      <div class="w-64 mx-auto mt-6">
        <div class="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2">
          <div 
            class="bg-gradient-to-r from-green-400 to-green-600 h-2 rounded-full transition-all duration-300 ease-out"
            :style="{ width: `${progress}%` }"
          ></div>
        </div>
        <p class="text-xs text-gray-500 dark:text-gray-400 mt-2">{{ progress }}%</p>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'

const progress = ref(0)
const loadingText = ref('正在初始化...')

const loadingSteps = [
  { text: '正在初始化...', duration: 500 },
  { text: '检查 Xray Core...', duration: 400 },
  { text: '加载配置文件...', duration: 300 },
  { text: '准备用户界面...', duration: 400 },
  { text: '启动完成', duration: 400 }
]

onMounted(() => {
  let currentStep = 0
  let currentProgress = 0
  
  const updateProgress = () => {
    if (currentStep < loadingSteps.length) {
      const step = loadingSteps[currentStep]
      loadingText.value = step.text
      
      const targetProgress = ((currentStep + 1) / loadingSteps.length) * 100
      const progressInterval = setInterval(() => {
        currentProgress += 2
        progress.value = Math.min(currentProgress, targetProgress)
        
        if (currentProgress >= targetProgress) {
          clearInterval(progressInterval)
          currentStep++
          
          setTimeout(() => {
            updateProgress()
          }, step.duration)
        }
      }, 20)
    }
  }
  
  updateProgress()
})
</script>
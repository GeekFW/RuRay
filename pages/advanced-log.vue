<template>
  <div class="h-screen w-screen">
    <AdvancedLogViewer />
  </div>
</template>

<script setup lang="ts">
// 设置页面布局为空，因为这是一个独立窗口
definePageMeta({
  layout: false
})

// 确保国际化功能在新窗口中正确初始化
const { locale } = useI18n()

// 设置页面标题
useHead({
  title: 'RuRay - 核心日志查看器'
})
</script>
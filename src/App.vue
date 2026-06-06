<script setup lang="ts">
// AeroSync 主界面
</script>

<template>
  <div class="h-screen w-screen flex flex-col items-center justify-center bg-gray-900 text-white select-none">
    <div class="p-8 rounded-2xl bg-gray-800/50 backdrop-blur-xl border border-gray-700 shadow-2xl flex flex-col items-center">
      <div class="w-20 h-20 mb-6 bg-gradient-to-br from-blue-500 to-indigo-600 rounded-2xl shadow-lg flex items-center justify-center">
        <svg class="w-10 h-10 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4"></path>
        </svg>
      </div>
      <h1 class="text-3xl font-bold mb-2 tracking-tight">AeroSync</h1>
      <p class="text-gray-400">现代化的高性能 Syncthing 客户端</p>

      <div class="mt-8 px-4 py-2 bg-blue-500/10 text-blue-400 rounded-full text-sm font-medium border border-blue-500/20">
        正在连接至 Syncthing 核心...
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 可以在这里添加局部样式 */
</style>

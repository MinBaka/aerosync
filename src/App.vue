<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";

const statusText = ref("正在连接...");
const isRunning = ref(false);
const apiKey = ref("");
let timer: number | null = null;

const currentTab = ref("dashboard");
const tabs = [
  { id: "dashboard", name: "仪表盘", icon: "M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6" },
  { id: "folders", name: "文件夹", icon: "M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" },
  { id: "devices", name: "远程设备", icon: "M9 3v2m6-2v2M9 19v2m6-2v2M5 9H3m2 6H3m14-6h2m-2 6h2M7 19h10a2 2 0 002-2V7a2 2 0 00-2-2H7a2 2 0 00-2 2v10a2 2 0 002 2zM9 9h6v6H9V9z" },
  { id: "settings", name: "设置", icon: "M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z M15 12a3 3 0 11-6 0 3 3 0 016 0z" },
];

const checkStatus = async () => {
  try {
    const running = await invoke<boolean>("get_syncthing_status");
    isRunning.value = running;

    if (running) {
      statusText.value = "运行中";
      try {
        if (!apiKey.value) {
          const key = await invoke<string>("get_syncthing_api_key");
          if (key) apiKey.value = key;
        }
      } catch (e) {}
    } else {
      statusText.value = "正在连接...";
    }
  } catch (error) {
    statusText.value = "离线";
  }
};

onMounted(() => {
  checkStatus();
  timer = setInterval(checkStatus, 1000);
});

onUnmounted(() => {
  if (timer) clearInterval(timer);
});
</script>

<template>
  <div class="h-screen w-screen flex overflow-hidden bg-gray-900 text-gray-200">
    <!-- 左侧导航栏 -->
    <div class="w-56 bg-gray-900 border-r border-gray-800 flex flex-col py-6 shrink-0 shadow-lg z-10">
      <div class="px-6 mb-8 flex items-center gap-3">
        <div class="w-8 h-8 bg-gradient-to-br from-blue-500 to-indigo-600 rounded-lg shadow-lg flex items-center justify-center">
          <svg class="w-5 h-5 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4"></path>
          </svg>
        </div>
        <span class="font-bold tracking-wide text-lg text-gray-100">AeroSync</span>
      </div>

      <div class="px-4 mb-3 text-xs font-bold text-gray-500 uppercase tracking-wider">菜单</div>
      <nav class="flex-1 px-3 space-y-2">
        <button v-for="tab in tabs" :key="tab.id" @click="currentTab = tab.id"
          class="w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-all duration-200 focus:outline-none"
          :class="currentTab === tab.id ? 'bg-blue-600 text-white shadow-md shadow-blue-900/20' : 'text-gray-400 hover:bg-gray-800 hover:text-gray-200'">
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" :d="tab.icon"></path>
          </svg>
          {{ tab.name }}
        </button>
      </nav>

      <!-- 底部状态指示器 -->
      <div class="px-4 mt-auto">
        <div class="flex items-center gap-2 px-3 py-2 rounded-lg text-xs font-medium border"
             :class="isRunning ? 'bg-green-500/10 text-green-400 border-green-500/20' : 'bg-gray-500/10 text-gray-400 border-gray-500/20'">
          <span class="relative flex h-2 w-2">
            <span v-if="isRunning" class="animate-ping absolute inline-flex h-full w-full rounded-full bg-green-400 opacity-75"></span>
            <span class="relative inline-flex rounded-full h-2 w-2" :class="isRunning ? 'bg-green-500' : 'bg-gray-500'"></span>
          </span>
          <span>系统状态: {{ statusText }}</span>
        </div>
      </div>
    </div>

    <!-- 右侧主面板 -->
    <div class="flex-1 flex flex-col bg-gray-900 relative">
      <div class="absolute inset-0 bg-gradient-to-br from-gray-800/20 to-transparent pointer-events-none"></div>

      <div class="flex-1 p-10 overflow-y-auto z-0">
        <div v-if="currentTab === 'dashboard'" class="max-w-4xl mx-auto">
          <h2 class="text-3xl font-bold text-white mb-8 tracking-tight">系统概览</h2>

          <div class="grid grid-cols-3 gap-6 mb-10">
            <div class="bg-gray-800/60 backdrop-blur-md rounded-2xl p-6 border border-gray-700/50 shadow-xl transition hover:border-gray-600">
              <div class="text-gray-400 text-sm font-medium mb-2">同步状态</div>
              <div class="text-3xl font-bold text-white tracking-tight">同步完成</div>
            </div>
            <div class="bg-gray-800/60 backdrop-blur-md rounded-2xl p-6 border border-gray-700/50 shadow-xl transition hover:border-gray-600">
              <div class="text-gray-400 text-sm font-medium mb-2">全局下载</div>
              <div class="text-3xl font-bold text-blue-400 tracking-tight">0 B/s</div>
            </div>
            <div class="bg-gray-800/60 backdrop-blur-md rounded-2xl p-6 border border-gray-700/50 shadow-xl transition hover:border-gray-600">
              <div class="text-gray-400 text-sm font-medium mb-2">全局上传</div>
              <div class="text-3xl font-bold text-green-400 tracking-tight">0 B/s</div>
            </div>
          </div>

          <h3 class="text-xl font-bold text-gray-100 mb-5 tracking-tight">本机信息</h3>
          <div class="bg-gray-800/60 backdrop-blur-md rounded-2xl p-2 border border-gray-700/50 shadow-xl">
            <div class="flex justify-between items-center px-5 py-4 border-b border-gray-700/50">
              <span class="text-gray-400 font-medium">核心 API 密钥</span>
              <span class="font-mono text-sm text-gray-300 bg-gray-900/50 px-3 py-1 rounded-md">{{ apiKey || '等待核心启动...' }}</span>
            </div>
            <div class="flex justify-between items-center px-5 py-4 border-b border-gray-700/50">
              <span class="text-gray-400 font-medium">监听地址</span>
              <span class="text-sm text-gray-300">127.0.0.1:8384</span>
            </div>
            <div class="flex justify-between items-center px-5 py-4">
              <span class="text-gray-400 font-medium">核心版本</span>
              <span class="text-sm text-gray-300">v1.27.7</span>
            </div>
          </div>
        </div>

        <div v-else class="flex flex-col items-center justify-center h-full text-gray-500">
          <div class="w-24 h-24 mb-6 rounded-full bg-gray-800/50 flex items-center justify-center shadow-inner">
            <svg class="w-10 h-10 opacity-50" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10"></path>
            </svg>
          </div>
          <h2 class="text-xl font-bold text-gray-300 mb-2">开发中</h2>
          <p>《{{ tabs.find(t => t.id === currentTab)?.name }}》 功能模块即将到来</p>
        </div>

      </div>
    </div>
  </div>
</template>

<style scoped>
</style>
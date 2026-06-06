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

const minimize = () => invoke("win_minimize");
const toggleMaximize = () => invoke("win_toggle_maximize");
const close = () => invoke("win_close");

onMounted(() => {
  checkStatus();
  timer = setInterval(checkStatus, 1000);
});

onUnmounted(() => {
  if (timer) clearInterval(timer);
});
</script>

<template>
  <!-- 外层绝对透明，无边框 -->
  <div class="h-screen w-screen bg-transparent p-0 m-0 overflow-hidden">
    <!-- 内部的界面容器带圆角和阴影，稍微缩进一点点以便显示阴影 -->
    <div class="flex flex-col h-[calc(100vh-16px)] w-[calc(100vw-16px)] m-2 bg-gray-900/95 backdrop-blur-3xl rounded-xl border border-gray-700/50 shadow-2xl overflow-hidden text-gray-200">

      <!-- 自定义系统标题栏 (可拖拽) -->
      <div data-tauri-drag-region class="h-12 flex justify-between items-center px-4 bg-gray-800/40 border-b border-gray-800/80 shrink-0 select-none">

        <!-- 左侧: 品牌 Logo 和名称 (穿透点击) -->
        <div data-tauri-drag-region class="flex items-center gap-3 pointer-events-none">
          <div class="w-7 h-7 bg-blue-500 rounded-lg flex items-center justify-center">
            <svg class="w-4 h-4 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4"></path>
            </svg>
          </div>
          <span class="font-bold tracking-wide text-sm text-gray-100">AeroSync</span>

          <div class="ml-4 flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[10px] font-medium border"
               :class="isRunning ? 'bg-green-500/10 text-green-400 border-green-500/20' : 'bg-gray-500/10 text-gray-400 border-gray-500/20'">
            <span class="relative flex h-1.5 w-1.5">
              <span v-if="isRunning" class="animate-ping absolute inline-flex h-full w-full rounded-full bg-green-400 opacity-75"></span>
              <span class="relative inline-flex rounded-full h-1.5 w-1.5" :class="isRunning ? 'bg-green-500' : 'bg-gray-500'"></span>
            </span>
            {{ statusText }}
          </div>
        </div>

        <!-- 右侧: macOS 风格的窗口控制按钮 -->
        <div class="flex items-center gap-2 z-50 pointer-events-auto">
          <button @click="minimize" class="w-3.5 h-3.5 rounded-full bg-yellow-500 hover:bg-yellow-400 transition-colors focus:outline-none flex items-center justify-center group cursor-pointer">
            <svg class="w-2 h-2 text-yellow-900 opacity-0 group-hover:opacity-100" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M20 12H4"></path></svg>
          </button>
          <button @click="toggleMaximize" class="w-3.5 h-3.5 rounded-full bg-green-500 hover:bg-green-400 transition-colors focus:outline-none flex items-center justify-center group cursor-pointer">
            <svg class="w-2 h-2 text-green-900 opacity-0 group-hover:opacity-100" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M4 8V4m0 0h4M4 4l5 5m11-1V4m0 0h-4m4 0l-5 5M4 16v4m0 0h4m-4 0l5-5m11 5l-5-5m5 5v-4m0 4h-4"></path></svg>
          </button>
          <button @click="close" class="w-3.5 h-3.5 rounded-full bg-red-500 hover:bg-red-400 transition-colors focus:outline-none flex items-center justify-center group cursor-pointer">
            <svg class="w-2 h-2 text-red-900 opacity-0 group-hover:opacity-100" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M6 18L18 6M6 6l12 12"></path></svg>
          </button>
        </div>
      </div>

      <!-- 主体内容区: 左侧边栏 + 右侧功能面板 -->
      <div class="flex-1 flex overflow-hidden">

        <!-- 左侧导航栏 -->
        <div class="w-56 bg-gray-900/40 border-r border-gray-800/80 flex flex-col py-4 shrink-0">
          <div class="px-4 mb-2 text-xs font-semibold text-gray-500 uppercase tracking-wider">菜单</div>
          <nav class="flex-1 px-2 space-y-1">
            <button v-for="tab in tabs" :key="tab.id" @click="currentTab = tab.id"
              class="w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm transition-all duration-200 focus:outline-none"
              :class="currentTab === tab.id ? 'bg-blue-600 text-white shadow-md shadow-blue-900/20' : 'text-gray-400 hover:bg-gray-800/60 hover:text-gray-200'">
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" :d="tab.icon"></path>
              </svg>
              {{ tab.name }}
            </button>
          </nav>
        </div>

        <!-- 右侧主面板 -->
        <div class="flex-1 p-8 overflow-y-auto bg-gray-900/20">
          <div v-if="currentTab === 'dashboard'" class="max-w-4xl mx-auto">
            <h2 class="text-2xl font-bold text-white mb-6">系统概览</h2>

            <div class="grid grid-cols-3 gap-6 mb-8">
              <div class="bg-gray-800/40 rounded-xl p-5 border border-gray-700/50">
                <div class="text-gray-400 text-sm font-medium mb-1">同步状态</div>
                <div class="text-2xl font-bold text-white">同步完成</div>
              </div>
              <div class="bg-gray-800/40 rounded-xl p-5 border border-gray-700/50">
                <div class="text-gray-400 text-sm font-medium mb-1">全局下载</div>
                <div class="text-2xl font-bold text-blue-400">0 B/s</div>
              </div>
              <div class="bg-gray-800/40 rounded-xl p-5 border border-gray-700/50">
                <div class="text-gray-400 text-sm font-medium mb-1">全局上传</div>
                <div class="text-2xl font-bold text-green-400">0 B/s</div>
              </div>
            </div>

            <h3 class="text-lg font-semibold text-gray-200 mb-4">本机信息</h3>
            <div class="bg-gray-800/40 rounded-xl p-5 border border-gray-700/50 space-y-3">
              <div class="flex justify-between items-center py-2 border-b border-gray-700/50">
                <span class="text-gray-400">核心 API 密钥</span>
                <span class="font-mono text-sm text-gray-300">{{ apiKey || '等待核心启动...' }}</span>
              </div>
              <div class="flex justify-between items-center py-2 border-b border-gray-700/50">
                <span class="text-gray-400">监听地址</span>
                <span class="text-sm text-gray-300">127.0.0.1:8384</span>
              </div>
              <div class="flex justify-between items-center py-2">
                <span class="text-gray-400">核心版本</span>
                <span class="text-sm text-gray-300">v1.27.7</span>
              </div>
            </div>
          </div>

          <div v-else class="flex flex-col items-center justify-center h-full text-gray-500">
            <svg class="w-16 h-16 mb-4 opacity-50" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10"></path>
            </svg>
            <p>《{{ tabs.find(t => t.id === currentTab)?.name }}》 功能模块正在火热开发中...</p>
          </div>

        </div>
      </div>

    </div>
  </div>
</template>

<style scoped>
</style>
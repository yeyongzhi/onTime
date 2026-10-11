<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { RouterView } from 'vue-router'
import { Toaster } from '@/components/ui/sonner'
import { useAlarmsStore } from '@/stores/alarms'
import { darkMode } from '@/stores/appearance'
const store = useAlarmsStore()
onMounted(() => { void store.start() })
onUnmounted(() => store.stop())
</script>
<template><RouterView /><Toaster position="bottom-right" :theme="darkMode ? 'dark' : 'light'" rich-colors /></template>

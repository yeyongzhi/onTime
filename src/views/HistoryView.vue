<script setup lang="ts">
import { useAlarmsStore } from '@/stores/alarms'
import { dateLabel, statusLabels } from '@/lib/types'
import { Table,TableHeader,TableHead,TableRow,TableBody,TableCell } from '@/components/ui/table'
import { Card, CardContent } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Empty,EmptyHeader,EmptyTitle,EmptyDescription } from '@/components/ui/empty'
const store=useAlarmsStore()
</script>
<template><main class="workspace-page"><header><h1 class="workspace-heading">提醒记录</h1><p class="mt-2 text-sm text-muted-foreground">最近 90 天 · 最多 500 条记录</p></header><Card><CardContent><Table v-if="store.snapshot?.occurrences.length"><TableHeader><TableRow><TableHead>提醒</TableHead><TableHead>计划时间</TableHead><TableHead>类型</TableHead><TableHead>状态</TableHead><TableHead>贪睡</TableHead></TableRow></TableHeader><TableBody><TableRow v-for="o in store.snapshot.occurrences" :key="o.id"><TableCell>{{ o.title }}</TableCell><TableCell>{{ dateLabel(o.scheduledAt) }}</TableCell><TableCell>{{ o.reason }}</TableCell><TableCell><Badge variant="secondary">{{ statusLabels[o.status]||o.status }}</Badge></TableCell><TableCell>{{ o.snoozeCount }} 次</TableCell></TableRow></TableBody></Table><Empty v-else><EmptyHeader><EmptyTitle>还没有提醒记录</EmptyTitle><EmptyDescription>提醒触发后，处理结果会显示在这里。</EmptyDescription></EmptyHeader></Empty></CardContent></Card></main></template>
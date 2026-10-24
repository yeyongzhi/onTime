<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { appearance } from '@/stores/appearance'
import { useRouter } from 'vue-router'
import { PlusIcon, PencilIcon, Trash2Icon, EyeIcon, XIcon, LayoutGridIcon, ListIcon } from '@lucide/vue'
import type { Tag } from '@/lib/types'
import { useAlarmsStore } from '@/stores/alarms'
import OrganizationColorPicker from '@/components/OrganizationColorPicker.vue'
import ListPagination from '@/components/ListPagination.vue'
import { Table, TableHeader, TableHead, TableRow, TableBody, TableCell } from '@/components/ui/table'
import { Button } from '@/components/ui/button'
import { InputGroup, InputGroupInput, InputGroupAddon, InputGroupButton } from '@/components/ui/input-group'
import { Input } from '@/components/ui/input'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter } from '@/components/ui/card'
import { FieldGroup, Field, FieldLabel } from '@/components/ui/field'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '@/components/ui/dialog'
import { Empty, EmptyHeader, EmptyTitle, EmptyDescription, EmptyContent } from '@/components/ui/empty'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
const props = defineProps<{ kind: 'groups' | 'tags' }>()
const store = useAlarmsStore()
const router = useRouter()
const isGroup = computed(() => props.kind === 'groups')
const label = computed(() => isGroup.value ? '分组' : '标签')
const items = computed(() => store.snapshot?.[props.kind] ?? [])
const query = ref('')
const appliedQuery = ref('')
function applyOrganizationQuery() { appliedQuery.value=query.value; page.value=1 }
function resetOrganizationQuery() { query.value=''; applyOrganizationQuery() }
const displayMode = computed({ get: () => appearance.displayMode, set: value => { appearance.displayMode = value as 'grid' | 'list' } })
const filteredItems = computed(() => items.value.filter(item => item.name.toLocaleLowerCase().includes(appliedQuery.value.trim().toLocaleLowerCase())))
function clearName() { query.value=''; document.getElementById('organization-search')?.focus() }
watch(() => props.kind, () => { query.value = ''; appliedQuery.value='' })
const editing = ref(false)
const form = ref<Tag>({ id: '', name: '', color: '#7c6cf0' })
const deletion = ref<Tag | null>(null)
function count(id: string) { return store.snapshot?.alarms.filter(a => isGroup.value ? a.groupId === id : a.tags.includes(id)).length ?? 0 }
function edit(item?: Tag) { form.value = item ? { ...item } : { id: '', name: '', color: '#7c6cf0' }; editing.value = true }
async function save() {
  try {
    await store.action(isGroup.value ? 'save_group' : 'save_tag', { [isGroup.value ? 'group' : 'tag']: form.value }, `${label.value}已保存`)
    editing.value = false
  } catch { /* Store reports errors. */ }
}
async function remove() {
  if (!deletion.value) return
  try { await store.action(isGroup.value ? 'delete_group' : 'delete_tag', { id: deletion.value.id }, `${label.value}已删除`); deletion.value = null } catch { /* Store reports errors. */ }
}
function view(id: string) { router.push({ path: '/reminders', query: { [isGroup.value ? 'group' : 'tag']: id } }) }
const page = ref(1)
const pageSize = 10
const totalPages = computed(() => Math.max(1, Math.ceil(filteredItems.value.length / pageSize)))
const paged = computed(() => filteredItems.value.slice((page.value - 1) * pageSize, page.value * pageSize))
watch(totalPages, total => { page.value = Math.min(page.value, total) })
watch([appliedQuery, () => props.kind], () => { page.value = 1 })
</script>
<template>
  <main class="workspace-page organization-page">
    <Card class="shrink-0"><CardHeader class="sr-only"><CardTitle>{{ label }}管理</CardTitle></CardHeader><CardContent class="organization-toolbar-grid">
      <Field class="min-w-0"><FieldLabel for="organization-search" class="sr-only">{{ label }}名称</FieldLabel><InputGroup><InputGroupInput id="organization-search" v-model="query" :placeholder="`搜索${label}名称…`" /><InputGroupAddon v-if="query" align="inline-end"><InputGroupButton size="icon-xs" aria-label="清空名称" @click="clearName"><XIcon /></InputGroupButton></InputGroupAddon></InputGroup></Field>
      <div class="organization-filter-actions"><Button @click="applyOrganizationQuery">查询</Button><Button variant="secondary" @click="resetOrganizationQuery">重置</Button></div>
    </CardContent></Card>
    <Card class="organization-action-card"><CardContent class="organization-action-row"><span class="reminder-total">共有 {{ items.length }} 条{{ label }}</span><Tabs v-model="displayMode"><TabsList :aria-label="`${label}显示方式`"><TabsTrigger value="grid"><LayoutGridIcon />网格</TabsTrigger><TabsTrigger value="list"><ListIcon />列表</TabsTrigger></TabsList></Tabs><Button :disabled="!store.snapshot || store.busy" @click="edit()"><PlusIcon data-icon="inline-start" />新建{{ label }}</Button></CardContent></Card>
    <Card class="organization-content"><ScrollArea class="min-h-0 flex-1"><CardContent>
    <Table v-if="filteredItems.length && displayMode === 'list'"><TableHeader><TableRow><TableHead>序号</TableHead><TableHead>{{ label }}名称</TableHead><TableHead>提醒数量</TableHead><TableHead>操作</TableHead></TableRow></TableHeader><TableBody><TableRow v-for="(item, index) in paged" :key="item.id"><TableCell class="tabular-nums">{{ (page - 1) * pageSize + index + 1 }}</TableCell><TableCell><div class="flex items-center gap-2"><span class="size-3 shrink-0 rounded-full" :style="{ backgroundColor: item.color }" /><span class="font-medium">{{ item.name }}</span></div></TableCell><TableCell>{{ count(item.id) }} 个提醒</TableCell><TableCell><div class="flex justify-start gap-2"><Button variant="secondary" size="sm" @click="view(item.id)">查看提醒</Button><Button variant="secondary" size="sm" :disabled="store.busy" @click="edit(item)">编辑</Button><Button variant="destructive" size="sm" :disabled="store.busy" @click="deletion = item">删除</Button></div></TableCell></TableRow></TableBody></Table>
    <div v-else-if="filteredItems.length" class="grid gap-3 organization-grid" :data-display="displayMode"><Card v-for="item in paged" :key="item.id" size="sm" class="data-[size=sm]:gap-1"><CardHeader><CardTitle class="group-data-[size=sm]/card:text-base font-bold">{{ item.name }}</CardTitle><CardDescription>包含 {{ count(item.id) }} 个提醒</CardDescription></CardHeader><CardFooter class="flex flex-wrap items-center justify-end gap-1 bg-transparent px-3 pb-3 pt-0 border-t-0">
      <Tooltip><TooltipTrigger as-child><Button variant="secondary" size="icon-sm" :aria-label="`查看${label}${item.name}的提醒`" @click="view(item.id)"><EyeIcon /></Button></TooltipTrigger><TooltipContent>查看提醒</TooltipContent></Tooltip>
      <Tooltip><TooltipTrigger as-child><Button variant="secondary" size="icon-sm" :disabled="store.busy" :aria-label="`编辑${label}${item.name}`" @click="edit(item)"><PencilIcon /></Button></TooltipTrigger><TooltipContent>编辑{{ label }}</TooltipContent></Tooltip>
      <Tooltip><TooltipTrigger as-child><Button variant="destructive" size="icon-sm" :disabled="store.busy" :aria-label="`删除${label}${item.name}`" @click="deletion = item"><Trash2Icon /></Button></TooltipTrigger><TooltipContent>删除{{ label }}</TooltipContent></Tooltip>
    </CardFooter></Card></div>
    <Empty v-else><EmptyHeader><EmptyTitle>{{ items.length ? `没有匹配的${label}` : `还没有${label}` }}</EmptyTitle><EmptyDescription>{{ items.length ? '请调整搜索名称。' : isGroup ? '创建分组后，在新建或编辑提醒时选择归属。' : '创建标签后，在新建或编辑提醒时勾选标签。' }}</EmptyDescription></EmptyHeader><EmptyContent><Button variant="secondary" :disabled="!store.snapshot" @click="edit()">新建{{ label }}</Button></EmptyContent></Empty>
    </CardContent></ScrollArea><ListPagination v-if="filteredItems.length" v-model="page" :total="filteredItems.length" :page-size="pageSize" /></Card>
    <Dialog v-model:open="editing"><DialogContent><DialogHeader><DialogTitle>{{ form.id ? '编辑' : '新建' }}{{ label }}</DialogTitle><DialogDescription>{{ isGroup ? '分组用于划分独立的提醒列表。' : '标签用于标记和筛选提醒。' }}</DialogDescription></DialogHeader><form id="organization-form" @submit.prevent="save"><FieldGroup><Field><FieldLabel for="organization-name">名称</FieldLabel><Input id="organization-name" v-model="form.name" maxlength="24" required :disabled="store.busy" /></Field><Field><FieldLabel>颜色</FieldLabel><OrganizationColorPicker v-model="form.color" :disabled="store.busy" /></Field></FieldGroup></form><DialogFooter><Button variant="secondary" :disabled="store.busy" @click="editing = false">取消</Button><Button type="submit" form="organization-form" :disabled="store.busy">保存{{ label }}</Button></DialogFooter></DialogContent></Dialog>
    <Dialog :open="!!deletion" @update:open="value => { if (!value) deletion = null }"><DialogContent><DialogHeader><DialogTitle>删除{{ label }}“{{ deletion?.name }}”？</DialogTitle><DialogDescription>{{ isGroup ? '组内提醒将移至未分组，提醒时间、启用状态和标签保留。' : '从所有提醒上移除此标签，提醒和分组保留。' }}</DialogDescription></DialogHeader><DialogFooter><Button variant="secondary" :disabled="store.busy" @click="deletion = null">取消</Button><Button variant="destructive" :disabled="store.busy" @click="remove">删除{{ label }}</Button></DialogFooter></DialogContent></Dialog>
  </main>
</template>

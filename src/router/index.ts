import { createRouter, createWebHashHistory } from 'vue-router'
export default createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', component: () => import('@/components/AppShell.vue'), children: [
      { path: '', component: () => import('@/views/HomeView.vue') },
      { path: 'reminders/new', component: () => import('@/views/ReminderEditorView.vue') },
      { path: 'reminders/:id/edit', name: 'edit-reminder', component: () => import('@/views/ReminderEditorView.vue') },
      { path: 'history', component: () => import('@/views/HistoryView.vue') },
      { path: 'settings', component: () => import('@/views/SettingsView.vue') },
    ] },
    { path: '/alert', component: () => import('@/views/AlertView.vue') },
  ],
})

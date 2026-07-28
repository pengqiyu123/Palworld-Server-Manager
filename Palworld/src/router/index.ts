import { createRouter, createWebHistory, type RouteRecordRaw } from 'vue-router'

/**
 * 路由表（视觉还原轮）：
 * - 四个主屏 S1~S4：/overview /config /network /rcon
 * - 旧设置和故障排查地址重定向至系统日志页
 * - / 重定向到 /overview
 * - /system-logs 提供可复制、可导出的应用运行记录
 */
const routes: RouteRecordRaw[] = [
  {
    path: '/',
    redirect: '/overview',
  },
  {
    path: '/overview',
    name: 'overview',
    component: () => import('@/views/OverviewView.vue'),
    meta: { title: '概览' },
  },
  {
    path: '/players',
    name: 'players',
    component: () => import('@/views/PlayersView.vue'),
    meta: { title: '玩家管理' },
  },
  {
    path: '/config',
    name: 'config',
    component: () => import('@/views/ConfigView.vue'),
    meta: { title: '配置' },
  },
  {
    path: '/rcon',
    name: 'rcon',
    component: () => import('@/views/RconView.vue'),
    meta: { title: '服务器控制台' },
  },
  {
    path: '/logs',
    name: 'logs',
    component: () => import('@/views/LogsView.vue'),
    meta: { title: '实时日志' },
  },
  {
    path: '/saves',
    name: 'saves',
    component: () => import('@/views/SaveManagementView.vue'),
    meta: { title: '世界存档' },
  },
  {
    path: '/migrate',
    name: 'migrate',
    component: () => import('@/views/SaveMigrationView.vue'),
    meta: { title: '世界与角色迁移' },
  },
  {
    path: '/modifier',
    name: 'modifier',
    component: () => import('@/views/ModifierView.vue'),
    meta: { title: '修改器' },
  },
  {
    path: '/backup',
    redirect: { path: '/saves', query: { tab: 'backup' } },
  },
  {
    path: '/settings',
    redirect: '/system-logs',
  },
  {
    path: '/troubleshoot',
    redirect: '/system-logs',
  },
  {
    path: '/system-logs',
    name: 'system-logs',
    component: () => import('@/views/SystemLogsView.vue'),
    meta: { title: '系统日志' },
  },
]

const router = createRouter({
  history: createWebHistory(),
  routes,
})

// 路由前置守卫：更新页面标题
router.beforeEach((to, _from, next) => {
  const title = to.meta.title as string | undefined
  if (title) {
    document.title = `${title} - Palworld Server Manager`
  }
  next()
})

// 路由切换后重置所有 .screen 滚动位置（避免返回时长表单停在奇怪位置）
router.afterEach(() => {
  const screens = document.querySelectorAll<HTMLElement>('.screen')
  screens.forEach((el) => {
    el.scrollTop = 0
  })
})

export default router

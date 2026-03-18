/**
 * 路由配置
 */
import {
  createRouter,
  createWebHistory,
  type RouteRecordRaw,
} from "vue-router";

const MainLayout = () => import("@/components/layout/MainLayout.vue");

const routes: RouteRecordRaw[] = [
  { path: "/", redirect: "/dashboard" },
  {
    path: "/",
    component: MainLayout,
    children: [
      {
        path: "dashboard",
        name: "Dashboard",
        component: () => import("@/views/dashboard/DashboardView.vue"),
        meta: { title: "仪表盘" },
      },
      {
        path: "records",
        name: "Records",
        component: () => import("@/views/records/RecordsView.vue"),
        meta: { title: "学习记录" },
      },
      {
        path: "categories",
        name: "Categories",
        component: () => import("@/views/categories/CategoriesView.vue"),
        meta: { title: "分类管理" },
      },
      {
        path: "stages",
        name: "Stages",
        component: () => import("@/views/stages/StagesView.vue"),
        meta: { title: "阶段管理" },
      },
      {
        path: "milestones",
        name: "Milestones",
        component: () => import("@/views/milestones/MilestonesView.vue"),
        meta: { title: "成就时刻" },
      },
      {
        path: "milestones/categories",
        name: "MilestoneCategories",
        component: () =>
          import("@/views/milestones/MilestoneCategoryManager.vue"),
        meta: { title: "成就分类管理" },
      },
      {
        path: "countdown",
        name: "Countdown",
        component: () => import("@/views/countdown/CountdownView.vue"),
        meta: { title: "倒计时" },
      },
      {
        path: "focus",
        name: "Focus",
        component: () => import("@/views/focus/FocusView.vue"),
        meta: { title: "专注模式" },
      },
      {
        path: "charts",
        name: "Charts",
        component: () => import("@/views/charts/ChartsView.vue"),
        meta: { title: "统计分析" },
      },
      {
        path: "ai",
        name: "AIAssistant",
        component: () => import("@/views/ai/AIAssistantView.vue"),
        meta: { title: "智能规划" },
      },
      {
        path: "settings",
        component: () => import("@/views/settings/SettingsLayout.vue"),
        redirect: "/settings/account",
        children: [
          {
            path: "account",
            name: "SettingsAccount",
            component: () => import("@/views/settings/Account.vue"),
            meta: { title: "本地档案" },
          },
          {
            path: "data",
            name: "SettingsData",
            component: () => import("@/views/settings/Data.vue"),
            meta: { title: "数据管理" },
          },
          {
            path: "ai",
            name: "SettingsAI",
            component: () => import("@/views/settings/AIConfig.vue"),
            meta: { title: "AI 配置" },
          },
          // 新增：阶段管理（与顶层 /stages 复用同一组件）
          {
            path: "stages",
            name: "SettingsStages",
            component: () => import("@/views/stages/StagesView.vue"),
            meta: { title: "阶段管理" },
          },
          // 新增：分类管理（与顶层 /categories 复用同一组件）
          {
            path: "categories",
            name: "SettingsCategories",
            component: () => import("@/views/categories/CategoriesView.vue"),
            meta: { title: "分类管理" },
          },
          // 新增：格言管理（占位组件）
          {
            path: "mottos",
            name: "SettingsMottos",
            component: () => import("@/views/settings/MottoManagement.vue"),
            meta: { title: "格言管理" },
          },
        ],
      },
    ],
  },
  {
    path: "/:pathMatch(.*)*",
    name: "NotFound",
    component: () => import("@/views/error/NotFoundView.vue"),
  },
];

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes,
});

export default router;

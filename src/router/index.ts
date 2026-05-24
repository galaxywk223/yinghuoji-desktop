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
  { path: "/ai", redirect: "/dashboard" },
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
        meta: { title: "学习分类" },
      },
      {
        path: "stages",
        name: "Stages",
        component: () => import("@/views/stages/StagesView.vue"),
        meta: { title: "学习阶段" },
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
        meta: { title: "成就分类" },
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
        meta: { title: "专注计时" },
      },
      {
        path: "charts",
        name: "Charts",
        component: () => import("@/views/charts/ChartsView.vue"),
        meta: { title: "学习回顾" },
      },
      {
        path: "settings",
        component: () => import("@/views/settings/SettingsLayout.vue"),
        redirect: "/settings/data",
        children: [
          {
            path: "ai",
            redirect: "/settings/data",
          },
          {
            path: "data",
            name: "SettingsData",
            component: () => import("@/views/settings/Data.vue"),
            meta: { title: "学习数据" },
          },
          // 新增：学习阶段（与顶层 /stages 复用同一组件）
          {
            path: "stages",
            name: "SettingsStages",
            component: () => import("@/views/stages/StagesView.vue"),
            meta: { title: "学习阶段" },
          },
          // 新增：学习分类（与顶层 /categories 复用同一组件）
          {
            path: "categories",
            name: "SettingsCategories",
            component: () => import("@/views/categories/CategoriesView.vue"),
            meta: { title: "学习分类" },
          },
          // 新增：每日一句（占位组件）
          {
            path: "mottos",
            name: "SettingsMottos",
            component: () => import("@/views/settings/MottoManagement.vue"),
            meta: { title: "每日一句" },
          },
          {
            path: "focus",
            name: "SettingsFocusPreferences",
            component: () => import("@/views/settings/FocusPreferences.vue"),
            meta: { title: "专注偏好" },
          },
          {
            path: "about",
            name: "SettingsAboutUpdate",
            component: () => import("@/views/settings/AboutUpdate.vue"),
            meta: { title: "关于与更新" },
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

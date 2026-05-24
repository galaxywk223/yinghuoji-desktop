<template>
  <div class="settings-shell">
    <PageContainer
      :title="{ icon: 'lucide:settings-2', text: '偏好设置' }"
      subtitle="把应用调整成更适合自己的样子"
      :custom-class="'settings-layout-container'"
      max-width="wide"
      fill-height
    >
      <div class="settings-main-content">
        <aside class="settings-sidebar">
          <nav class="settings-nav">
            <router-link to="/settings/data" class="settings-nav-item">
              <Icon icon="lucide:database" />
              <span>学习数据</span>
            </router-link>
            <router-link to="/settings/stages" class="settings-nav-item">
              <Icon icon="lucide:flag" />
              <span>学习阶段</span>
            </router-link>
            <router-link to="/settings/categories" class="settings-nav-item">
              <Icon icon="lucide:folder" />
              <span>学习分类</span>
            </router-link>
            <router-link to="/settings/mottos" class="settings-nav-item">
              <Icon icon="lucide:message-square-quote" />
              <span>每日一句</span>
            </router-link>
            <router-link to="/settings/focus" class="settings-nav-item">
              <Icon icon="lucide:timer-reset" />
              <span>专注偏好</span>
            </router-link>
            <router-link to="/settings/about" class="settings-nav-item">
              <Icon icon="lucide:info" />
              <span>关于与更新</span>
            </router-link>
          </nav>
        </aside>
        <main class="settings-content">
          <router-view v-slot="{ Component }">
            <transition name="fade" mode="out-in">
              <component :is="Component" />
            </transition>
          </router-view>
        </main>
      </div>
    </PageContainer>
  </div>
</template>

<script setup>
import { Icon } from "@iconify/vue";
import PageContainer from "@/components/layout/PageContainer.vue";
</script>

<style scoped>
@import "@/styles/views/settings/layout.scss";
</style>

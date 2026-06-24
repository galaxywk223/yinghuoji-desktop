<template>
  <PageContainer
    :title="{ icon: 'lucide:sparkles', text: 'AI 助手' }"
    subtitle="基于本地学习数据进行多轮分析和问答"
    custom-class="ai-assistant-page"
    max-width="full"
    fill-height
  >
    <template #actions>
      <div class="ai-header-actions">
        <router-link to="/settings/ai" class="pill-btn secondary">
          <Icon icon="lucide:key-round" />
          AI 设置
        </router-link>
        <button class="pill-btn primary" type="button" @click="newSession">
          <Icon icon="lucide:plus" />
          新建对话
        </button>
      </div>
    </template>

    <div class="ai-workbench">
      <aside class="session-panel">
        <div class="panel-head">
          <span>会话</span>
          <button class="icon-btn" type="button" aria-label="刷新会话" @click="store.fetchSessions()">
            <Icon icon="lucide:refresh-cw" />
          </button>
        </div>
        <div v-if="store.loadingSessions" class="session-state">正在加载会话</div>
        <div v-else-if="!store.sessions.length" class="session-state">
          暂无历史会话
        </div>
        <div v-else class="session-list">
          <div
            v-for="session in store.sessions"
            :key="session.id"
            class="session-item"
            :class="{ active: session.id === store.activeSessionId }"
          >
            <button
              class="session-select"
              type="button"
              @click="store.selectSession(session.id)"
            >
              <span class="session-title">{{ session.title }}</span>
              <span class="session-time">{{ formatDateTime(session.last_message_at) }}</span>
            </button>
            <button
              class="session-title-btn"
              :class="{ spinning: store.generatingTitleSessionId === session.id }"
              type="button"
              :aria-label="`自动生成标题：${session.title}`"
              :disabled="store.generatingTitleSessionId === session.id || !store.hasProviderKey"
              @click.stop="generateTitle(session.id)"
            >
              <Icon
                :icon="
                  store.generatingTitleSessionId === session.id
                    ? 'lucide:loader-circle'
                    : 'lucide:wand-sparkles'
                "
              />
            </button>
          </div>
        </div>
      </aside>

      <section class="chat-panel">
        <div v-if="!store.hasProviderKey" class="config-warning">
          <Icon icon="lucide:triangle-alert" />
          <span>需要先在 AI 设置中保存 API Key。</span>
          <router-link to="/settings/ai">前往设置</router-link>
        </div>

        <div ref="messageListRef" class="message-list">
          <div v-if="store.loadingMessages" class="empty-chat">
            <Icon icon="lucide:loader-circle" />
            <p>正在读取会话</p>
          </div>

          <div v-else-if="!store.messages.length" class="empty-chat">
            <Icon icon="lucide:message-square-text" />
            <h3>从一个具体问题开始</h3>
            <p>AI 会按问题主动读取阶段、记录、课程画像、成就或趋势数据。</p>
            <div class="prompt-grid">
              <button
                v-for="prompt in starterPrompts"
                :key="prompt"
                type="button"
                @click="draft = prompt"
              >
                {{ prompt }}
              </button>
            </div>
          </div>

          <template v-else>
            <article
              v-for="message in store.messages"
              :key="message.id"
              class="message-row"
              :class="`message-row--${message.role}`"
            >
              <div class="message-avatar">
                <Icon :icon="message.role === 'user' ? 'lucide:user' : 'lucide:sparkles'" />
              </div>
              <div class="message-bubble">
                <div class="message-meta">
                  <span>{{ message.role === "user" ? "我" : "AI 助手" }}</span>
                  <small>{{ formatDateTime(message.created_at) }}</small>
                </div>
                <p v-if="message.role === 'user'" class="plain-message">{{ message.content }}</p>
                <MarkdownContent v-else :content="message.content" />
              </div>
            </article>
          </template>

          <article v-if="store.sending" class="message-row message-row--assistant">
            <div class="message-avatar">
              <Icon icon="lucide:sparkles" />
            </div>
            <div class="message-bubble thinking">
              <div class="message-meta">
                <span>AI 助手</span>
                <small>读取数据中</small>
              </div>
              <p>正在判断需要查询哪些本地数据...</p>
            </div>
          </article>
        </div>

        <form class="composer" @submit.prevent="send">
          <textarea
            v-model="draft"
            rows="3"
            placeholder="例如：分析一下最近两周我在哪些科目上投入不足，给出具体依据。"
            @keydown.enter.exact.prevent="send"
          />
          <div class="composer-actions">
            <span>AI 可按问题读取全部本地数据</span>
            <button
              class="pill-btn primary"
              type="submit"
              :disabled="store.sending || !draft.trim() || !store.hasProviderKey"
            >
              <Icon icon="lucide:send-horizontal" />
              发送
            </button>
          </div>
        </form>
      </section>
    </div>
  </PageContainer>
</template>

<script setup lang="ts">
import { nextTick, onMounted, ref, watch } from "vue";
import { Icon } from "@iconify/vue";
import { ElMessage } from "element-plus";
import PageContainer from "@/components/layout/PageContainer.vue";
import MarkdownContent from "@/components/ai/MarkdownContent.vue";
import { useAiStore } from "@/stores/modules/ai";

const store = useAiStore();
const draft = ref("");
const messageListRef = ref<HTMLElement | null>(null);

const starterPrompts = [
  "最近两周我的学习投入有什么明显变化？",
  "结合课程画像，找出高学分但投入不足的课程。",
  "本月哪些分类效率偏低？请给出依据。",
  "回顾最近的成就和倒计时，帮我规划下一步。"
];

function formatDateTime(value?: string) {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return `${date.getMonth() + 1}/${date.getDate()} ${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
}

async function newSession() {
  await store.createSession({
    title: "新的对话",
  });
}

async function generateTitle(sessionId: number) {
  if (!store.hasProviderKey) {
    ElMessage.warning("请先在 AI 设置中保存 API Key");
    return;
  }
  await store.generateSessionTitle(sessionId);
}

async function send() {
  const text = draft.value.trim();
  if (!text) return;
  if (!store.hasProviderKey) {
    ElMessage.warning("请先在 AI 设置中保存 API Key");
    return;
  }
  draft.value = "";
  await store.sendMessage(text);
  await scrollToBottom();
}

async function scrollToBottom() {
  await nextTick();
  if (messageListRef.value) {
    messageListRef.value.scrollTop = messageListRef.value.scrollHeight;
  }
}

watch(
  () => store.messages.length,
  () => {
    void scrollToBottom();
  },
);

onMounted(async () => {
  await Promise.all([
    store.fetchProvider(),
    store.fetchSessions(),
  ]);
  if (!store.activeSession && store.sessions.length) {
    await store.selectSession(store.sessions[0].id);
  }
});
</script>

<style scoped lang="scss">
.ai-header-actions {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.ai-assistant-page {
  height: 100%;
  min-height: 0;
  overflow: hidden;
}

.ai-assistant-page :deep(.page-body) {
  flex: 1;
  min-height: 0;
  height: 100%;
  overflow: hidden;
}

.ai-workbench {
  flex: 1;
  min-height: 0;
  height: 100%;
  display: grid;
  grid-template-columns: 260px minmax(0, 1fr);
  gap: 16px;
  overflow: hidden;
}

.session-panel,
.chat-panel {
  min-height: 0;
  border: 1px solid var(--stroke-soft);
  border-radius: 14px;
  background: var(--surface-card);
  box-shadow: var(--box-shadow-card);
}

.session-panel {
  padding: 14px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  overflow: hidden;
}

.panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  color: var(--color-text-heading);
  font-size: 14px;
  font-weight: 800;
}

.icon-btn {
  border: none;
  background: transparent;
  color: var(--color-text-secondary);
  cursor: pointer;
}

.icon-btn {
  width: 30px;
  height: 30px;
  border-radius: 8px;
  display: grid;
  place-items: center;
}

.icon-btn:hover {
  color: var(--color-primary);
  background: var(--surface-card-muted);
}

.session-state {
  padding: 14px;
  border-radius: 10px;
  background: var(--surface-card-muted);
  color: var(--color-text-secondary);
  font-size: 13px;
  text-align: center;
}

.session-list {
  flex: 1;
  min-height: 0;
  overflow: auto;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding-right: 2px;
}

.session-item {
  border: 1px solid transparent;
  border-radius: 10px;
  padding: 6px;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 32px;
  gap: 6px;
  align-items: center;
  background: transparent;
}

.session-item:hover,
.session-item.active {
  border-color: var(--stroke-soft);
  background: var(--surface-card-muted);
}

.session-select {
  min-width: 0;
  border: none;
  padding: 4px 2px 4px 4px;
  display: grid;
  gap: 5px;
  background: transparent;
  text-align: left;
  cursor: pointer;
}

.session-title {
  color: var(--color-text-heading);
  font-size: 13px;
  font-weight: 700;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.session-time {
  color: var(--color-text-muted);
  font-size: 12px;
}

.session-title-btn {
  width: 32px;
  height: 32px;
  border: none;
  border-radius: 8px;
  display: grid;
  place-items: center;
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
}

.session-title-btn:hover {
  color: var(--color-primary);
  background: var(--surface-card);
}

.session-title-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.session-title-btn svg {
  width: 15px;
  height: 15px;
}

.session-title-btn.spinning svg {
  animation: spin 1s linear infinite;
}

.chat-panel {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.config-warning {
  margin: 14px 14px 0;
  padding: 10px 12px;
  border: 1px solid rgba(220, 104, 3, 0.24);
  border-radius: 10px;
  background: rgba(220, 104, 3, 0.1);
  color: var(--color-warning);
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
}

.config-warning a {
  color: var(--color-primary);
  font-weight: 700;
}

.message-list {
  flex: 1;
  min-height: 0;
  overflow: auto;
  overscroll-behavior: contain;
  padding: 18px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.empty-chat {
  margin: auto;
  max-width: 620px;
  display: grid;
  justify-items: center;
  gap: 12px;
  color: var(--color-text-secondary);
  text-align: center;
}

.empty-chat > svg {
  width: 38px;
  height: 38px;
  color: var(--color-primary);
}

.empty-chat h3 {
  margin: 0;
  color: var(--color-text-heading);
  font-size: 20px;
}

.empty-chat p {
  margin: 0;
  font-size: 14px;
}

.prompt-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
  width: 100%;
}

.prompt-grid button {
  border: 1px solid var(--stroke-soft);
  border-radius: 10px;
  padding: 12px;
  background: var(--surface-card-muted);
  color: var(--color-text-heading);
  font-size: 13px;
  text-align: left;
  cursor: pointer;
}

.message-row {
  display: grid;
  grid-template-columns: 34px minmax(0, 1fr);
  gap: 10px;
  align-items: start;
}

.message-row--user {
  grid-template-columns: minmax(0, 1fr) 34px;
}

.message-row--user .message-avatar {
  grid-column: 2;
}

.message-row--user .message-bubble {
  grid-column: 1;
  grid-row: 1;
  justify-self: end;
  background: var(--color-primary);
  color: var(--color-text-inverse);
}

.message-row--user .message-meta,
.message-row--user .message-meta small {
  color: color-mix(in srgb, var(--color-text-inverse) 78%, transparent);
}

.message-avatar {
  width: 34px;
  height: 34px;
  border-radius: 10px;
  display: grid;
  place-items: center;
  background: var(--surface-card-muted);
  color: var(--color-primary);
}

.message-bubble {
  max-width: min(760px, 100%);
  padding: 12px 14px;
  border-radius: 12px;
  background: var(--surface-card-muted);
  color: var(--color-text-base);
}

.message-meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 6px;
  color: var(--color-text-muted);
  font-size: 12px;
  font-weight: 700;
}

.message-meta small {
  color: var(--color-text-muted);
  font-weight: 500;
}

.plain-message {
  margin: 0;
  white-space: pre-wrap;
  line-height: 1.7;
  font-size: 14px;
}

.thinking {
  opacity: 0.82;
}

.composer {
  border-top: 1px solid var(--stroke-soft);
  padding: 14px;
  display: grid;
  gap: 10px;
  flex: 0 0 auto;
  background: var(--surface-card);
}

.composer textarea {
  width: 100%;
  min-height: 82px;
  resize: vertical;
  border: 1px solid var(--stroke-soft);
  border-radius: 12px;
  padding: 12px;
  background: var(--surface-card-muted);
  color: var(--color-text-base);
  font: inherit;
  line-height: 1.55;
}

.composer textarea:focus {
  outline: none;
  border-color: var(--color-primary);
  box-shadow: var(--focus-ring);
}

.composer-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  color: var(--color-text-secondary);
  font-size: 13px;
}

.pill-btn {
  min-height: 38px;
  border-radius: 8px;
  border: 1px solid var(--stroke-soft);
  padding: 0 13px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  color: var(--color-text-heading);
  background: var(--surface-card);
  font-weight: 700;
  font-size: 14px;
  text-decoration: none;
  cursor: pointer;
}

.pill-btn.primary {
  border-color: var(--color-primary);
  background: var(--color-primary);
  color: var(--color-text-inverse);
}

.pill-btn.secondary {
  background: var(--surface-card-muted);
}

.pill-btn:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.pill-btn svg {
  width: 16px;
  height: 16px;
}

@media (max-width: 1180px) {
  .ai-workbench {
    grid-template-columns: 220px minmax(0, 1fr);
  }
}

@media (max-width: 860px) {
  .ai-workbench {
    grid-template-columns: 1fr;
    grid-template-rows: minmax(140px, 220px) minmax(0, 1fr);
    overflow: hidden;
  }

  .session-panel {
    max-height: none;
  }

  .prompt-grid {
    grid-template-columns: 1fr;
  }
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>

<template>
  <PageContainer
    :title="{ icon: 'lucide:database', text: '数据管理' }"
    subtitle="备份、恢复或清空您的学习数据"
    :custom-class="'settings-subpage'"
    max-width="full"
    fill-height
  >
    <div class="data-workbench">
      <div class="data-grid">
        <!-- 导出数据卡片 -->
        <div class="data-card export-card">
          <div class="card-icon">🗂️</div>
          <div class="card-text">
            <h4>导出全部数据</h4>
            <p>包含学习记录、阶段、分类、计划、成就、格言、倒计时等。</p>
          </div>
          <button
            type="button"
            class="pill-btn primary"
            :disabled="exporting"
            @click="handleExport"
          >
            <span>📦 导出 ZIP 备份</span>
          </button>
        </div>

        <!-- 导入数据卡片 -->
        <div
          class="data-card import-card"
          :class="{ dragging: dragging }"
          @dragover.prevent="dragging = true"
          @dragleave.prevent="dragging = false"
          @drop.prevent="onDrop"
        >
          <input
            ref="fileInput"
            type="file"
            accept=".zip"
            class="file-input"
            @change="onInputFile"
          />
          <div class="import-content" @click="fileInput?.click()">
            <div class="card-icon ghost">📦</div>
            <h4>导入备份数据</h4>
            <p class="desc">点击或拖拽 ZIP 文件到此处</p>
            <p class="warn">将覆盖当前所有数据且不可恢复</p>
          </div>
          <div v-if="selectedFile" class="file-info">
            <div class="file-name">{{ selectedFile.name }}</div>
            <div class="file-size">
              {{ (selectedFile.size / 1024).toFixed(1) }} KB
            </div>
            <div class="file-actions">
              <button
                class="pill-btn danger"
                type="button"
                :disabled="importing"
                @click="confirmImport"
              >
                {{ importing ? "正在导入..." : "导入并覆盖" }}
              </button>
              <button
                class="pill-btn ghost"
                type="button"
                :disabled="importing"
                @click="clearSelection"
              >
                取消
              </button>
            </div>
          </div>
        </div>

        <!-- 危险区域 -->
        <div class="danger-row">
          <div class="danger-left">
            <span class="danger-icon">🚨</span>
            <div>
              <div class="danger-title">清空所有数据</div>
              <div class="danger-desc">此操作不可恢复，请谨慎操作。</div>
            </div>
          </div>
          <button
            type="button"
            class="pill-btn danger solid"
            :disabled="clearing"
            @click="confirmClear"
          >
            🗑️ {{ clearing ? "正在清空..." : "立即清空" }}
          </button>
        </div>
      </div>

      <aside class="data-support-panel">
        <section>
          <p class="support-eyebrow">备份范围</p>
          <h4>一次导出完整学习工作区</h4>
          <ul>
            <li>学习记录与阶段关系</li>
            <li>分类、子分类与格言</li>
            <li>倒计时、成就和本地配置</li>
          </ul>
        </section>

        <section>
          <p class="support-eyebrow">导入要求</p>
          <h4>仅支持 ZIP 备份文件</h4>
          <p>
            导入会覆盖当前数据库。执行前建议先导出当前数据，保留一份可回退的备份。
          </p>
        </section>

        <section class="support-warning">
          <p class="support-eyebrow">风险区</p>
          <h4>清空数据不可撤销</h4>
          <p>清空后需要通过备份导入恢复，未导出的数据无法找回。</p>
        </section>
      </aside>
    </div>
  </PageContainer>
</template>

<script setup>
import { ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import PageContainer from "@/components/layout/PageContainer.vue";
import request, { extractRequestErrorMessage } from "@/utils/request";

defineOptions({ name: "DataSettingsView" });

const exporting = ref(false);
const importing = ref(false);
const clearing = ref(false);
const selectedFile = ref(null);
const dragging = ref(false);
const fileInput = ref(null);

function onInputFile(e) {
  const file = e.target.files[0];
  if (!file) return;
  if (!file.name.toLowerCase().endsWith(".zip")) {
    ElMessage.error("仅支持 .zip 文件");
    return;
  }
  selectedFile.value = file;
}

function onDrop(e) {
  dragging.value = false;
  const file = e.dataTransfer.files[0];
  if (!file) return;
  if (!file.name.toLowerCase().endsWith(".zip")) {
    ElMessage.error("仅支持 .zip 文件");
    return;
  }
  selectedFile.value = file;
}

function clearSelection() {
  selectedFile.value = null;
  if (fileInput.value) fileInput.value.value = "";
}

async function handleExport() {
  if (exporting.value) return;
  exporting.value = true;
  try {
    const res = await request.get("/api/records/export", {
      responseType: "blob",
    });
    const disp = res?.headers?.["content-disposition"] || "";
    const m = /filename\*=UTF-8''([^;]+)|filename="?([^;"]+)"?/i.exec(disp);
    const filename = decodeURIComponent(
      m?.[1] || m?.[2] || "records_backup.zip",
    );
    const blob = res?.data;
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
    ElMessage.success("ZIP 导出已开始下载");
  } catch (e) {
    console.error(e);
    ElMessage.error("导出失败");
  } finally {
    exporting.value = false;
  }
}

async function confirmImport() {
  if (!selectedFile.value) return;
  try {
    await ElMessageBox.confirm(
      "此操作将覆盖您当前的所有数据，且无法撤销。确定继续吗？",
      "确认导入",
      { type: "warning", confirmButtonText: "继续", cancelButtonText: "取消" },
    );
  } catch {
    return;
  }
  importing.value = true;
  try {
    const formData = new FormData();
    formData.append("file", selectedFile.value);
    const res = await request.post("/api/records/import_zip", formData, {
      headers: { "Content-Type": "multipart/form-data" },
    });
    if (res?.success) {
      ElMessage.success(res.message || "导入成功");
      selectedFile.value = null;
      setTimeout(() => window.location.reload(), 1200);
    } else {
      ElMessage.error(res?.message || "导入失败");
    }
  } catch (e) {
    console.error("导入错误:", e);
    const errorMsg = extractRequestErrorMessage(e) || "导入失败，请检查文件格式";
    ElMessage.error(errorMsg);
  } finally {
    importing.value = false;
  }
}

async function confirmClear() {
  try {
    await ElMessageBox.confirm(
      "最后警告：确定要永久清空账户的所有数据吗？此操作无法恢复！",
      "清空数据",
      {
        type: "error",
        confirmButtonText: "是的，清空",
        cancelButtonText: "取消",
        confirmButtonClass: "el-button--danger",
      },
    );
  } catch {
    return;
  }
  clearing.value = true;
  try {
    const res = await request.post("/api/records/clear_data", {});
    if (res?.success) {
      ElMessage.success(res?.message || "数据已清空");
    } else {
      ElMessage.error(res?.message || "清空失败");
    }
  } catch (e) {
    console.error(e);
    ElMessage.error("清空失败");
  } finally {
    clearing.value = false;
  }
}
</script>

<style scoped>
.data-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
  align-items: stretch;
  align-content: start;
  min-width: 0;
}

.data-workbench {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 300px;
  gap: 18px;
  align-items: start;
}

.data-card {
  background: var(--surface-card);
  border: 1px solid var(--stroke-soft);
  border-radius: 14px;
  padding: 18px;
  box-shadow: var(--box-shadow-card);
  display: flex;
  flex-direction: column;
  gap: 14px;
  min-height: 220px;
}

.data-card h4 {
  margin: 0;
  font-size: 17px;
  font-weight: 800;
  color: var(--color-text-heading);
}

.data-card p {
  margin: 0;
  color: var(--color-text-secondary);
  font-size: 13px;
}

.card-icon {
  width: 44px;
  height: 44px;
  border-radius: 12px;
  display: grid;
  place-items: center;
  background: var(--color-primary-light);
  font-size: 20px;
  color: var(--color-primary);
  box-shadow: inset 0 1px 0 var(--surface-card);
}

.card-icon.ghost {
  background: var(--surface-card-muted);
  color: var(--color-primary);
}

.card-text {
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex: 1;
  justify-content: flex-start;
}

.pill-btn {
  border: none;
  border-radius: 8px;
  min-height: 40px;
  padding: 0 14px;
  font-weight: 700;
  font-size: 14px;
  cursor: pointer;
  transition:
    transform 0.15s ease,
    box-shadow 0.2s ease,
    opacity 0.15s ease;
  align-self: flex-start;
}

.data-card > .pill-btn {
  align-self: stretch;
}

.pill-btn.primary {
  background: linear-gradient(
    135deg,
    var(--color-primary),
    var(--color-primary-dark)
  );
  color: var(--color-text-inverse);
  box-shadow: var(--box-shadow);
}

.pill-btn.primary:disabled {
  opacity: 0.6;
  cursor: not-allowed;
  box-shadow: none;
}

.pill-btn.ghost {
  background: var(--surface-card-muted);
  color: var(--color-text-secondary);
  border: 1px solid var(--stroke-soft);
}

.pill-btn.danger {
  background: rgba(239, 68, 68, 0.14);
  color: var(--color-error);
  border: 1px solid rgba(239, 68, 68, 0.28);
}

.pill-btn.danger.solid {
  background: var(--color-error);
  color: var(--color-text-inverse);
  border: none;
  box-shadow: var(--box-shadow);
}

.import-card {
  background: var(--surface-card-muted);
  border: 1px dashed var(--stroke-strong);
  justify-content: center;
  position: relative;
  overflow: hidden;
}

.import-card.dragging {
  border-color: var(--color-primary);
  background: var(--surface-subtle);
}

.file-input {
  position: absolute;
  inset: 0;
  opacity: 0;
  cursor: pointer;
}

.import-content {
  text-align: center;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  cursor: pointer;
}

.import-content h4 {
  font-size: 17px;
  margin: 0;
}

.import-content .desc {
  font-size: 13px;
  color: var(--color-text-secondary);
}

.import-content .warn {
  font-size: 12px;
  color: var(--color-warning);
}

.file-info {
  margin-top: auto;
  background: var(--surface-card);
  border-radius: 12px;
  padding: 10px 12px;
  border: 1px solid var(--stroke-soft);
  display: flex;
  flex-direction: column;
  gap: 4px;
  position: relative;
  z-index: 10;
}

.file-actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.danger-row {
  grid-column: span 2;
  background: rgba(239, 68, 68, 0.1);
  border: 1px solid rgba(239, 68, 68, 0.25);
  border-radius: 14px;
  min-height: 92px;
  padding: 18px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.danger-row .pill-btn {
  align-self: center;
  flex: 0 0 auto;
  width: auto;
  min-width: 112px;
  height: 40px;
  border-radius: 8px;
  padding: 0 16px;
}

.danger-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.danger-icon {
  font-size: 22px;
}

.danger-title {
  font-weight: 800;
  color: var(--color-error);
  font-size: 15px;
}

.danger-desc {
  color: var(--color-text-secondary);
  font-size: 13px;
}

.data-support-panel {
  min-width: 0;
  padding: 14px;
  border: 1px solid var(--stroke-soft);
  border-radius: 14px;
  background: var(--surface-card);
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.data-support-panel section {
  padding: 14px 16px;
  border-radius: 12px;
  background: var(--surface-card-muted);
  border: 1px solid var(--stroke-soft);
}

.data-support-panel h4 {
  margin: 0 0 10px;
  font-size: 15px;
  color: var(--color-text-heading);
}

.data-support-panel p {
  margin: 0;
  color: var(--color-text-secondary);
  font-size: 13px;
  line-height: 1.65;
}

.support-eyebrow {
  margin-bottom: 6px !important;
  color: var(--color-text-muted) !important;
  font-size: 11px !important;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.data-support-panel ul {
  margin: 0;
  padding-left: 18px;
  display: grid;
  gap: 8px;
  color: var(--color-text-secondary);
  font-size: 13px;
}

.support-warning {
  margin-top: 0;
}

@media (max-width: 900px) {
  .data-workbench {
    grid-template-columns: 1fr;
  }

  .data-grid {
    grid-template-columns: 1fr;
  }

  .danger-row {
    grid-column: span 1;
    flex-direction: column;
    align-items: flex-start;
  }
}
</style>

<template>
  <PageContainer
    :title="{ icon: 'lucide:graduation-cap', text: '课程画像' }"
    subtitle="把课程学分、成绩和学习记录放在同一张分析表里"
    :custom-class="'settings-subpage'"
    max-width="full"
    fill-height
  >
    <template #actions>
      <div class="actions course-actions">
        <button class="pill-btn primary" @click="openImportDialog">
          <Icon icon="lucide:file-spreadsheet" />
          导入课程
        </button>
        <button class="pill-btn secondary" @click="courseListVisible = true">
          <Icon icon="lucide:list" />
          课程数据
        </button>
        <button class="pill-btn secondary" :disabled="store.loading" @click="refresh">
          <Icon icon="lucide:refresh-cw" />
          刷新
        </button>
        <el-dropdown trigger="click" @command="handleHeaderCommand">
          <button class="pill-btn secondary more-action" type="button">
            <Icon icon="lucide:more-horizontal" />
            更多
          </button>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item command="template">
                <Icon icon="lucide:download" />
                下载模板
              </el-dropdown-item>
              <el-dropdown-item command="settings" divided>
                <Icon icon="lucide:settings-2" />
                课程设置
              </el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
      </div>
    </template>

    <div class="course-profile-main">
        <section class="summary-grid">
          <div v-for="card in summaryCards" :key="card.label" class="summary-card">
            <span>{{ card.label }}</span>
            <strong>{{ card.value }}</strong>
            <small>{{ card.meta }}</small>
          </div>
        </section>

        <section v-if="store.preview" class="data-card mapping-workbench">
          <div class="section-head">
            <div>
              <span class="section-label">映射工作台</span>
              <h2>检查推荐匹配结果</h2>
            </div>
            <div class="section-actions">
              <button class="pill-btn outline" @click="store.preview = null">取消预览</button>
              <button
                class="pill-btn primary"
                :disabled="store.importing"
                @click="store.confirmImport"
              >
                确认入库
              </button>
            </div>
          </div>
          <div class="mapping-summary">
            <div>
              <strong>{{ previewMatchedCount }}</strong>
              <span>已推荐匹配</span>
            </div>
            <div>
              <strong>{{ store.preview.mismatches?.unmatched_courses || 0 }}</strong>
              <span>未匹配课程</span>
            </div>
            <div>
              <strong>{{ store.preview.mismatches?.unlinked_subcategories?.length || 0 }}</strong>
              <span>无课程子分类</span>
            </div>
            <div>
              <strong>{{ store.preview.mismatches?.duplicate_matches?.length || 0 }}</strong>
              <span>共享映射</span>
            </div>
          </div>
          <el-table :data="store.preview.items" class="mapping-table" max-height="420">
            <el-table-column prop="semester" label="学期" min-width="120" />
            <el-table-column prop="course_name" label="课程名称" min-width="180" />
            <el-table-column prop="credits" label="学分" width="90" />
            <el-table-column label="成绩" width="110">
              <template #default="{ row }">
                <span>{{ row.grade ?? "未出" }}</span>
              </template>
            </el-table-column>
            <el-table-column label="匹配子分类" min-width="220">
              <template #default="{ row }">
                <el-select
                  v-model="row.matched_subcategory_id"
                  placeholder="暂不匹配"
                  clearable
                  filterable
                  @change="handlePreviewMatchChange(row)"
                >
                  <el-option
                    v-for="candidate in store.preview?.candidates || []"
                    :key="candidate.id"
                    :label="candidate.name"
                    :value="candidate.id"
                  />
                </el-select>
              </template>
            </el-table-column>
            <el-table-column label="状态" width="120">
              <template #default="{ row }">
                <el-tag :type="row.matched_subcategory_id ? 'success' : 'warning'" effect="plain">
                  {{ row.matched_subcategory_id ? "已匹配" : "未匹配" }}
                </el-tag>
              </template>
            </el-table-column>
          </el-table>
        </section>

        <section class="data-card analysis-panel">
          <div class="section-head analysis-head">
            <div>
              <span class="section-label">课程对比</span>
              <h2>{{ activeMetricView.label || "多指标分析" }}</h2>
              <p>{{ activeMetricView.description || "切换指标查看课程投入、重要度和结果表现。" }}</p>
            </div>
            <div class="metric-switch">
              <button
                v-for="metric in metricOptions"
                :key="metric.key"
                type="button"
                :class="{ active: activeMetric === metric.key }"
                @click="activeMetric = metric.key"
              >
                {{ metric.label }}
              </button>
            </div>
          </div>
          <CategoryComposite
            v-if="hasMetricData"
            class="course-profile-distribution"
            :main="metricCompositeData"
            :bar-main="metricCompositeBarData"
            :drilldown="{}"
            :show-panel-header="false"
            :metric-mode="compositeMetricMode"
            :main-title="activeMetricView.pie_title || '占比分布'"
            :bar-main-title="activeMetricView.bar_title || '课程排名'"
            :doughnut-subtitle="activeMetricView.description || ''"
            :value-unit="metricPieValueUnit"
            :bar-value-unit="metricBarValueUnit"
            total-label="合计"
          />
          <div v-else class="analysis-empty">
            <Icon icon="lucide:chart-no-axes-combined" />
            <h3>暂无课程分析数据</h3>
            <p>导入课程成绩表并完成课程映射后，这里会显示时长、效率、学分和成绩的多指标分析。</p>
          </div>
        </section>
    </div>

    <el-dialog
      v-model="importVisible"
      title="导入课程成绩表"
      width="560px"
      :close-on-click-modal="false"
      destroy-on-close
    >
      <div class="import-dialog-body">
        <section class="dialog-info-card">
          <p class="support-eyebrow">课程来源</p>
          <h4>{{ sourceStatusText }}</h4>
          <p>课程来源只影响导入时的推荐匹配候选，不会自动改写已保存的课程映射。</p>
        </section>

        <section
          class="import-card"
          :class="{ dragging: importDragging }"
          @dragover.prevent="importDragging = true"
          @dragleave.prevent="importDragging = false"
          @drop.prevent="handleImportDrop"
        >
          <input
            ref="fileInput"
            type="file"
            accept=".csv,.xlsx"
            class="file-input"
            @change="handleFileChange"
          />
          <div class="import-content" @click="fileInput?.click()">
            <div class="card-icon ghost">
              <Icon icon="lucide:file-spreadsheet" />
            </div>
            <h4>{{ store.importing ? "正在解析文件" : "选择课程成绩表" }}</h4>
            <p class="desc">点击或拖拽 CSV、XLSX 文件到此处</p>
            <p class="warn">确认预览后才写入课程画像</p>
          </div>
        </section>

        <section class="dialog-info-card">
          <p class="support-eyebrow">导入要求</p>
          <h4>支持成绩未出的课程</h4>
          <ul>
            <li>字段包含学期、课程名称、学分、成绩。</li>
            <li>成绩可留空，系统会标记为未出成绩。</li>
            <li>课程数量不匹配时在映射工作台确认。</li>
          </ul>
        </section>
      </div>
      <template #footer>
        <button class="pill-btn secondary" @click="importVisible = false">关闭</button>
        <button
          class="pill-btn secondary"
          :disabled="store.templateDownloading"
          @click="downloadTemplate"
        >
          下载模板
        </button>
      </template>
    </el-dialog>

    <el-drawer
      v-model="courseListVisible"
      title="课程数据"
      size="72%"
      destroy-on-close
    >
      <div class="course-list-drawer">
        <section class="drawer-summary">
          <div>
            <span>已出成绩学分</span>
            <strong>{{ numberText(summary?.graded_credits) }}</strong>
          </div>
          <div>
            <span>待出成绩学分</span>
            <strong>{{ numberText(summary?.pending_credits) }}</strong>
          </div>
          <div>
            <span>未匹配课程</span>
            <strong>{{ summary?.unmatched_count || 0 }}</strong>
          </div>
          <div>
            <span>共享映射课程</span>
            <strong>{{ summary?.shared_mapping_count || 0 }}</strong>
          </div>
        </section>

        <section class="drawer-table-card">
          <div class="section-head">
            <div>
              <span class="section-label">课程列表</span>
              <h2>已有科目的课程画像数据</h2>
            </div>
            <div class="filters">
              <el-select v-model="store.semesterFilter" class="filter-select" @change="refreshData">
                <el-option label="全部学期" value="all" />
                <el-option
                  v-for="semester in store.semesters"
                  :key="semester"
                  :label="semester"
                  :value="semester"
                />
              </el-select>
              <el-select v-model="store.matchStatusFilter" class="filter-select" @change="refreshData">
                <el-option label="全部匹配状态" value="all" />
                <el-option label="已自动匹配" value="auto" />
                <el-option label="已手动匹配" value="manual" />
                <el-option label="未匹配" value="unmatched" />
              </el-select>
            </div>
          </div>

          <el-table v-loading="store.loading" :data="store.courses" class="courses-table" height="520">
            <el-table-column prop="semester" label="学期" min-width="115" />
            <el-table-column prop="course_name" label="课程" min-width="180" />
            <el-table-column label="学分" width="90">
              <template #default="{ row }">
                <span>{{ row.credits > 0 ? numberText(row.credits) : "待补" }}</span>
              </template>
            </el-table-column>
            <el-table-column label="成绩" width="100">
              <template #default="{ row }">
                <el-tag v-if="row.grade_status === 'pending'" type="info" effect="plain">未出</el-tag>
                <span v-else>{{ row.grade }}</span>
              </template>
            </el-table-column>
            <el-table-column label="匹配" min-width="160">
              <template #default="{ row }">
                <div class="match-cell">
                  <span>{{ row.matched_subcategory_name || "未匹配" }}</span>
                  <el-tag
                    v-if="row.shared_mapping_count > 1"
                    type="warning"
                    size="small"
                    effect="plain"
                  >
                    共享
                  </el-tag>
                </div>
              </template>
            </el-table-column>
            <el-table-column label="学习时长" width="110">
              <template #default="{ row }">{{ numberText(row.learning_hours) }}h</template>
            </el-table-column>
            <el-table-column label="效率" width="100">
              <template #default="{ row }">{{ row.efficiency ?? "--" }}</template>
            </el-table-column>
            <el-table-column label="投入匹配" width="120">
              <template #default="{ row }">
                <el-tag :type="fitTagType(analysisById[row.id]?.duration_fit_label)" effect="plain">
                  {{ analysisById[row.id]?.duration_fit_label || "--" }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column label="成绩效果" width="120">
              <template #default="{ row }">
                <el-tag :type="gradeTagType(analysisById[row.id]?.grade_effect_label)" effect="plain">
                  {{ analysisById[row.id]?.grade_effect_label || "待观察" }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column label="成绩贡献" width="110">
              <template #default="{ row }">
                {{ valueText(analysisById[row.id]?.grade_credit_contribution) }}
              </template>
            </el-table-column>
            <el-table-column label="成绩回报" width="110">
              <template #default="{ row }">
                {{ valueText(analysisById[row.id]?.grade_return_index) }}
              </template>
            </el-table-column>
            <el-table-column label="诊断" min-width="220">
              <template #default="{ row }">
                <div class="diagnosis-cell">
                  <el-tag
                    v-for="tag in visibleDiagnosisTags(row.id)"
                    :key="tag"
                    :type="diagnosisTagType(tag)"
                    size="small"
                    effect="plain"
                  >
                    {{ tag }}
                  </el-tag>
                </div>
              </template>
            </el-table-column>
            <el-table-column label="操作" width="150" fixed="right">
              <template #default="{ row }">
                <button class="table-action" @click="openEditDialog(row)">编辑</button>
                <button
                  v-if="row.is_profile_enriched"
                  class="table-action danger"
                  @click="deleteCourse(row)"
                >
                  清除资料
                </button>
              </template>
            </el-table-column>
          </el-table>
        </section>
      </div>
    </el-drawer>

    <el-dialog
      v-model="settingsVisible"
      title="课程画像设置"
      width="520px"
      :close-on-click-modal="false"
      destroy-on-close
    >
      <div class="settings-dialog-body">
        <div class="settings-note">
          <Icon icon="lucide:info" />
          <p>
            课程来源父分类只影响后续导入时的推荐匹配候选，不会自动改写已经保存的课程映射。
          </p>
        </div>
        <label class="settings-label">课程来源父分类</label>
        <el-select
          v-model="selectedSourceCategoryId"
          class="source-select settings-source-select"
          placeholder="选择父分类"
          filterable
          :loading="categoryStore.loading || store.settingsLoading"
        >
          <el-option
            v-for="category in categoryStore.tree"
            :key="category.id"
            :label="category.name"
            :value="category.id"
          />
        </el-select>
      </div>
      <template #footer>
        <button class="pill-btn secondary" @click="settingsVisible = false">
          取消
        </button>
        <button
          class="pill-btn primary"
          :disabled="store.settingsLoading"
          @click="saveSourceCategory"
        >
          保存设置
        </button>
      </template>
    </el-dialog>

    <el-dialog
      v-model="editVisible"
      title="编辑课程资料"
      width="520px"
      :close-on-click-modal="false"
      destroy-on-close
    >
      <el-form v-if="editingCourse" label-position="top" class="edit-form">
        <el-form-item label="学期">
          <el-input v-model="editingCourse.semester" />
        </el-form-item>
        <el-form-item label="已有科目">
          <el-input v-model="editingCourse.course_name" />
        </el-form-item>
        <div class="edit-grid">
          <el-form-item label="学分">
            <el-input-number v-model="editingCourse.credits" :min="0.1" :step="0.5" />
          </el-form-item>
          <el-form-item label="成绩">
            <el-input-number
              v-model="editingCourse.grade"
              :min="0"
              :max="100"
              :step="1"
              controls-position="right"
              placeholder="留空表示未出"
            />
          </el-form-item>
        </div>
        <el-form-item label="关联已有科目">
          <el-select
            v-model="editingCourse.matched_subcategory_id"
            placeholder="暂不匹配"
            clearable
            filterable
          >
            <el-option
              v-for="candidate in sourceSubcategories"
              :key="candidate.id"
              :label="candidate.name"
              :value="candidate.id"
            />
          </el-select>
        </el-form-item>
      </el-form>
      <template #footer>
        <button class="pill-btn secondary" @click="editVisible = false">取消</button>
        <button class="pill-btn primary" @click="submitEdit">保存</button>
      </template>
    </el-dialog>
  </PageContainer>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { Icon } from "@iconify/vue";
import { ElMessage, ElMessageBox } from "element-plus";
import PageContainer from "@/components/layout/PageContainer.vue";
import CategoryComposite from "@/components/business/charts/CategoryComposite.vue";
import { useCategoryStore } from "@/stores/category";
import {
  type CourseProfileCourse,
  useCourseProfileStore,
} from "@/stores/modules/courseProfile";

const store = useCourseProfileStore();
const categoryStore = useCategoryStore();
const fileInput = ref<HTMLInputElement | null>(null);
const selectedSourceCategoryId = ref<number | null>(null);
const importVisible = ref(false);
const courseListVisible = ref(false);
const settingsVisible = ref(false);
const editVisible = ref(false);
const editingCourse = ref<any | null>(null);
const importDragging = ref(false);
const activeMetric = ref("duration_credit");

const metricOptions = [
  { key: "duration", label: "时长" },
  { key: "efficiency", label: "效率" },
  { key: "credits", label: "学分" },
  { key: "duration_credit", label: "投入匹配" },
  { key: "efficiency_credit", label: "效率匹配" },
  { key: "grade", label: "成绩表现" },
  { key: "grade_contribution", label: "成绩贡献" },
  { key: "grade_return", label: "成绩回报" },
  { key: "diagnosis", label: "诊断" },
];

const summary = computed(() => store.summary || {});

const metricViews = computed(() => summary.value.metric_views || {});

const activeMetricView = computed(() => {
  const fallback = metricViews.value.duration || {};
  return metricViews.value[activeMetric.value] || fallback;
});

const hasMetricData = computed(() => {
  const view = activeMetricView.value;
  return Boolean(view.bar?.items?.length || view.pie?.items?.length);
});

const metricCompositeData = computed(() => {
  const sourceItems = activeMetricView.value.pie?.items?.length
    ? activeMetricView.value.pie.items
    : activeMetricView.value.bar?.items || [];
  const items = sourceItems
    .map((item: any) => ({
      name: item.name,
      value: Number(item.value ?? 0),
    }))
    .filter((item: any) => item.name && Number.isFinite(item.value));
  return {
    labels: items.map((item: any) => item.name),
    data: items.map((item: any) => item.value),
  };
});

const metricCompositeBarData = computed(() => {
  const sourceItems = activeMetricView.value.bar?.items?.length
    ? activeMetricView.value.bar.items
    : activeMetricView.value.pie?.items || [];
  const items = sourceItems
    .map((item: any) => ({
      name: item.name,
      value: Number(item.value ?? 0),
    }))
    .filter((item: any) => item.name && Number.isFinite(item.value));
  return {
    labels: items.map((item: any) => item.name),
    data: items.map((item: any) => item.value),
  };
});

const compositeMetricMode = computed(() =>
  activeMetric.value === "efficiency" ? "efficiency" : "duration",
);

const metricBarValueUnit = computed(() => {
  const unit = activeMetricView.value.bar_unit || activeMetricView.value.unit || "";
  if (unit === "h") return "h";
  return unit;
});

const metricPieValueUnit = computed(() => {
  const unit = activeMetricView.value.pie_unit || activeMetricView.value.unit || "";
  if (unit === "h") return "h";
  return unit;
});

const analysisRows = computed(() => summary.value.analysis_rows || []);

const analysisById = computed(() => {
  const map: Record<number, any> = {};
  for (const row of analysisRows.value) {
    if (row.id != null) {
      map[Number(row.id)] = row;
    }
  }
  return map;
});

const sourceStatusText = computed(() => {
  if (store.sourceCategory?.name) {
    return store.sourceCategory.name;
  }
  if (store.sourceCategoryId) {
    const category = categoryStore.tree.find(
      (item: any) => item.id === store.sourceCategoryId,
    );
    return category?.name || "已设置";
  }
  return "未设置，导入前需要指定父分类";
});

const sourceSubcategories = computed(() => {
  const category = categoryStore.tree.find(
    (item: any) => item.id === store.sourceCategoryId,
  );
  return category?.subcategories || category?.children || [];
});

const previewMatchedCount = computed(
  () =>
    store.preview?.items.filter((item: any) => item.matched_subcategory_id)
      .length || 0,
);

const summaryCards = computed(() => [
  {
    label: "总学分",
    value: numberText(summary.value.total_credits),
    meta: `${summary.value.total_courses || 0} 门课程`,
  },
  {
    label: "加权均分",
    value: scoreText(summary.value.weighted_grade),
    meta: `覆盖率 ${summary.value.grade_coverage || 0}%`,
  },
  {
    label: "平均成绩",
    value: scoreText(summary.value.average_grade),
    meta: "仅统计已出成绩课程",
  },
  {
    label: "学习时长",
    value: `${numberText(summary.value.total_learning_hours)}h`,
    meta: `${summary.value.matched_count || 0} 门已匹配`,
  },
]);

function numberText(value: unknown) {
  const num = Number(value ?? 0);
  if (!Number.isFinite(num)) return "--";
  return num.toFixed(Math.abs(num % 1) > 0 ? 1 : 0);
}

function scoreText(value: unknown) {
  if (value === null || value === undefined) return "--";
  const num = Number(value);
  return Number.isFinite(num) ? num.toFixed(2) : "--";
}

function valueText(value: unknown) {
  if (value === null || value === undefined) return "--";
  const num = Number(value);
  if (!Number.isFinite(num)) return "--";
  return num.toFixed(Math.abs(num % 1) > 0 ? 1 : 0);
}

function visibleDiagnosisTags(courseId: number) {
  const tags = analysisById.value[courseId]?.diagnosis_tags || [];
  return tags.length ? tags.slice(0, 3) : ["状态稳定"];
}

function fitTagType(label?: string) {
  if (label === "投入匹配") return "success";
  if (label === "投入偏少" || label === "投入偏多") return "warning";
  if (label === "缺少学习记录") return "info";
  return "info";
}

function gradeTagType(label?: string) {
  if (label === "高于均分") return "success";
  if (label === "低于均分") return "danger";
  if (label === "接近均分") return "info";
  return "info";
}

function diagnosisTagType(tag: string) {
  if (
    [
      "高学分低投入",
      "高投入低成绩",
      "投入匹配但效率偏低",
      "投入偏少",
      "投入不足",
      "效率偏低",
      "映射不确定",
    ].includes(tag)
  ) {
    return "warning";
  }
  if (
    ["重投入高回报", "低投入高成绩", "低投入高结果", "高效优势", "状态稳定"].includes(
      tag,
    )
  ) {
    return "success";
  }
  return "info";
}

async function refreshData() {
  await Promise.all([store.fetchCourses(), store.fetchSummary()]);
}

async function refresh() {
  await Promise.all([categoryStore.fetchAll(true), store.refreshAll()]);
  selectedSourceCategoryId.value = store.sourceCategoryId;
}

function openImportDialog() {
  importVisible.value = true;
}

function handleHeaderCommand(command: string) {
  if (command === "template") {
    downloadTemplate();
    return;
  }
  if (command === "settings") {
    openSettingsDialog();
  }
}

async function openSettingsDialog() {
  await categoryStore.ensureLoaded();
  selectedSourceCategoryId.value = store.sourceCategoryId;
  settingsVisible.value = true;
}

async function saveSourceCategory() {
  const previous = store.sourceCategoryId;
  if (selectedSourceCategoryId.value === previous) {
    settingsVisible.value = false;
    return;
  }
  try {
    await ElMessageBox.confirm(
      "更改课程来源会影响后续导入的推荐匹配候选，不会自动修改已入库课程映射。确认保存新的课程来源吗？",
      "确认更改课程来源",
      {
        confirmButtonText: "保存设置",
        cancelButtonText: "取消",
        type: "warning",
      },
    );
  } catch (error) {
    if (error === "cancel" || error === "close") return;
    throw error;
  }
  await store.saveSettings(selectedSourceCategoryId.value);
  settingsVisible.value = false;
}

async function handleFileChange(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = "";
  await handleImportFile(file);
}

async function handleImportDrop(event: DragEvent) {
  importDragging.value = false;
  const file = event.dataTransfer?.files?.[0];
  await handleImportFile(file);
}

async function handleImportFile(file?: File) {
  if (!file) return;
  if (!store.sourceCategoryId) {
    ElMessage.warning("请先设置课程来源父分类");
    openSettingsDialog();
    return;
  }
  await store.previewImport(file);
  if (store.preview) {
    importVisible.value = false;
  }
}

function handlePreviewMatchChange(row: any) {
  const candidate = store.preview?.candidates.find(
    (item) => item.id === row.matched_subcategory_id,
  );
  row.matched_subcategory_name = candidate?.name || null;
  row.match_status = row.matched_subcategory_id ? "manual" : "unmatched";
}

async function downloadTemplate() {
  await store.downloadTemplate();
}

function openEditDialog(row: CourseProfileCourse) {
  editingCourse.value = {
    ...row,
    semester: row.semester || "未设置",
    credits: row.credits > 0 ? row.credits : 0,
  };
  editVisible.value = true;
}

async function submitEdit() {
  if (!editingCourse.value) return;
  await store.updateCourse(editingCourse.value.id, {
    semester: editingCourse.value.semester,
    course_name: editingCourse.value.course_name,
    credits: Number(editingCourse.value.credits),
    grade:
      editingCourse.value.grade === null || editingCourse.value.grade === ""
        ? null
        : Number(editingCourse.value.grade),
    matched_subcategory_id: editingCourse.value.matched_subcategory_id || null,
    match_status: editingCourse.value.matched_subcategory_id
      ? "manual"
      : "unmatched",
  });
  editVisible.value = false;
}

async function deleteCourse(row: CourseProfileCourse) {
  if (!row.is_profile_enriched) return;
  try {
    await ElMessageBox.confirm(
      `确认清除“${row.course_name}”的学分和成绩资料吗？已有科目和学习记录不会删除。`,
      "清除课程资料",
      {
        confirmButtonText: "清除资料",
        cancelButtonText: "取消",
        type: "warning",
      },
    );
  } catch (error) {
    if (error === "cancel" || error === "close") return;
    throw error;
  }
  await store.deleteCourse(row.id);
}

watch(
  () => store.sourceCategoryId,
  (value) => {
    selectedSourceCategoryId.value = value;
  },
);

onMounted(async () => {
  await Promise.all([categoryStore.fetchAll(), store.refreshAll()]);
  selectedSourceCategoryId.value = store.sourceCategoryId;
});
</script>

<style scoped lang="scss">
.course-actions,
.section-actions,
.filters {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.course-actions :deep(svg),
.pill-btn :deep(svg),
.card-icon :deep(svg) {
  width: 16px;
  height: 16px;
}

.course-profile-main {
  min-width: 0;
  display: grid;
  gap: 16px;
  align-content: start;
}

.more-action {
  padding-inline: 10px;
}

:deep(.el-dropdown-menu__item) {
  display: flex;
  align-items: center;
  gap: 8px;
}

:deep(.el-dropdown-menu__item svg) {
  width: 15px;
  height: 15px;
}

.data-card {
  background: var(--surface-card);
  border: 1px solid var(--stroke-soft);
  border-radius: 14px;
  padding: 18px;
  box-shadow: var(--box-shadow-card);
  min-width: 0;
}

.section-head {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 14px;
  margin-bottom: 14px;
}

.section-head.compact {
  margin-bottom: 10px;
}

.analysis-head {
  align-items: flex-start;
}

.section-head h2 {
  margin: 4px 0 0;
  color: var(--color-text-heading);
  font-size: 17px;
  font-weight: 800;
  letter-spacing: 0;
}

.section-head p {
  margin: 6px 0 0;
  max-width: 560px;
  color: var(--color-text-secondary);
  font-size: 13px;
  line-height: 1.55;
}

.section-label,
.support-eyebrow {
  color: var(--color-text-muted);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.summary-grid,
.mapping-summary {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 12px;
}

.summary-card,
.mapping-summary div,
.drawer-summary div {
  border: 1px solid var(--stroke-soft);
  background: var(--surface-card-muted);
  border-radius: 12px;
  padding: 12px;
  min-width: 0;
}

.summary-card {
  display: grid;
  gap: 7px;
}

.summary-card span,
.summary-card small,
.mapping-summary span,
.drawer-summary span {
  color: var(--color-text-secondary);
  font-size: 13px;
  line-height: 1.35;
}

.summary-card strong,
.mapping-summary strong,
.drawer-summary strong {
  display: block;
  color: var(--color-text-heading);
  font-size: 23px;
  line-height: 1.1;
}

.mapping-summary {
  margin-bottom: 14px;
}

.metric-switch {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px;
  border: 1px solid var(--stroke-soft);
  border-radius: 10px;
  background: var(--surface-card-muted);
  flex-wrap: wrap;
  justify-content: flex-end;
}

.metric-switch button {
  border: none;
  border-radius: 8px;
  min-height: 30px;
  padding: 0 10px;
  background: transparent;
  color: var(--color-text-secondary);
  font-size: 13px;
  font-weight: 700;
  cursor: pointer;
}

.metric-switch button.active {
  background: var(--surface-card);
  color: var(--color-primary);
  box-shadow: var(--box-shadow-card);
}

.course-profile-distribution {
  margin-top: 4px;
}

.analysis-empty {
  min-height: 300px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 28px;
  border: 1px dashed var(--stroke-soft);
  border-radius: 12px;
  background: var(--surface-card-muted);
  text-align: center;
}

.analysis-empty > svg {
  width: 32px;
  height: 32px;
  color: var(--color-primary);
}

.analysis-empty h3 {
  margin: 0;
  font-size: 17px;
  font-weight: 800;
  color: var(--color-text-heading);
}

.analysis-empty p {
  margin: 0;
  max-width: 460px;
  color: var(--color-text-secondary);
  font-size: 13px;
  line-height: 1.6;
}

.support-eyebrow {
  display: block;
  margin: 0 0 6px;
}

.import-dialog-body,
.course-list-drawer {
  display: grid;
  gap: 14px;
}

.dialog-info-card,
.drawer-table-card {
  border: 1px solid var(--stroke-soft);
  border-radius: 12px;
  background: var(--surface-card-muted);
  padding: 14px;
  min-width: 0;
}

.dialog-info-card h4 {
  margin: 0 0 8px;
  color: var(--color-text-heading);
  font-size: 15px;
  font-weight: 800;
}

.dialog-info-card p,
.dialog-info-card li {
  color: var(--color-text-secondary);
  font-size: 13px;
  line-height: 1.6;
}

.dialog-info-card p {
  margin: 0;
}

.dialog-info-card ul {
  margin: 0;
  padding-left: 18px;
  display: grid;
  gap: 6px;
}

.drawer-summary {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 12px;
}

.drawer-table-card {
  background: var(--surface-card);
  overflow: hidden;
}

.import-card {
  min-height: 180px;
  padding: 24px;
  border-style: dashed !important;
  border-color: var(--stroke-strong) !important;
  cursor: pointer;
  position: relative;
  overflow: hidden;
}

.import-card.dragging {
  border-color: var(--color-primary) !important;
  background: var(--surface-subtle);
}

.file-input {
  position: absolute;
  inset: 0;
  opacity: 0;
  pointer-events: none;
}

.import-content {
  text-align: center;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
}

.card-icon {
  width: 44px;
  height: 44px;
  border-radius: 12px;
  display: grid;
  place-items: center;
  background: var(--surface-card);
  color: var(--color-primary);
  box-shadow: inset 0 1px 0 var(--surface-card);
}

.card-icon :deep(svg) {
  width: 22px;
  height: 22px;
}

.import-content h4 {
  margin: 0;
  font-size: 17px;
  font-weight: 800;
  color: var(--color-text-heading);
}

.import-content .desc {
  font-size: 13px;
  color: var(--color-text-secondary);
}

.import-content .warn {
  font-size: 12px;
  color: var(--color-warning);
}

.match-cell {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}

.diagnosis-cell {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.table-action {
  border: none;
  background: transparent;
  color: var(--color-primary);
  font-weight: 600;
  cursor: pointer;
  margin-right: 10px;
}

.table-action.danger {
  color: var(--color-error);
}

.filter-select {
  width: 150px;
}

.source-select,
.settings-source-select {
  width: 100%;
}

.settings-dialog-body {
  display: grid;
  gap: 14px;
}

.settings-note {
  display: grid;
  grid-template-columns: 20px minmax(0, 1fr);
  gap: 10px;
  padding: 12px;
  border: 1px solid var(--stroke-soft);
  border-radius: 12px;
  background: var(--surface-card-muted);
}

.settings-note svg {
  width: 18px;
  height: 18px;
  color: var(--color-primary);
  margin-top: 2px;
}

.settings-note p {
  margin: 0;
  color: var(--color-text-secondary);
  font-size: 13px;
  line-height: 1.6;
}

.settings-label {
  display: block;
  color: var(--color-text-secondary);
  font-size: 13px;
  font-weight: 700;
}

.edit-form {
  display: grid;
  gap: 4px;
}

.edit-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}

@media (max-width: 1080px) {
}

@media (max-width: 760px) {
  .summary-grid,
  .mapping-summary,
  .drawer-summary {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .section-head {
    flex-direction: column;
  }

  .filters,
  .section-actions {
    width: 100%;
  }

  .metric-switch {
    width: 100%;
    justify-content: flex-start;
  }

  .filter-select {
    width: min(100%, 220px);
  }
}

@media (max-width: 520px) {
  .summary-grid,
  .mapping-summary,
  .drawer-summary {
    grid-template-columns: 1fr;
  }

  .filter-select {
    width: 100%;
  }

  .edit-grid {
    grid-template-columns: 1fr;
  }
}
</style>

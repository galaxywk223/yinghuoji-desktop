import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { ElMessage } from "element-plus";
import { courseProfileAPI } from "@/api/modules/courseProfile";

export type CourseProfileCourse = {
  id: number;
  semester: string;
  course_name: string;
  credits: number;
  grade: number | null;
  grade_status: "graded" | "pending";
  match_status: "auto" | "manual" | "unmatched";
  matched_subcategory_id: number | null;
  matched_subcategory_name?: string | null;
  profile_id?: number | null;
  is_profile_enriched?: boolean;
  learning_hours: number;
  efficiency: number | null;
  shared_mapping_count: number;
};

type ImportPreview = {
  import_id: string;
  source_category_id: number | null;
  items: any[];
  candidates: { id: number; name: string }[];
  mismatches: Record<string, any>;
};

export const useCourseProfileStore = defineStore("courseProfile", () => {
  const loading = ref(false);
  const importing = ref(false);
  const templateDownloading = ref(false);
  const settingsLoading = ref(false);
  const sourceCategoryId = ref<number | null>(null);
  const sourceCategory = ref<any | null>(null);
  const courses = ref<CourseProfileCourse[]>([]);
  const semesters = ref<string[]>([]);
  const summary = ref<any | null>(null);
  const preview = ref<ImportPreview | null>(null);
  const semesterFilter = ref<string>("all");
  const matchStatusFilter = ref<string>("all");

  const hasCourses = computed(() => courses.value.length > 0);
  const queryParams = computed(() => ({
    semester: semesterFilter.value === "all" ? undefined : semesterFilter.value,
    match_status:
      matchStatusFilter.value === "all" ? undefined : matchStatusFilter.value,
  }));

  async function fetchSettings() {
    settingsLoading.value = true;
    try {
      const res: any = await courseProfileAPI.getSettings();
      const settings = res?.settings || {};
      sourceCategoryId.value = settings.source_category_id ?? null;
      sourceCategory.value = settings.source_category ?? null;
    } finally {
      settingsLoading.value = false;
    }
  }

  async function saveSettings(categoryId: number | null) {
    settingsLoading.value = true;
    try {
      const res: any = await courseProfileAPI.setSettings({
        source_category_id: categoryId,
      });
      const settings = res?.settings || {};
      sourceCategoryId.value = settings.source_category_id ?? categoryId;
      sourceCategory.value = settings.source_category ?? null;
      preview.value = null;
      ElMessage.success("课程来源父分类已保存");
    } finally {
      settingsLoading.value = false;
    }
  }

  async function fetchCourses() {
    loading.value = true;
    try {
      const res: any = await courseProfileAPI.getCourses(queryParams.value);
      courses.value = res?.courses || [];
      semesters.value = res?.semesters || [];
    } finally {
      loading.value = false;
    }
  }

  async function fetchSummary() {
    const res: any = await courseProfileAPI.getSummary(queryParams.value);
    summary.value = res?.summary || null;
  }

  async function refreshAll() {
    await Promise.all([fetchSettings(), fetchCourses(), fetchSummary()]);
  }

  async function previewImport(file: File) {
    importing.value = true;
    try {
      const res: any = await courseProfileAPI.previewImport(file);
      preview.value = res?.preview || null;
      if (preview.value) {
        ElMessage.success("导入预览已生成");
      }
    } finally {
      importing.value = false;
    }
  }

  async function confirmImport() {
    if (!preview.value) return;
    importing.value = true;
    try {
      await courseProfileAPI.confirmImport({
        import_id: preview.value.import_id,
        courses: preview.value.items,
      });
      preview.value = null;
      await Promise.all([fetchCourses(), fetchSummary()]);
      ElMessage.success("课程画像导入完成");
    } finally {
      importing.value = false;
    }
  }

  async function downloadTemplate() {
    if (templateDownloading.value) return null;
    templateDownloading.value = true;
    try {
      const res: any = await courseProfileAPI.downloadTemplate();
      const rowCount = Number(res?.row_count || 0);
      const suffix = res?.revealed ? "，已打开所在文件夹" : "";
      ElMessage.success(`课程模板已生成（${rowCount} 门科目）${suffix}`);
      return res;
    } finally {
      templateDownloading.value = false;
    }
  }

  async function updateCourse(courseId: number, payload: any) {
    await courseProfileAPI.updateCourse(courseId, payload);
    await Promise.all([fetchCourses(), fetchSummary()]);
    ElMessage.success("课程已更新");
  }

  async function deleteCourse(courseId: number) {
    await courseProfileAPI.deleteCourse(courseId);
    await Promise.all([fetchCourses(), fetchSummary()]);
    ElMessage.success("课程已删除");
  }

  function setSemesterFilter(value: string) {
    semesterFilter.value = value;
  }

  function setMatchStatusFilter(value: string) {
    matchStatusFilter.value = value;
  }

  return {
    loading,
    importing,
    templateDownloading,
    settingsLoading,
    sourceCategoryId,
    sourceCategory,
    courses,
    semesters,
    summary,
    preview,
    semesterFilter,
    matchStatusFilter,
    hasCourses,
    fetchSettings,
    saveSettings,
    fetchCourses,
    fetchSummary,
    refreshAll,
    previewImport,
    confirmImport,
    downloadTemplate,
    updateCourse,
    deleteCourse,
    setSemesterFilter,
    setMatchStatusFilter,
  };
});

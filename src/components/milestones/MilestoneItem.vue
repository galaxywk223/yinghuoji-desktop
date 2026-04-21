<template>
  <li class="timeline-item">
    <div class="timeline-icon"><Icon icon="lucide:trophy" /></div>
    <div class="timeline-content">
      <div class="timeline-content-header">
        <div>
          <span class="badge bg-primary timeline-category mb-1">{{
            categoryName
          }}</span>
          <h5 class="timeline-title">{{ item.title }}</h5>
          <p class="timeline-date mb-0">{{ formatDate(item.event_date) }}</p>
        </div>
        <div v-if="enableActions" class="dropdown">
          <el-dropdown trigger="click">
            <span class="el-dropdown-link btn-more" @click.stop>
              <Icon icon="lucide:more-horizontal" />
            </span>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item @click.stop="editItem">编辑</el-dropdown-item>
                <el-dropdown-item
                  divided
                  class="danger"
                  @click.stop="deleteItem"
                  >删除</el-dropdown-item
                >
              </el-dropdown-menu>
            </template>
          </el-dropdown>
        </div>
      </div>
      <p
        class="timeline-description"
        :class="{ 'text-muted': !hasDescription }"
      >
        {{ descriptionText }}
      </p>
      <div
        v-if="attachments.length"
        class="timeline-attachments border-top pt-3 mt-3"
      >
        <h6 class="card-subtitle mb-2 text-muted">附件:</h6>
        <div class="attachments-flex">
          <div
            v-for="att in attachments"
            :id="`attachment-${att.id}`"
            :key="att.id"
            class="attachment-item"
          >
            <button
              type="button"
              class="attachment-link"
              @click.stop="openAttachment(att)"
            >
              <Icon
                icon="lucide:image"
                :style="{ width: '16px', height: '16px' }"
              />
              <span>{{ att.original_filename }}</span>
            </button>
            <el-popconfirm
              v-if="enableActions"
              title="确定要永久删除这个附件吗？"
              confirm-button-text="删除"
              cancel-button-text="取消"
              @confirm="deleteAttachment(att)"
            >
              <template #reference>
                <button
                  type="button"
                  class="attachment-delete-btn"
                  title="删除此附件"
                  @click.stop
                >
                  <Icon icon="lucide:x" />
                </button>
              </template>
            </el-popconfirm>
          </div>
        </div>
      </div>
    </div>
  </li>
</template>

<script setup>
import { computed } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { Icon } from "@iconify/vue";
import { milestoneAPI } from "@/api/modules/milestone";

const props = defineProps({
  item: { type: Object, required: true },
  categories: { type: Array, default: () => [] },
  enableActions: { type: Boolean, default: true },
});
const emits = defineEmits(["edit", "deleted", "attachment-deleted"]);

const categoryName = computed(() => {
  const cat = props.categories.find((c) => c.id === props.item.category_id);
  return cat ? cat.name : "未分类";
});

const attachments = computed(() => props.item.attachments || []);
function formatDate(d) {
  if (!d) return "";
  const dt = new Date(d);
  return `${dt.getFullYear()}年${String(dt.getMonth() + 1).padStart(2, "0")}月${String(dt.getDate()).padStart(2, "0")}日`;
}
const hasDescription = computed(() => {
  const value = props.item.description;
  return typeof value === "string" && value.trim().length > 0;
});
const descriptionText = computed(() =>
  hasDescription.value ? props.item.description.trim() : "没有详细描述。",
);

function editItem() {
  emits("edit", props.item);
}

async function confirmDeletion(message, title = "确认删除") {
  try {
    await ElMessageBox.confirm(message, title, {
      type: "warning",
      confirmButtonText: "删除",
      cancelButtonText: "取消",
      confirmButtonClass: "el-button--danger",
    });
    return true;
  } catch {
    return false;
  }
}

async function deleteItem() {
  if (!(await confirmDeletion("确定要永久删除这个成就吗？"))) return;
  try {
    await milestoneAPI.remove(props.item.id);
    emits("deleted", props.item.id);
    ElMessage.success("成就已删除");
  } catch (e) {
    console.error("delete milestone failed", e);
    ElMessage.error("删除成就失败");
  }
}
async function openAttachment(att) {
  try {
    await milestoneAPI.openAttachment(att.id);
  } catch (error) {
    console.error("open attachment failed", error);
    ElMessage.error("打开附件失败");
  }
}
async function deleteAttachment(att) {
  try {
    await milestoneAPI.deleteAttachment(props.item.id, att.id);
    emits("attachment-deleted", {
      milestoneId: props.item.id,
      attachmentId: att.id,
    });
    ElMessage.success("附件已删除");
  } catch (e) {
    console.error("delete attachment failed", e);
    ElMessage.error("删除附件失败");
  }
}
</script>

<style scoped src="@/styles/components/milestones/milestone-item.scss"></style>

import { h } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { check } from "@tauri-apps/plugin-updater";
import { ElLoading, ElMessage, ElMessageBox } from "element-plus";

const STARTUP_CHECK_DELAY_MS = 1800;

let startupCheckScheduled = false;
let updateCheckInFlight = false;

function shouldCheckForUpdates() {
  return import.meta.env.PROD && isTauri();
}

function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return "";
  }

  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(0)} KB`;
  }

  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function createUpdateMessage(version: string, currentVersion: string, notes: string) {
  return h("div", { style: "white-space: pre-line; line-height: 1.7;" }, [
    h(
      "div",
      {
        style:
          "font-size: 15px; font-weight: 700; color: var(--color-text-heading, #1c1c1e); margin-bottom: 8px;",
      },
      `发现新版本 ${version}`,
    ),
    h(
      "div",
      {
        style:
          "font-size: 13px; color: var(--color-text-secondary, #6b7280); margin-bottom: 14px;",
      },
      `当前版本 ${currentVersion}`,
    ),
    h(
      "div",
      {
        style:
          "font-size: 13px; color: var(--color-text-base, #374151); white-space: pre-line;",
      },
      notes,
    ),
  ]);
}

async function runStartupUpdateCheck() {
  if (!shouldCheckForUpdates() || updateCheckInFlight) {
    return;
  }

  updateCheckInFlight = true;
  let exitPrepared = false;

  let updateHandle: Awaited<ReturnType<typeof check>> | null = null;

  try {
    updateHandle = await check({ timeout: 15000 });

    if (!updateHandle) {
      return;
    }

    const releaseNotes =
      updateHandle.body?.trim() || "该版本未提供更新说明。";

    await ElMessageBox.confirm(
      createUpdateMessage(
        updateHandle.version,
        updateHandle.currentVersion,
        releaseNotes,
      ),
      "发现新版本",
      {
        type: "info",
        confirmButtonText: "立即更新",
        cancelButtonText: "稍后",
        closeOnClickModal: false,
        closeOnPressEscape: false,
        autofocus: false,
      },
    );

    await invoke("app_prepare_exit_for_update");
    exitPrepared = true;

    let downloadedBytes = 0;
    let totalBytes = 0;
    const loading = ElLoading.service({
      lock: true,
      text: "正在下载更新…",
      background: "rgba(15, 23, 42, 0.28)",
    });

    try {
      await updateHandle.downloadAndInstall((event) => {
        if (event.event === "Started") {
          totalBytes = event.data.contentLength ?? 0;
          return;
        }

        if (event.event === "Progress") {
          downloadedBytes += event.data.chunkLength;
          if (typeof loading.setText === "function" && totalBytes > 0) {
            loading.setText(
              `正在下载更新… ${formatBytes(downloadedBytes)} / ${formatBytes(totalBytes)}`,
            );
          }
        }
      });
    } catch (error) {
      console.error("应用更新下载失败", error);
      if (exitPrepared) {
        await invoke("app_cancel_exit_for_update").catch(() => void 0);
        exitPrepared = false;
      }
      ElMessage.error("更新下载失败，请稍后重试");
    } finally {
      loading.close();
    }
  } catch (error) {
    if (error === "cancel" || error === "close") {
      return;
    }
    console.error("应用更新检查失败", error);
  } finally {
    if (updateHandle) {
      await updateHandle.close().catch(() => void 0);
    }
    updateCheckInFlight = false;
  }
}

export function scheduleStartupUpdateCheck() {
  if (!shouldCheckForUpdates() || startupCheckScheduled) {
    return;
  }

  startupCheckScheduled = true;
  window.setTimeout(() => {
    void runStartupUpdateCheck();
  }, STARTUP_CHECK_DELAY_MS);
}

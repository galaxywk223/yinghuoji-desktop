import { invoke, isTauri } from "@tauri-apps/api/core";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";

const FOCUS_NOTIFICATION_ID = 260724;

async function ensureNotificationPermission(): Promise<boolean> {
  if (!isTauri()) return false;
  if (await isPermissionGranted()) return true;
  return (await requestPermission()) === "granted";
}

export async function scheduleFocusReminder(
  deadline: Date,
  taskName: string,
  durationMinutes: number,
): Promise<boolean> {
  if (!isTauri()) return false;

  await cancelFocusReminder();
  const wakeScheduled = await invoke("focus_reminder_schedule", {
    deadlineMillis: deadline.getTime(),
  })
    .then(() => true)
    .catch((error) => {
      console.error("专注结束窗口唤醒调度失败", error);
      return false;
    });

  try {
    await ensureNotificationPermission();
  } catch (error) {
    console.error("专注结束系统通知权限检查失败", error);
  }
  return wakeScheduled;
}

export async function cancelFocusReminder(): Promise<void> {
  if (!isTauri()) return;

  await invoke("focus_reminder_cancel").catch(() => void 0);
}

export async function ensureFocusCompletionNotification(
  taskName: string,
  durationMinutes: number,
): Promise<boolean> {
  if (!isTauri()) return false;

  try {
    if (!(await ensureNotificationPermission())) return false;
    sendNotification({
      id: FOCUS_NOTIFICATION_ID,
      title: "专注倒计时结束",
      body: `${taskName || "本次专注"} · ${durationMinutes} 分钟`,
      autoCancel: true,
    });
    return true;
  } catch (error) {
    console.error("专注结束系统通知发送失败", error);
    return false;
  }
}

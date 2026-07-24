import focusCompleteSoundUrl from "@/assets/audio/focus-complete.wav";

const ALERT_VOLUME = 0.75;
let alertAudio: HTMLAudioElement | null = null;

function getAlertAudio(): HTMLAudioElement {
  if (alertAudio) return alertAudio;

  alertAudio = new Audio(focusCompleteSoundUrl);
  alertAudio.loop = true;
  alertAudio.preload = "auto";
  alertAudio.volume = ALERT_VOLUME;
  return alertAudio;
}

export async function primeFocusAlert(): Promise<void> {
  const audio = getAlertAudio();
  audio.load();

  const previousVolume = audio.volume;
  audio.volume = 0;
  try {
    await audio.play();
    audio.pause();
    audio.currentTime = 0;
  } finally {
    audio.volume = previousVolume;
  }
}

export async function startFocusAlertLoop(): Promise<void> {
  const audio = getAlertAudio();
  audio.pause();
  audio.currentTime = 0;
  audio.volume = ALERT_VOLUME;
  try {
    await audio.play();
  } catch (error) {
    console.error("专注结束提示音播放失败", error);
  }
}

export function stopFocusAlert(): void {
  if (!alertAudio) return;
  alertAudio.pause();
  alertAudio.currentTime = 0;
}

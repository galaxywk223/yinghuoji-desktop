import { computed, ref } from "vue";
import { defineStore } from "pinia";

export type ThemeFamilyId = "paper" | "ocean" | "forest" | "amber" | "midnight";
export type ThemeMode = "light" | "dark";
export type ThemeId = `${ThemeFamilyId}-${ThemeMode}`;

export type ThemeOption = {
  id: ThemeId;
  family: ThemeFamilyId;
  mode: ThemeMode;
  name: string;
  shortName: string;
  preview: string;
  description: string;
};

export type ThemeFamilyOption = {
  id: ThemeFamilyId;
  name: string;
  description: string;
  themes: ThemeOption[];
};

const STORAGE_KEY = "app-theme";

const DEFAULT_THEME: ThemeId = "paper-light";

const LEGACY_THEME_MAP: Record<string, ThemeId> = {
  light: "paper-light",
  paper: "paper-light",
  sakura: "paper-light",
  latte: "amber-light",
  ocean: "ocean-light",
  forest: "forest-light",
  amber: "amber-light",
  dark: "midnight-dark",
  midnight: "midnight-dark",
  cyberpunk: "midnight-dark",
  violet: "paper-dark",
  sunset: "amber-light",
  coffee: "amber-dark",
};

const THEME_OPTIONS: ThemeOption[] = [
  {
    id: "paper-light",
    family: "paper",
    mode: "light",
    name: "纸感晨雾 · 浅色",
    shortName: "浅色",
    preview: "linear-gradient(135deg, #ffffff 0%, #f1f2f0 52%, #7f56d9 100%)",
    description: "中性浅色工作台，适合默认使用。",
  },
  {
    id: "paper-dark",
    family: "paper",
    mode: "dark",
    name: "纸感晨雾 · 深色",
    shortName: "深色",
    preview: "linear-gradient(135deg, #171a21 0%, #2c1c5f 58%, #b692f6 100%)",
    description: "保留紫色强调的深色工作台。",
  },
  {
    id: "ocean-light",
    family: "ocean",
    mode: "light",
    name: "静海坐标 · 浅色",
    shortName: "浅色",
    preview: "linear-gradient(135deg, #ffffff 0%, #edf4f8 52%, #0e7090 100%)",
    description: "冷静青蓝，适合统计和图表场景。",
  },
  {
    id: "ocean-dark",
    family: "ocean",
    mode: "dark",
    name: "静海坐标 · 深色",
    shortName: "深色",
    preview: "linear-gradient(135deg, #0c1d28 0%, #123b4a 58%, #67e3f9 100%)",
    description: "低亮度青蓝，适合夜间复盘。",
  },
  {
    id: "forest-light",
    family: "forest",
    mode: "light",
    name: "松林草稿 · 浅色",
    shortName: "浅色",
    preview: "linear-gradient(135deg, #ffffff 0%, #edf3ea 52%, #157f3c 100%)",
    description: "自然绿色，适合长时间阅读。",
  },
  {
    id: "forest-dark",
    family: "forest",
    mode: "dark",
    name: "松林草稿 · 深色",
    shortName: "深色",
    preview: "linear-gradient(135deg, #121f1a 0%, #143c2a 58%, #75e0a7 100%)",
    description: "安静深绿，适合专注和记录。",
  },
  {
    id: "amber-light",
    family: "amber",
    mode: "light",
    name: "琥珀砂页 · 浅色",
    shortName: "浅色",
    preview: "linear-gradient(135deg, #ffffff 0%, #f4ede3 52%, #b54708 100%)",
    description: "暖砂色，适合计划与整理。",
  },
  {
    id: "amber-dark",
    family: "amber",
    mode: "dark",
    name: "琥珀砂页 · 深色",
    shortName: "深色",
    preview: "linear-gradient(135deg, #221a12 0%, #4a2e0a 58%, #fdb022 100%)",
    description: "低亮度暖色，适合夜间计划。",
  },
  {
    id: "midnight-light",
    family: "midnight",
    mode: "light",
    name: "午夜墨蓝 · 浅色",
    shortName: "浅色",
    preview: "linear-gradient(135deg, #ffffff 0%, #eef2f8 52%, #344054 100%)",
    description: "蓝灰浅色，偏专业报表界面。",
  },
  {
    id: "midnight-dark",
    family: "midnight",
    mode: "dark",
    name: "午夜墨蓝 · 深色",
    shortName: "深色",
    preview: "linear-gradient(135deg, #111827 0%, #202d53 58%, #8da4ef 100%)",
    description: "默认深色主题，适合沉浸复盘。",
  },
];

const FAMILY_META: Array<Omit<ThemeFamilyOption, "themes">> = [
  { id: "paper", name: "纸感晨雾", description: "默认中性界面" },
  { id: "ocean", name: "静海坐标", description: "青蓝数据界面" },
  { id: "forest", name: "松林草稿", description: "绿色阅读界面" },
  { id: "amber", name: "琥珀砂页", description: "暖色计划界面" },
  { id: "midnight", name: "午夜墨蓝", description: "蓝灰专业界面" },
];

function normalizeThemeId(rawTheme: string | null | undefined): ThemeId {
  if (!rawTheme) return DEFAULT_THEME;

  const matched = THEME_OPTIONS.find((item) => item.id === rawTheme);
  if (matched) return matched.id;

  return LEGACY_THEME_MAP[rawTheme] || DEFAULT_THEME;
}

function applyThemeToDocument(themeId: ThemeId) {
  const theme = THEME_OPTIONS.find((item) => item.id === themeId) || THEME_OPTIONS[0];
  document.documentElement.setAttribute("data-theme", theme.id);
  document.documentElement.classList.toggle("dark", theme.mode === "dark");
}

export const useThemeStore = defineStore("theme", () => {
  const currentTheme = ref<ThemeId>(DEFAULT_THEME);

  const themes = THEME_OPTIONS;

  const themeFamilies = computed<ThemeFamilyOption[]>(() =>
    FAMILY_META.map((family) => ({
      ...family,
      themes: themes.filter((theme) => theme.family === family.id),
    })),
  );

  const currentThemeMeta = computed(
    () => themes.find((theme) => theme.id === currentTheme.value) || themes[0],
  );

  const isDark = computed(() => currentThemeMeta.value.mode === "dark");

  function setTheme(themeId: string) {
    const normalized = normalizeThemeId(themeId);
    currentTheme.value = normalized;
    localStorage.setItem(STORAGE_KEY, normalized);
    applyThemeToDocument(normalized);
  }

  function initTheme() {
    const savedTheme = normalizeThemeId(localStorage.getItem(STORAGE_KEY));
    currentTheme.value = savedTheme;
    localStorage.setItem(STORAGE_KEY, savedTheme);
    applyThemeToDocument(savedTheme);
  }

  return {
    currentTheme,
    currentThemeMeta,
    themes,
    themeFamilies,
    isDark,
    setTheme,
    initTheme,
  };
});

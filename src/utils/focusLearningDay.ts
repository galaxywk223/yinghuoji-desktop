import dayjs, { type ConfigType } from "dayjs";

export const DEFAULT_FOCUS_DAY_BOUNDARY_HOUR = 4;
export const MIN_FOCUS_DAY_BOUNDARY_HOUR = 0;
export const MAX_FOCUS_DAY_BOUNDARY_HOUR = 12;

export function normalizeFocusDayBoundaryHour(value: unknown): number {
  const raw = typeof value === "string" && value.trim() !== "" ? Number(value) : value;
  const numberValue = typeof raw === "number" && Number.isFinite(raw) ? raw : DEFAULT_FOCUS_DAY_BOUNDARY_HOUR;
  const integerValue = Math.trunc(numberValue);

  return Math.min(
    MAX_FOCUS_DAY_BOUNDARY_HOUR,
    Math.max(MIN_FOCUS_DAY_BOUNDARY_HOUR, integerValue),
  );
}

export function getFocusLogDate(
  endedAt: ConfigType,
  boundaryHour: unknown = DEFAULT_FOCUS_DAY_BOUNDARY_HOUR,
): string {
  const endTime = dayjs(endedAt);
  const effectiveEndTime = endTime.isValid() ? endTime : dayjs();
  const normalizedBoundaryHour = normalizeFocusDayBoundaryHour(boundaryHour);

  return effectiveEndTime
    .subtract(normalizedBoundaryHour, "hour")
    .format("YYYY-MM-DD");
}

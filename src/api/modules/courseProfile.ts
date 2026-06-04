import request from "@/utils/request";

export const courseProfileAPI = {
  getSettings() {
    return request({ url: "/api/course-profile/settings", method: "get" });
  },
  setSettings(data: { source_category_id: number | null }) {
    return request({
      url: "/api/course-profile/settings",
      method: "post",
      data,
    });
  },
  previewImport(file: File) {
    const formData = new FormData();
    formData.append("file", file);
    return request({
      url: "/api/course-profile/import/preview",
      method: "post",
      data: formData,
    });
  },
  confirmImport(data: any) {
    return request({
      url: "/api/course-profile/import/confirm",
      method: "post",
      data,
    });
  },
  getCourses(params: Record<string, any> = {}) {
    return request({ url: "/api/course-profile/courses", method: "get", params });
  },
  updateCourse(courseId: number, data: any) {
    return request({
      url: `/api/course-profile/courses/${courseId}`,
      method: "put",
      data,
    });
  },
  deleteCourse(courseId: number) {
    return request({
      url: `/api/course-profile/courses/${courseId}`,
      method: "delete",
    });
  },
  getSummary(params: Record<string, any> = {}) {
    return request({ url: "/api/course-profile/summary", method: "get", params });
  },
  downloadTemplate() {
    return request({ url: "/api/course-profile/template", method: "post" });
  },
};

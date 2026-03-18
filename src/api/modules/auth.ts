/**
 * 认证相关API
 */
import request from "@/utils/request";

export const authAPI = {
  register(data) {
    return request({
      url: "/api/auth/register",
      method: "post",
      data,
    });
  },

  // 用户登录
  login(data) {
    return request({
      url: "/api/auth/login",
      method: "post",
      data,
    });
  },

  refreshToken() {
    return request({
      url: "/api/auth/refresh",
      method: "post",
    });
  },

  getCurrentUser() {
    return request({
      url: "/api/auth/me",
      method: "get",
    });
  },

  changePassword(data) {
    return request({
      url: "/api/auth/change-password",
      method: "post",
      data,
    });
  },
};

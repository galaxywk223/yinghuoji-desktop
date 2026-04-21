# 萤火集

萤火集的独立仓库，定位为单用户、本地优先的桌面版本。

## 仓库关系

- 原始 Web 仓库：[`learning-analytics-system`](https://github.com/galaxywk223/learning-analytics-system)
- 当前桌面仓库：[`yinghuoji-desktop`](https://github.com/galaxywk223/yinghuoji-desktop)

这个仓库以原 Web 仓库为功能与产品参考，但和原仓库是两个完全独立的 Git 仓库：

- 不共享运行时代码
- 不使用 submodule
- 不与原仓库做共仓演进

## 这个仓库负责什么

桌面端延续了原萤火集的核心产品方法：

- 仪表盘
- 学习记录
- 阶段管理
- 分类管理
- 统计分析
- 专注模式
- 倒计时
- 成就时刻
- 数据导入导出
- AI 助手与 AI 历史

同时桌面端明确改成了本地模式：

- 移除注册、登录、JWT、刷新令牌
- 移除社区排行
- 账户设置改为本地档案与应用设置
- 数据默认保存在本地 SQLite
- 附件保存在应用数据目录
- AI 为可选在线能力，API Key 保存到系统安全存储

## 技术栈

- Vue 3 + TypeScript + Pinia + Element Plus
- Tauri 2
- Rust
- SQLite

## 开发

```bash
npm install
npm run desktop:dev
```

## 构建

```bash
npm run desktop:build
```

## 补充说明

项目完整背景、产品来历和 Web 版本整体说明可参考：

- [`learning-analytics-system` README](https://github.com/galaxywk223/learning-analytics-system)

## 开源协议

桌面端仓库沿用 Web 仓库的协议，使用 [GNU Affero General Public License v3.0 only](./LICENSE) 发布。

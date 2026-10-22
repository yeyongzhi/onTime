# onTime

<img src="public/brand/app-icon.svg" width="96" alt="onTime 应用图标" />

**轻量桌面提醒工具**

要紧的事，到点提醒。

onTime 是一款轻量、本地优先的桌面提醒应用。定点提醒、倒计时、重复日程和自选铃声，帮助你记住会议、休息、服药与生活中的小事。

[产品介绍](https://yeyongzhi.github.io/onTime/) · [下载安装包](https://github.com/yeyongzhi/onTime/releases) · [反馈问题](https://github.com/yeyongzhi/onTime/issues)

当前代码版本：**v0.0.1-beta5（预览版）**。发布状态与可用安装包请以对应 Release 页面为准。

[查看 beta5 预发布](https://github.com/yeyongzhi/onTime/releases/tag/v0.0.1-beta5) · [查看全部发布](https://github.com/yeyongzhi/onTime/releases) · [更新日志](docs/changelog.md)

## 你可以用它做什么

- **安排时间**：定点或倒计时；一次性或每天、每周、每月、每季度、每年重复，并可设定提醒生效日期；重复提醒支持多个时间或每半小时、1 小时、2 小时间隔。
- **选择提醒方式**：独立提醒窗口、系统通知、内置轻铃或导入音频；音量渐强、自动停止、5 / 10 分钟贪睡与次数上限。
- **管理多个提醒**：分组、彩色标签、搜索与筛选，网格 / 列表切换，批量启用、停用和删除。
- **回看提醒记录**：本地记录提醒处理结果，支持按名称、状态、类型、分组、标签和日期筛选并分页，保留触发时分类。
- **按喜好调整外观**：亮色、暗色、跟随系统，统一品牌紫色，支持圆角、字号、侧栏宽度和折叠。
- **保管自己的数据**：提醒、分组、标签和历史保存在本地 SQLite；支持每日自动备份、ZIP 导出和恢复、数据迁移、历史保留策略和未使用铃声清理，无需注册账号。

## 开始使用

1. 从 [Releases](https://github.com/yeyongzhi/onTime/releases) 下载与你的系统匹配的安装包。实际可用版本和平台以发布文件为准。
2. 首次启动自动初始化本地数据；Windows 默认保存在安装目录，已有自选目录继续沿用。
3. 在“全部提醒”点击“新建提醒”，填写名称、时间和铃声，启用后保存。

完整说明：[安装与首次启动](https://yeyongzhi.github.io/onTime/guide.html#下载与安装) · [使用指南](https://yeyongzhi.github.io/onTime/guide.html)。

支持 Windows / macOS 安装包。Windows 安装包未签名，首次运行可能出现未知发布者提示；macOS 安装包未完成 Developer ID 签名和公证。

**提醒需要应用运行。** 关闭主窗口会收起到托盘；从托盘退出会停止提醒。关机、睡眠及退出期间不会响铃，也不会主动唤醒电脑。可在设置中开启登录自启动。

## 数据与备份

Windows 新安装默认将数据保存在程序安装目录，macOS 默认使用系统用户数据目录；已有自选目录继续沿用，也可在设置中切换或迁移。外观偏好保存为 `setting/appearance.json`。系统应用数据目录只保存数据目录定位信息。不要把数据目录放在受保护位置或 `.app` 包内部。

在“设置 → 数据”可以分别选择“切换数据目录”与“迁移数据”。切换读取目标目录；迁移将当前业务数据和设置复制到空目录，成功后切换，原目录保留。迁移前需处理响铃和贪睡中的提醒。

历史默认永久保留，可选择保留 30、90、180 或 365 天；每日备份成功后清理已结束的旧记录。手动清理会先备份业务数据及未使用铃声，再按策略清理；恢复备份会取消其中的响铃与贪睡状态，避免旧提醒再次响起。

重复日程按本地日期与普通星期规则计算。应用优先从官网静态清单检测稳定版和预览版，自动检查最多每 6 小时一次，并保留上次成功结果；官网不可用时有限度地尝试 GitHub API。下载安装包后校验 SHA-256，关于页也可直接前往发布页。beta3 / beta4 遇到 GitHub 限流时，请先手动下载安装 beta5 以启用新机制；beta1 / beta2 同样建议手动升级。

## 开发与自动构建

应用使用 Tauri 2、Rust、Vue 3、TypeScript、Pinia 和 shadcn-vue。开发需要 Node.js 24、pnpm 12.9.1、Rust stable 及 [Tauri 平台依赖](https://v2.tauri.app/start/prerequisites/)。

```sh
pnpm install --frozen-lockfile
pnpm tauri:dev
```

验证与构建：

```sh
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml --lib --locked
pnpm tauri:build --bundles nsis
pnpm docs:dev
pnpm docs:build
```

`pnpm dev` 只预览界面，不执行后台提醒。产品站使用 VitePress，源文件位于 `docs/`。

GitHub Actions 在 `dev` / `main` 推送时构建 Windows x64、macOS Apple Silicon 和 Intel 安装包；推送 `v*` 标签会构建并将安装包发布到 GitHub Release。产品站仅在推送 `main` 分支时构建并部署 GitHub Pages。发布新版本时先集成并推送 `main`，再推送版本标签；站点构建会最多等待 20 分钟，确认当前版本的三个安装包已发布且具备 SHA-256 后生成 `updates.json`，避免提前公布未上传完整的版本。仓库 Pages 的 Source 需设为 **GitHub Actions**。

产品站网址：<https://yeyongzhi.github.io/onTime/>。部署状态以对应提交的 Actions 结果为准。

详细开发资料：[项目说明书](项目说明书.md) · [技术开发说明书](技术开发说明书.md) · [开发进度](开发进度.md)

## 技术栈

| 技术 | 用途 |
| --- | --- |
| Vue 3、TypeScript、Pinia | 界面与应用状态 |
| shadcn-vue、Tailwind CSS | 交互组件与样式 |
| Tauri 2、Rust | 桌面运行时与后台提醒 |
| SQLite | 本地提醒及历史存储 |
| Vite、VitePress | 应用构建与产品文档站 |

## 联系作者

[GitHub 主页](https://github.com/yeyongzhi) · [问题反馈](https://github.com/yeyongzhi/onTime/issues)

# onTime

**重要时刻，准时提醒。**

onTime 是一款轻量、本地优先的桌面提醒应用。定点提醒、倒计时、重复日程和自选铃声，帮助你记住会议、休息、服药与生活中的小事。

[产品介绍](https://yeyongzhi.github.io/onTime/) · [下载安装包](https://github.com/yeyongzhi/onTime/releases) · [反馈问题](https://github.com/yeyongzhi/onTime/issues)

## 你可以用它做什么

- **安排时间**：定点或倒计时；一次性或每天、每周、每月、每季度、每年重复，并可设定提醒生效日期。
- **选择提醒方式**：独立提醒窗口、系统通知、内置轻铃或导入音频；音量渐强、自动停止、5 / 10 分钟贪睡与次数上限。
- **管理多个提醒**：分组、彩色标签、搜索与筛选，网格 / 列表切换，批量启用、停用和删除。
- **回看提醒记录**：本地记录提醒处理结果，支持按名称、状态和时间筛选并分页。
- **按喜好调整外观**：亮色、暗色、跟随系统，主题色与自定义颜色、圆角、字号、侧栏宽度和折叠。
- **保管自己的数据**：提醒、分组、标签和历史保存在本地 SQLite；支持每日自动备份、ZIP 导出和恢复，无需注册账号。

## 开始使用

1. 从 [Releases](https://github.com/yeyongzhi/onTime/releases) 下载与你的系统匹配的安装包。实际可用版本和平台以发布文件为准。
2. 首次启动时选择一个可写、长期可用的本地数据目录。
3. 点击“新建提醒”，填写名称、时间和铃声，启用后保存。

支持 Windows / macOS 安装包。Windows 安装包未签名，首次运行可能出现未知发布者提示；macOS 安装包未完成 Developer ID 签名和公证。

**提醒需要应用运行。** 关闭主窗口会收起到托盘；从托盘退出会停止提醒。关机、睡眠及退出期间不会响铃，也不会主动唤醒电脑。可在设置中开启登录自启动。

## 数据与备份

数据保存在用户选择的本地目录中；外观偏好保存为 `setting/appearance.json`。系统应用数据目录只保存数据目录定位信息。不要把数据目录放在受保护位置或 `.app` 包内部。

在“设置 → 数据”切换目录时，会打开目标目录的数据，不会自动迁移。迁移前先导出备份，切换后再恢复。恢复会覆盖提醒、标签与记录并取消当前活动提醒，应用会保留恢复前备份。升级前请退出应用并保留数据目录。

节假日日历来自 [holiday-cn](https://github.com/NateScarlet/holiday-cn)，内置 2025、2026 年日历；缺失当年日历时按普通星期规则计算，可在设置中更新。项目保留其 MIT 授权文件。

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

GitHub Actions 在 `dev` / `main` 推送时构建 Windows x64、macOS Apple Silicon 和 Intel 安装包；推送 `v*` 标签会构建并将安装包发布到 GitHub Release。产品站仅在 `main` 分支相关文件变更时构建并部署 GitHub Pages。仓库 Pages 的 Source 需设为 **GitHub Actions**。

产品站网址：<https://yeyongzhi.github.io/onTime/>。首次部署是否成功以 Actions 结果为准。

详细开发资料：[项目说明书](项目说明书.md) · [技术开发说明书](技术开发说明书.md) · [开发进度](开发进度.md)

# 爱雅思写作开发指南

2026-10-25 更新

## 运行

当前以 macOS 为桌面验收目标，需要 Node.js 22.12+、Rust stable 和 Xcode Command Line Tools。当前 Vite 依赖也接受 Node.js 20.19+。

在仓库根目录执行：

```bash
npm ci
npm run tauri dev
```

`npm run dev` 只启动浏览器交互原型，其 `localStorage` 不等于桌面数据；默认原型没有完整模型、图片导入和 Word 导出能力。

日常构建并打开当前源码：

```bash
npm run desktop
```

如果应用正在运行，先从应用内正常退出，让草稿保存完成。入口会重新构建前端和 `.app`；单独 `npm run build` 不会更新旧桌面包。当前构建名为“爱雅思写作 aiELTS Writing.app”，应用标识保持 `com.local.ieltswriting`。`npm run desktop` 会更新“应用程序”目录的同名安装并启动；只想构建时使用 `npm run tauri -- build --bundles app`。不要为更名搬动日常数据目录。

## 模块分工

| 位置 | 职责 |
| --- | --- |
| `src/App.tsx`、`src/reviewWorkflow.ts` | 写作、批阅、修改页面状态；保存完成后选择或固定明确的送审版本 |
| `src/sessionSave.ts` | 保存队列、修订号与切换/退出时的保存协调 |
| `src/AnnotatedReview.tsx`、`src/reviewText.ts` | 固定原文与批注卡片联动、删改对照、保留笔迹 |
| `src/V2.tsx` | 批阅任务、聊天、问题记录、多篇分析与记录导出入口 |
| `src/ModelSettings.tsx` | 模型连接、提示词管理、配置管理、协议和上下文能力测试 |
| `src-tauri/` | 桌面窗口、文件、资源和正常 macOS 钥匙串适配 |
| `crates/writing-core/` | 不依赖 Tauri 的版本、题库、归档、Word 生成与 AI 核心 |
| `crates/writing-core/src/review.rs` | 结构化批注解析、证据校验与定位 |
| `crates/writing-core/src/conversation.rs` 等 | 对话隔离、上下文接续与待处理回复恢复 |
| `crates/writing-core/prompts/` | 通用、Academic Task 1 与批注输出协议等程序内置要求 |
| `contracts/`、`src/generated/dto.ts` | 数据契约与生成类型；不要直接手改生成结果 |

批阅只绑定送审时的固定稿；编辑中的修改稿不能被旧批注覆盖。页面来回切换不应增加模型请求或修改版本。新单篇批阅输出结构化批注，旧文字报告继续兼容，多篇分析沿用既有工作流。

普通自定义 Prompt 是补充要求。调整程序内置批阅或结构化协议时，请同时检查解析、定位、预算和旧数据兼容；修改辅助 Prompt 应说明是否要求整篇重写，保留用户对修改范围的决定权。

## 验证

根据改动运行相关检查；完整回归可以执行：

```bash
npm run build
npm test
cargo test -p writing-core
cargo test --manifest-path src-tauri/Cargo.toml
```

更改契约后按项目生成脚本的用法重新生成并检查一致性。核心文件流程可用不存在的新隔离目录运行：

```bash
cargo run -p writing-core --example smoke -- /tmp/ielts-writing-smoke
```

涉及保存、固定、切换、退出、迁移时验证失败恢复；涉及批注时验证来源固定、重叠批注与无法定位的处理；涉及上下文时区分未确认结果与已收到但未保存的回复。

本机 ad hoc 签名不等于经过签名、公证的公开发行包。公开安装包、可用平台和模型兼容范围以实际发布验证为准。

## v2 发行包

Git 标签为 `v2`，应用版本为 `2.0.0`。当前发布 Apple Silicon 的优化构建，采用 ad hoc 临时签名，未经 Apple 公证。下载与安装条件见[安装说明](INSTALL.md)，资源清单见[下载目录](downloads/README.md)。

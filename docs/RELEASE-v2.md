# 爱雅思写作 · aiELTS Writing v2

先独立写作，再把每次反馈用到修改里。

这是项目首次在 GitHub 公开发布。Git 标签为 **`v2`**，应用版本为 **`2.0.0`**。

## 下载应用

**新增完整题库版（2026-10-26）：应用＋156 道题，一个安装包下载齐，首次确认数据目录后自动入库。**

- **[推荐：完整版 · DMG，约 86 MB](https://github.com/tomot-ch/aielts-writing/releases/download/v2/aiELTS-Writing-v2.0.0-macos-arm64-full.dmg)**
- [完整版 · ZIP](https://github.com/tomot-ch/aielts-writing/releases/download/v2/aiELTS-Writing-v2.0.0-macos-arm64-full.zip)：与 DMG 二选一。
- [轻量版 · DMG](https://github.com/tomot-ch/aielts-writing/releases/download/v2/aiELTS-Writing-v2.0.0-macos-arm64.dmg) · [轻量版 · ZIP](https://github.com/tomot-ch/aielts-writing/releases/download/v2/aiELTS-Writing-v2.0.0-macos-arm64.zip)：继续保留，附少量示例题，可自行扩充题库。
- [安装与首次打开说明](https://github.com/tomot-ch/aielts-writing/blob/main/docs/INSTALL.md) · [使用指南](https://github.com/tomot-ch/aielts-writing/blob/main/docs/USAGE.md)

当前提供 M 系列 Mac 安装包，采用 **ad hoc 临时签名，未经 Apple 公证**；首次打开可能被 macOS 拦截，请先阅读安装说明。最低部署目标为 macOS 11，本次构建与测试环境为 macOS 26.2，旧版系统尚未逐一验证。AI 需要自配接口和 API Key，费用由服务商收取。

## 练习流程

- **写作**：题目与文章并排，可选计时、实时字数和自动保存。
- **批阅**：固定送审稿，原句与批注卡片双向定位，对照局部改句与解释。
- **修改**：独立修改稿与 AI 聊天并排，支持四色笔迹、固定版本及明确发起重新批阅。
- **问题记录入库**：开启自动记录后，保存单篇问题归纳及来源，可编辑、移除并留作下次复习。
- **复盘与归档**：多篇分析、本地存储，以及 Word / Markdown 导出。
- **自选模型与 Prompt**：模型连接、提示词与配置分开管理，支持可选上下文接续。

## 下载题库与前置 Prompt

- **[学术类合集：剑雅 4–19，共 128 题](https://github.com/tomot-ch/aielts-writing/releases/download/v2/ielts-writing-questions.zip)**
- **[培训类补充包：28 题](https://github.com/tomot-ch/aielts-writing/releases/download/v2/ielts-writing-general-training.zip)**
- **[v2 Prompt 包](https://github.com/tomot-ch/aielts-writing/releases/download/v2/ielts-writing-prompts.zip)**：默认补充 Prompt、通用前置规则、Academic Task 1 专项和结构化批注协议。
- [全部下载与学术类单册入口](https://github.com/tomot-ch/aielts-writing/blob/main/docs/downloads/README.md)
- [156 题目录](https://github.com/tomot-ch/aielts-writing/releases/download/v2/question-catalog.json) · [附件 SHA-256 清单](https://github.com/tomot-ch/aielts-writing/releases/download/v2/SHA256SUMS.txt)

完整版自动入库，无需再下载单独题包。轻量版用户或需要补充题目时，解压题包后，在应用“准备&其他 → 下载或自行上传题目”批量导入 `images/` 内的 PNG。当前不能直接导入 ZIP / PDF，应用内自动下载尚未接入。学术类合集与单册包包含重复题目，按需要选择下载；本次整理范围为 156 题。

Prompt 包的教程已按 v2 更新。内置前置规则由程序自动加入，无需重复粘贴；普通使用者可在“提示词管理”添加补充要求，再于“配置管理”搭配模型启用。

## 欢迎参与

欢迎实际练一篇、提出修改或完善功能的 [Issue](https://github.com/tomot-ch/aielts-writing/issues/new/choose)，也欢迎帮助完善功能、界面、教程与题库。

**特别欢迎大家提供优秀的作文评阅或辅助修改 Prompt。** 可以分享完整内容、用途、测试模型与可选的匿名效果示例。具体方式见[贡献指南](https://github.com/tomot-ch/aielts-writing/blob/main/CONTRIBUTING.md)。

## 验证与当前边界

前端生产构建、优化后的 Tauri 应用构建通过；前端 12 项、核心 59 项、桌面 4 项测试通过。另有 2 项真实钥匙串集成测试保持忽略。题包通过 ZIP 完整性检查，156 张图片逐一与目录校验值核对。完整版另经自动入库、重复导入、保留自导入题目和既有作文的隔离验证；安装包签名、DMG 及 ZIP 内容检查通过。

反馈和教学预估仅供练习参考，不是官方评分。源码许可证尚未选定；题目版权归原权利人所有，来源保留在目录中，本项目与 IELTS 官方及 Cambridge 无隶属关系。Windows、Intel Mac 暂无经过验证的发行包。

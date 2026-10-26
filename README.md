<div align="center">

# 爱雅思写作 · aiELTS Writing

**看清哪里需要改，边改边问 AI。**

雅思写作练习 · 原句关联批注 · 修改与追问 · macOS

[快速了解与开始](#快速了解与开始) · [下载完整版](https://github.com/tomot-ch/aielts-writing/releases/download/v2/aiELTS-Writing-v2.0.0-macos-arm64-full.dmg) · [界面与功能](#界面与功能) · [题库与 Prompt](docs/downloads/README.md) · [使用指南](docs/USAGE.md) · [参与共建](CONTRIBUTING.md)

</div>

爱雅思写作是一个面向雅思备考者的 Mac 写作工具。先自己完成作文，写完对照原句看 AI 批注，再一边修改、一边追问。初稿、修改稿和反馈都保留下来，方便回看。

**[一键下载完整版：应用＋156 道题](https://github.com/tomot-ch/aielts-writing/releases/download/v2/aiELTS-Writing-v2.0.0-macos-arm64-full.dmg)** · 首次选择保存位置后自动入库，直接选题开始练习。

## 快速了解与开始

### 它能怎样帮你练作文？

- **看清哪句需要改。** 点击一条批注，回到对应原句，对照改句与中文解释。
- **不明白，就接着问。** 为什么这样改？例子够不够有说服力？在修改页边写边和 AI 讨论。
- **原稿和修改稿，都能回看。** 保留初稿和修改版本，练过几篇后，还能归纳自己反复出现的问题。

**一次练习：写作 → 批阅 → 修改 → 问题记录入库。**

### 第一次使用，跟着这三步

1. **下载完整版，直接选题写。** [下载安装包](https://github.com/tomot-ch/aielts-writing/releases/download/v2/aiELTS-Writing-v2.0.0-macos-arm64-full.dmg)，把应用拖入“应用程序”。首次打开确认作文保存位置，156 道题会自动入库，无需另外下载、解压或导入题包；也可以点击“直接开始写作”。
2. **连好 AI，先用内置提示词。** 在“准备&其他 → 语言大模型配置”填写模型服务的接口、API Key 和模型名，完成连接测试；再到“配置管理”选好模型与内置提示词，保存并启用。具体操作见[配置教程](docs/USAGE.md#6-分类配置模型与-prompt)。
3. **写完点“批阅”，看完点“修改”。** 点击批注看原句，再到修改页动手改、继续问。修改自动保存；想留下一版时点“保存修改版本”，想看新反馈时点“重新批阅当前稿”。

不配置 AI，也可以先选题、写作、计时和自动保存。当前安装包适用于 **M 系列 Mac**，未经 Apple 公证，首次打开请参照[安装说明](docs/INSTALL.md)。AI 需要自配 API Key，调用费用按服务商规则计算。

### AI 和提示词，简单理解就够了

**大模型（LLM）负责读文章、给反馈；提示词（Prompt）告诉它重点看什么、怎样解释。** 应用会把相关题目、作文和要求交给你选的模型，再把反馈显示成批注。

程序已经配好基础评阅要求。连好模型后，可以直接使用内置提示词；想调整反馈方式，再添加自己的要求，例如：

> 重点检查我的论证是否充分，先用中文解释原因，再给两种局部改法。

这些补充要求在“提示词管理”中添加，再与模型搭配启用。内置规则会自动加入，具体设置见[使用指南](docs/USAGE.md#6-分类配置模型与-prompt)。

## 界面与功能

*以下为界面操作示例：使用当前应用组件、自拟题目和作文，批注与聊天为合成示例，不代表真实模型评分。*

### 写作：把注意力放在自己的表达上

题目与正文并排，实时显示字数、自动保存，可按需要开启 20 / 40 分钟计时。写作区关闭可控的拼写检查、自动更正与续写提示，正常输入法候选保留。

![写作：看着题目独立完成作文，显示字数与批阅入口](docs/images/01-writing.jpg)

### 批阅：建议对应哪一句，一眼看清

原文与批注并排，点击卡片可以定位原句。对照局部改句和中文解释，看清问题、原因与修改方向；无法可靠定位的内容会说明情况。

![批阅：点击右侧批注，在左侧找到对应原句](docs/images/02-annotated-review.jpg)

### 修改：自己动手，也能继续追问

左侧改文章，右侧问 AI。可以讨论语法、表达和论证，再决定怎样修改；支持黑、蓝、绿、红四种笔迹，方便标出自己改过的地方。

![修改：左侧编辑作文，右侧围绕反馈继续提问](docs/images/03-revision-chat.jpg)

初稿、批阅和修改版本分别保留。查看旧批阅不会重复请求模型；需要新反馈时，再点击“重新批阅当前稿”。

### 问题记录入库：把这次的问题，留给下次复习

开启“自动记录单篇分析中的问题”后，单篇批阅中的问题归纳会保存到“问题记录”。每条记录保留对应的作文、版本和批阅来源，可以自行编辑或移除，方便下次写作前回顾和自查。

## 练过的作文，继续用得上

- **找到反复出现的问题**：选择几篇作文的版本，一起复盘，整理下一次值得自查的地方。
- **留住练习记录**：稿件、版本、批阅和聊天保存在本地，题目图片也能重复使用。
- **导出继续复习**：稿件可导出为保留笔迹颜色的 Word，批阅和聊天支持 Markdown 或 Word。
- **按自己的习惯调整**：自选模型和 Prompt，也可以切换模型与提示词的搭配。

更多操作、模型设置和失败恢复见[使用指南](docs/USAGE.md)。

## 下载应用、题库与 Prompt

| 你需要什么 | 下载入口 |
| --- | --- |
| **推荐：完整版 v2（应用＋156 题）** | **[一键下载 DMG，约 86 MB](https://github.com/tomot-ch/aielts-writing/releases/download/v2/aiELTS-Writing-v2.0.0-macos-arm64-full.dmg)** · [ZIP](https://github.com/tomot-ch/aielts-writing/releases/download/v2/aiELTS-Writing-v2.0.0-macos-arm64-full.zip) |
| 轻量版（附少量示例题） | [DMG](https://github.com/tomot-ch/aielts-writing/releases/download/v2/aiELTS-Writing-v2.0.0-macos-arm64.dmg) · [ZIP](https://github.com/tomot-ch/aielts-writing/releases/download/v2/aiELTS-Writing-v2.0.0-macos-arm64.zip)，可自行扩充题库 |
| 学术类题库：剑雅 4–19，128 题 | [下载合集](https://github.com/tomot-ch/aielts-writing/releases/download/v2/ielts-writing-questions.zip) · [按册下载](docs/downloads/README.md) |
| 培训类题库：28 题 | [下载补充包](https://github.com/tomot-ch/aielts-writing/releases/download/v2/ielts-writing-general-training.zip) |
| 前置规则与补充 Prompt | [下载 Prompt 包](https://github.com/tomot-ch/aielts-writing/releases/download/v2/ielts-writing-prompts.zip) |

完整版已包含学术类 128 题、培训类 28 题，共 **156 题**，首次设置数据目录后自动入库。使用轻量版或想补充题目时，再按需下载单独题包，解压后在“准备&其他 → 下载或自行上传题目”导入 PNG；当前不能直接导入 ZIP / PDF。内置前置规则由程序自动加入，无需重复粘贴。

安装版不需要 Node.js 或 Rust。首次打开、平台条件与文件校验见[安装说明](docs/INSTALL.md)，源码运行见[开发指南](docs/DEVELOPMENT.md)。

## 欢迎一起完善

欢迎实际练一篇，把不顺手的地方告诉我们：提出修改或完善功能的 Issue，帮助改进界面和教程，也欢迎贡献代码与自拟或可公开分发的题目。

**特别欢迎大家提供优秀的作文评阅或辅助修改 Prompt。** 可以分享完整内容、用途、测试模型，以及可选的匿名效果示例。不会写代码，同样能帮助项目改进。

[反馈问题、建议功能或分享 Prompt](https://github.com/tomot-ch/aielts-writing/issues/new/choose) · [贡献指南](CONTRIBUTING.md)

如果它对你的练习有帮助，欢迎 Star，或分享给一起备考的朋友。

## 常见问题

**需要自己写复杂的提示词吗？**

可以先用内置提示词。程序会自动加入四项评价、原文证据与批注要求，适用时加入 Academic Task 1 专项。自定义 Prompt 用来补充你更关注的地方。默认提供分析和局部建议，完整改写需明确提出要求。

**作文会发到哪里？**

作文、版本、批阅和聊天保存在你选择的本地目录。调用 AI 时，相关作文、题目、提示词和对话上下文会发送到你配置的模型服务；模型服务的处理和存储受其规则约束。API Key 使用 macOS 钥匙串保存。

**AI 给出的分数可信吗？**

批阅、建议与教学预估仅供练习参考，可能出现误判，需要结合原文判断。本项目与 IELTS 官方无隶属关系，不提供官方评分或考试成绩承诺。

**支持其他平台吗？**

当前提供经过验证的 Apple Silicon（M 系列）Mac 安装包。Intel Mac、Windows 和 Linux 暂无经过验证的安装包；浏览器入口用于交互原型，不提供完整桌面功能。

## 项目状态与许可

项目持续完善中，欢迎试用和反馈。源码已公开，代码许可证尚未选定，当前未授予额外的开源使用许可。题目版权归原权利人所有，来源见[题目目录](docs/downloads/question-catalog.json)；本项目与 IELTS 官方及 Cambridge 无隶属关系。

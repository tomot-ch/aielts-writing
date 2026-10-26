# 安装爱雅思写作 v2

版本：**2.0.0**，Git 标签：`v2`。

## 下载与安装

当前安装包适用于 **Apple Silicon（M 系列）Mac**。最低部署目标为 macOS 11，本次构建与测试环境为 macOS 26.2，尚未逐一验证旧版系统。Intel Mac、Windows 和 Linux 暂无经过验证的安装包。

1. 推荐[下载完整版 DMG](https://github.com/tomot-ch/aielts-writing/releases/download/v2/aiELTS-Writing-v2.0.0-macos-arm64-full.dmg)，已包含应用和全部 156 道题。
2. 打开 DMG，将“爱雅思写作 aiELTS Writing”拖入“应用程序”。也可选择[完整版 ZIP](https://github.com/tomot-ch/aielts-writing/releases/download/v2/aiELTS-Writing-v2.0.0-macos-arm64-full.zip)，解压后拖入“应用程序”，二者选一种即可。
3. 从“应用程序”打开应用，确认长期保存作文和题库的目录。
4. 题目会自动入库，完成后直接从题库选题写作。无需再下载题包或手动导入图片。

原有[轻量版和单独题包](https://github.com/tomot-ch/aielts-writing/releases/tag/v2)继续保留，可按需要选择。轻量版只带少量示例题，完整题库需另行下载并导入。

安装版不需要 Node.js、Rust 或额外的本地模型。AI 功能需要你自行配置模型接口与 API Key；可以先不配置 AI，直接写作并自动保存。

## 首次打开

此版采用 **ad hoc 临时签名，未经 Apple 公证**，并非 Developer ID 签名发行版。macOS 可能阻止首次打开。

如果你已核对下载来源并决定运行，先尝试打开一次，再按照 Apple 的[官方说明](https://support.apple.com/zh-cn/102445)，在“系统设置 → 隐私与安全性”中为这一个应用选择“仍要打开”，完成确认。界面名称可能随系统版本略有不同。

## 导入题目与设置 Prompt

- **完整版**：学术类 128 题、培训类 28 题自动入库，题名标明类别；相同图片不重复导入，用户自行导入的题目与已有作文保留。
- **轻量版或补充题目**：在[下载目录](downloads/README.md)选择题包，解压后到应用“准备&其他 → 下载或自行上传题目 → 选择图片导入”，批量选择 `images/` 中的 PNG，再确认 Task 类型。
- 当前不能直接导入 ZIP 或 PDF，应用内的在线自动下载尚未接入。
- Prompt 包提供默认补充要求与当前内置规则。普通使用者在“语言大模型配置 → 提示词管理”填写补充 Prompt，再到“配置管理”搭配模型并启用；内置规则由程序自动加入。
- 更详细的写作、批阅和修改流程见[使用指南](USAGE.md)。

## 更新与数据

更新前从应用内正常退出，确认草稿保存完成，再替换“应用程序”中的同名应用。应用标识保持 `com.local.ieltswriting`；安装包不包含你的作文、聊天或 API Key。

作文与记录保存在你选定的数据目录，API Key 使用 macOS 钥匙串。需要迁移数据时使用应用内的目录迁移功能。建议先备份自己的数据目录再升级。

## 校验下载文件

Release 同时提供 `SHA256SUMS.txt`。把需要检查的文件与清单放在同一目录，在终端执行，例如：

```bash
shasum -a 256 aiELTS-Writing-v2.0.0-macos-arm64-full.dmg
```

将结果与清单中同名文件的 SHA-256 比较。校验可以确认文件内容是否与本次发布一致，不能替代 Apple 公证或独立的安全审查。

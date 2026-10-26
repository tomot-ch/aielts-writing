# v2 下载目录

应用版本：**2.0.0**（标签 `v2`）。题库整理版本保留原编号 `0.1.0-prep.2`，日期 2026-10-25；它是资源整理编号，不代表应用版本。

## 应用安装包

**推荐完整版：一个安装包包含应用和 156 道题。首次打开确认数据目录后自动入库，直接选题开始练习。**

| 文件 | 用途 |
| --- | --- |
| **[完整版 · DMG，约 86 MB](https://github.com/tomot-ch/aielts-writing/releases/download/v2/aiELTS-Writing-v2.0.0-macos-arm64-full.dmg)** | **应用＋156 题，自动入库，推荐下载** |
| [完整版 · ZIP](https://github.com/tomot-ch/aielts-writing/releases/download/v2/aiELTS-Writing-v2.0.0-macos-arm64-full.zip) | 同一份完整应用，与完整版 DMG 二选一 |
| [轻量版 · DMG](https://github.com/tomot-ch/aielts-writing/releases/download/v2/aiELTS-Writing-v2.0.0-macos-arm64.dmg) | 附少量示例题，可自行补充题库 |
| [轻量版 · ZIP](https://github.com/tomot-ch/aielts-writing/releases/download/v2/aiELTS-Writing-v2.0.0-macos-arm64.zip) | 与轻量版 DMG 二选一 |

安装包采用 ad hoc 临时签名，未经 Apple 公证。首次打开和使用条件见[安装说明](../INSTALL.md)。

## 题库

完整版已经包含以下题目，无需再下载单独题包。轻量版用户或仅需要题目图片时，可按下列入口下载。

本次包含 **156 题**：剑雅 4–19 学术类 128 题、培训类 28 题。这是本次整理的范围，不代表全部雅思题目。

| 下载 | 内容 |
| --- | --- |
| [学术类合集](https://github.com/tomot-ch/aielts-writing/releases/download/v2/ielts-writing-questions.zip) | 剑雅 4–19，128 题 |
| [培训类补充包](https://github.com/tomot-ch/aielts-writing/releases/download/v2/ielts-writing-general-training.zip) | 28 题，与学术类分开提供 |

学术类合集和下列单册包包含重复题目，按需要选择一种下载即可。

| 学术类单册 | 题数 |
| --- | --- |
| [剑雅 4](https://github.com/tomot-ch/aielts-writing/releases/download/v2/ielts-writing-cambridge-04.zip) | 8 |
| [剑雅 5](https://github.com/tomot-ch/aielts-writing/releases/download/v2/ielts-writing-cambridge-05.zip) | 8 |
| [剑雅 6](https://github.com/tomot-ch/aielts-writing/releases/download/v2/ielts-writing-cambridge-06.zip) | 8 |
| [剑雅 7](https://github.com/tomot-ch/aielts-writing/releases/download/v2/ielts-writing-cambridge-07.zip) | 8 |
| [剑雅 8](https://github.com/tomot-ch/aielts-writing/releases/download/v2/ielts-writing-cambridge-08.zip) | 8 |
| [剑雅 9](https://github.com/tomot-ch/aielts-writing/releases/download/v2/ielts-writing-cambridge-09.zip) | 8 |
| [剑雅 10](https://github.com/tomot-ch/aielts-writing/releases/download/v2/ielts-writing-cambridge-10.zip) | 8 |
| [剑雅 11](https://github.com/tomot-ch/aielts-writing/releases/download/v2/ielts-writing-cambridge-11.zip) | 8 |
| [剑雅 12](https://github.com/tomot-ch/aielts-writing/releases/download/v2/ielts-writing-cambridge-12.zip) | 8 |
| [剑雅 13](https://github.com/tomot-ch/aielts-writing/releases/download/v2/ielts-writing-cambridge-13.zip) | 8 |
| [剑雅 14](https://github.com/tomot-ch/aielts-writing/releases/download/v2/ielts-writing-cambridge-14.zip) | 8 |
| [剑雅 15](https://github.com/tomot-ch/aielts-writing/releases/download/v2/ielts-writing-cambridge-15.zip) | 8 |
| [剑雅 16](https://github.com/tomot-ch/aielts-writing/releases/download/v2/ielts-writing-cambridge-16.zip) | 8 |
| [剑雅 17](https://github.com/tomot-ch/aielts-writing/releases/download/v2/ielts-writing-cambridge-17.zip) | 8 |
| [剑雅 18](https://github.com/tomot-ch/aielts-writing/releases/download/v2/ielts-writing-cambridge-18.zip) | 8 |
| [剑雅 19](https://github.com/tomot-ch/aielts-writing/releases/download/v2/ielts-writing-cambridge-19.zip) | 8 |

### 怎样导入

完整版自动导入内置题目；以下步骤适用于单独下载题包或补充题目。

1. 解压下载的 ZIP，找到 `images/` 文件夹。
2. 打开应用“准备&其他 → 下载或自行上传题目 → 选择图片导入”。
3. 批量选择 PNG，确认标题与 Task 类型后导入；文件名含 `Task2` 的图片会自动识别为 Task 2。

当前不会直接导入 ZIP / PDF，也没有应用内自动下载。每个题包包含独立图片和说明，原始 PDF 不随本次发布。题目版权归原权利人所有；整理与下载不改变其原有权利。题目目录记录来源、页码与图片校验值，便于核对或报告问题。

## Prompt 包

[下载 v2 前置规则与补充 Prompt](https://github.com/tomot-ch/aielts-writing/releases/download/v2/ielts-writing-prompts.zip)。

| 文件 | 用途 |
| --- | --- |
| `ielts-writing-analysis.txt` | 可复制到设置的默认补充 Prompt，保留原始正文 |
| `review.md` | 通用雅思批阅前置规则 |
| `review-task1.md` | Academic Task 1 专项规则 |
| `annotations.md` | 结构化批注协议，供开发者和 Prompt 贡献者参考 |

后三项与 v2 源码一致，程序会自动加入，不必重复粘贴到普通自定义 Prompt。批注协议依赖应用提供的文本片段编号，不能直接当作独立聊天教程使用。包内附新版设置步骤和 SHA-256 校验。

**欢迎提供优秀的评阅或辅助修改 Prompt**，也欢迎通过 [Issue](https://github.com/tomot-ch/aielts-writing/issues/new/choose) 反馈错误、建议功能、补充题目。贡献方式见[贡献指南](../../CONTRIBUTING.md)。

## 清单与校验

- [SHA256SUMS.txt](https://github.com/tomot-ch/aielts-writing/releases/download/v2/SHA256SUMS.txt)：所有本次上传附件的 SHA-256（清单自身除外）。
- [question-catalog.json](https://github.com/tomot-ch/aielts-writing/releases/download/v2/question-catalog.json)：156 题目录、来源页码和图片校验值。
- [Release 页面](https://github.com/tomot-ch/aielts-writing/releases/tag/v2)：查看全部附件、发布说明与 GitHub 自动生成的源码归档。

仓库中的[题目目录副本](question-catalog.json)便于直接浏览；其内容与下载附件相同。

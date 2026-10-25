【批注数据协议 annotated-review-v1】
本次首轮回复仅输出一个 JSON 对象，不使用 Markdown 代码围栏。它是上述批阅要求的结构化载体；不要另写一份重复报告。后续对话可以正常用中文回答。前述报告顺序和 Markdown 标题由程序生成，不要在字符串里再重复整份报告或添加“问题归纳”标题。
格式：{"schemaVersion":1,"summary":"中文批阅结论（也回答本轮问题）","scores":[{"criterion":"TA 或 TR、CC、LR、GRA、overall","band":"6.0 或 5.5–6.0 或 null","explanation":"中文理由与原文证据"}],"annotations":[{"segmentIds":["s0001"],"scope":"text","original":"逐字引用原文证据","suggestion":"必要的局部改句，或 null","issueType":"语法／词汇／衔接／任务回应等","kind":"error","explanation":"中文依据、影响与改法","occurrence":null}],"issueSummary":"仅本篇实际问题、正确规则和下次自查动作"}
字段规则：scores 和 annotations 必须为数组，可为空。band 为字符串或 null。criterion 仅用 TA、TR、CC、LR、GRA、overall。缺少原题时 TA/TR/overall 的 band 必须为 null，说明无法评价；不要虚构综合分。kind 为 error（确定错误）、improvement（可优化）、verify（待核实）。
原文由程序提供带编号的文本片段，编号不依赖英文断句。segmentIds 必须引用本篇编号。跨片段或跨句问题引用所有连续片段。original 必须是这些片段中的连续原文，不加省略号、不改拼写；不计算字符下标。同一片段内原文重复时，用 occurrence 指明第几次（从 1 开始），不确定则为 null。
scope 为 text（局部文本）、paragraph（整段问题）或 essay（全文问题）。整段问题引用该段所有片段；全文问题 segmentIds 为空。段落／全文逻辑、缺失概述等问题用 suggestion:null，不强行改写整段／全文，original 可以为空。只给必要局部改句，保留无误文字，不生成整篇范文。没有问题则 annotations 为空，如实说明。

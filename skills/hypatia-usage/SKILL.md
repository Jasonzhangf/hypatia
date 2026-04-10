---
name: hypatia-usage
description: "Use Hypatia as your local AI memory + code index. Covers initialization, mining, search strategies (FTS vs vector vs JSE), and knowledge management. Trigger when: initializing hypatia, indexing code/projects, deciding which search mode to use, or managing memory."
user-invocable: true
allowed-tools: Bash, Read, Grep, Glob
argument-hint: <initialization | indexing | search | memory management>
---

# Hypatia Usage Guide

Hypatia 是一个一体化的本地 AI 记忆系统 + 代码索引工具，支持：结构化知识（Knowledge + Statement 三元组）、全文检索（FTS5）、语义向量检索（BERT embedding）、混合搜索（FTS + 向量 RRF 融合）。

## 1. 初始化

```bash
# 一键初始化：创建 shelf + 自动下载 embedding 模型
hypatia init ~/hypatia-data

# 模型自动下载到 ~/.hypatia/models/（约 86MB，首次）
# 后续启动自动检测，已存在则跳过
```

初始化后 `~/.hypatia/models/` 包含：
- `config.json` — BERT 配置
- `tokenizer.json` — 分词器
- `model.safetensors` — all-MiniLM-L6-v2 模型（384 维）

如果没有模型，Hypatia 会自动降级到 hash 嵌入（确定性，但无语义）。

## 2. 索引代码（Mine）

```bash
# 全量索引一个项目目录
hypatia mine ~/github/routecodex --shelf default

# 增量索引（只索引变更文件）
hypatia watch --shelf default

# 索引时自动：WalkDir 扫描 → 按代码结构切块 → 生成 embedding → 写入 FTS + 向量
```

索引后的数据：
- 每个代码块 → 一条 Knowledge 记录（含 path/lang/symbol 元数据）
- FTS 索引 → 支持关键词全文搜索
- 向量索引 → 支持语义相似度搜索

## 3. 知识管理

```bash
# 创建知识节点
hypatia knowledge-create "RouteCodex" -d "local AI memory system" -t "rust,ai"

# 创建三元组关系
hypatia statement-create RouteCodex manages OpenClash
hypatia statement-create WebAuto uses Camoufox

# 获取/删除
hypatia knowledge-get RouteCodex
hypatia knowledge-delete RouteCodex
hypatia statement-delete RouteCodex manages OpenClash
```

## 4. 搜索策略（关键：什么时候用什么）

### 4.1 模糊搜索 → `hypatia search`（FTS5）

**什么时候用：**
- 用户输入自然语言、口语化查询、可能有错别字
- 例如："找个和网络路由有关的东西"、"那个记认证逻辑的文件"
- Agent 需要"召回"记忆时的 first-pass

**特点：**
- BM25 加权：key=10, tags=5, synonyms=3, data=1
- Porter 词干提取：running → run, databases → databas
- 同义词扩展：创建时加 synonyms 可增强召回
- 支持多词搜索（会自动转义）

**示例：**
```bash
hypatia search "routing database"
hypatia search "authentication" --limit 5
```

### 4.2 精确搜索 → `hypatia query`（JSE）

**什么时候用：**
- Agent 内部执行查询（已知结构、精确条件）
- 需要布尔组合、关系过滤、时间范围
- 例如："找到所有 subject=RouteCodex 的三元组"、"找标签含 rust 的知识"

**JSE 语法示例：**
```bash
# 精确匹配
hypatia query '["$knowledge", ["$eq", "name", "RouteCodex"]]'

# 模糊模式
hypatia query '["$knowledge", ["$like", "tags", "%networking%"]]'

# 组合条件
hypatia query '["$and", ["$like", "name", "%Rust%"], ["$like", "tags", "%language%"]]'

# 三元组查询
hypatia query '["$statement", ["$triple", "RouteCodex", "$*", "$*"]]'
```

### 4.3 语义搜索 → `hypatia vsearch`（向量）

**什么时候用：**
- 语义相似但字面无关键词的查询
- 例如：搜 "authentication" 想命中 "OAuth2 token validation"
- 需要理解意图而非匹配字面

**示例：**
```bash
hypatia vsearch "how to secure API endpoints" --limit 5
```

### 4.4 混合搜索 → `hypatia hybrid`（FTS + 向量 RRF）

**什么时候用：**
- 不确定是关键词匹配还是语义匹配更好时
- 想要兼顾精确命中和语义发现
- 推荐作为 Agent 默认搜索策略

**原理：** RRF（Reciprocal Rank Fusion）融合 FTS 排名和向量排名，两者互补。

**示例：**
```bash
hypatia hybrid "route configuration" --limit 10
```

## 5. 推荐搜索决策树

```
用户输入
  │
  ├─ 有明确实体名/标签/关系？
  │   └─ YES → hypatia query（JSE 精确查询）
  │
  ├─ 需要语义理解/意图识别？
  │   └─ YES → hypatia hybrid（混合搜索，最稳）
  │
  ├─ 纯关键词匹配就够？
  │   └─ YES → hypatia search（FTS，最快）
  │
  └─ 字面完全不匹配但语义相关？
      └─ YES → hypatia vsearch（向量，最语义化）
```

## 6. 日常运维

```bash
# 查看状态（shelf 数量、FTS 文档数、向量数量）
hypatia status

# 健康检查（FTS 完整性、数据一致性）
hypatia doctor

# 导出 shelf
hypatia export default ~/backup/hypatia-shelf
```

## 7. Agent 集成最佳实践

1. **用户说"记住 X"**：→ `knowledge-create` + `statement-create`（主动建关系）
2. **用户说"找 X"**：→ 先判断类型 → 选 search/query/hybrid/vsearch
3. **定期索引**：Agent 启动时执行 `watch` 增量索引
4. **查询优先**：混合搜索 > FTS > 精确查询 > 向量（混合搜索覆盖最广）

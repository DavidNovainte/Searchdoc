# SearchDoc Roadmap

## Phase A — 骨架（已完成）

- Tauri + React 桌面壳
- SQLite FTS5
- Local md/txt 扫描
- 统一搜索 UI + 片段
- 扁平深色 UI

## Phase B — 真·Google Docs（已完成主干）

- Google Cloud OAuth（Desktop loopback）
- Drive `files.list` 过滤 Docs
- export `text/plain` 入库
- 连接/断开/保存配置 UI
- token 本机存储

## Phase C — 自用打磨（已完成主干）

- [x] pdf / docx 文本抽取
- [x] 本地 mtime / hash 增量
- [x] 全局快捷键唤起（Ctrl+Shift+Space）
- [x] Google `modifiedTime` 增量跳过
- [x] 来源过滤（仅本地 / 仅 Google）
- [x] 托盘驻留（关窗隐藏 + 菜单）
- [x] 粘贴 / 批量 Docs 链接观察列表
- [x] 深度搜索 v1：1–2 层 / 限额 / 仅已索引或可拉取 / 统计
- [x] Google 同步模式：watchlist_only / recent
- [x] Google 按文件夹筛选（Recent 模式、直接子文档、多文件夹 OR）
- [x] 开机自启（托盘静默启动 + 设置开关）
- [x] 搜索增强：scope（标题/正文）· sort（相关/最新）· OR 语法
- [x] 搜索页 / 侧栏信息架构精简

## Phase C+ — 正文搜索深挖（进行中）

- [x] 搜索语法：OR / `|`、NOT / `-term`
- [x] 范围（标题/正文）与排序（相关/最新）
- [x] 同步可取消、整盘确认、首启引导
- [ ] 书签/固定查询
- [ ] 同步细粒度进度（文件计数）
- [ ] 正式安装包分发说明

## Phase D — v0.2 可靠性与发布准备

详细执行计划见 [V0.2_PLAN.md](V0.2_PLAN.md)。

- [ ] 同步状态、进度、取消和错误分类
- [ ] 索引安全、数据库备份与恢复
- [ ] 搜索质量回归测试与性能基线
- [ ] LICENSE、CI、安装和升级验证
- [ ] 更新过时的架构与配置文档

## Phase E — 开源准备

- LICENSE
- OAuth 配置文档（不提交密钥）
- 基础单测（FTS query / local scan）
- CONTRIBUTING 与架构说明

## 明确后置（定位外，暂不做）

- 语义向量 / RAG 问答
- 网页深搜 / Notion 等新源（等正文路径「尖」了再开）
- 多用户企业权限
- 对标 Everything 的全盘文件名秒搜

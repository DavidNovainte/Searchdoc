# Google Docs 连接说明

SearchDoc 通过 **OAuth 桌面应用 + Drive API** 读取你的 Google Docs 正文，索引保存在本机。

## 1. 创建 Google Cloud 项目

1. 打开 [Google Cloud Console](https://console.cloud.google.com/)
2. 新建项目（或选用已有项目）
3. 启用 API：
   - **Google Drive API**
   - （可选）Google Docs API

## 2. 配置 OAuth 同意屏幕

1. **API 和服务 → OAuth 同意屏幕**
2. User Type 选 **外部**（个人 Google 账号常用）
3. 填应用名，例如 `SearchDoc`
4. 添加自己的测试用户邮箱（Testing 阶段必须）
5. Scope 可在客户端请求时再带；SearchDoc 使用：
   - `https://www.googleapis.com/auth/drive.readonly`
   - `https://www.googleapis.com/auth/documents.readonly`

## 3. 创建 OAuth 客户端

1. **凭据 → 创建凭据 → OAuth 客户端 ID**
2. 应用类型选 **桌面应用**
3. 创建后得到：
   - Client ID
   - Client Secret

### 关于 Redirect URI

SearchDoc 使用 **loopback**：

```text
http://127.0.0.1:<随机端口>
```

桌面客户端通常允许 loopback。若 Google 控制台要求填写，可加：

```text
http://127.0.0.1
http://localhost
```

实际端口由应用每次启动本地监听时分配。

## 4. 在 SearchDoc 里连接

1. 打开应用 → **设置**
2. 填入 Client ID / Client Secret → **保存配置**
3. 点 **连接 Google**，浏览器完成授权
4. 授权成功后自动做一次同步（默认最多约 200 篇 Docs）
5. 回到 **搜索** 用关键词检索；需要指定文档时点 **添加 Docs 链接**

也可用环境变量（优先于文件）：

```bash
SEARCHDOC_GOOGLE_CLIENT_ID=...
SEARCHDOC_GOOGLE_CLIENT_SECRET=...
```

## 5. 本机文件位置

- OAuth 配置：`%APPDATA%\SearchDoc\SearchDoc\google_oauth.json`（具体以应用「来源」页路径为准）
- Token：系统凭据库（服务名 `com.searchdoc.google`）；旧版 `google_tokens.json` 会在首次读取时自动迁移并删除
- 索引库：`index.db`

**不要把 client secret / token 提交到 Git。**

## 6. 同步范围（重要）

在 **设置 → Google 同步范围** 可选择：

1. **仅观察列表（默认，推荐）**  
   - 「同步全部 / 同步 Google」只同步你在搜索页粘贴的 Docs  
   - 不会扫描整个云盘  
   - 适合个人精确收藏

2. **最近文档浅同步**  
   - 会拉取最近一批 Google Docs（有数量上限）  
   - 适合想快速扫一圈云端文档的场景  

连接 Google 后：  
- 若是「仅观察列表」，**不会**自动全盘同步  
- 请到搜索页「添加 Docs 链接」后再同步  

## 7. 粘贴 / 批量链接

在 **搜索** 页点 **添加 Docs 链接**，可粘贴：

```text
https://docs.google.com/document/d/DOC_ID/edit
https://docs.google.com/document/d/ANOTHER_ID/edit?usp=sharing
```

也支持逗号/换行分隔的批量链接，或直接贴文档 ID。  
导入后会写入本机观察列表 `google_watchlist.json`，并立即同步正文。

## 8. 限制（当前）

- 仅 Google Docs（`application/vnd.google-apps.document`）
- 全文 export 为 `text/plain`
- 「最近文档浅同步」有篇数上限
- 「仅观察列表」不会自动发现未粘贴的文档
- 深度搜索可跟随 Docs 外链（有层数/限额）
- 「最近文档」支持按 Drive 文件夹筛选（多个文件夹 OR，仅直接子文档）

## 9. 常见问题

**redirect_uri_mismatch**  
检查客户端类型是否为「桌面应用」，并确认 loopback 策略。

**access_denied / 应用未验证**  
Testing 模式下把账号加为测试用户。

**同步 0 篇**  
确认账号下确有 Docs；看状态栏错误；可点来源里的「同步」重试。

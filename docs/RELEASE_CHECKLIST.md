# SearchDoc 发布检查清单

## 自动检查

- `npm ci`
- `npm audit --omit=dev --audit-level=high`
- `npm test`
- `npm run build`
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- `cargo test --manifest-path src-tauri/Cargo.toml`
- 确认 CI 中 RustSec `audit-check` 通过
- `npm run tauri -- build --bundles nsis`

## Windows 发布

- 在干净 Windows 环境安装并启动 MSI/NSIS 安装包
- 验证首次添加文件夹、索引、搜索、预览和打开原文
- 验证托盘隐藏、全局快捷键和退出
- 验证全局快捷键改绑：设置 → 系统 → 输入新键位应用，重启后仍生效；占用冲突时提示并回滚旧键
- 验证 Google OAuth 登录、刷新 Token 和断开连接；同步中限流（429）会自动退避重试
- 验证 Notion：错误 Token / 未授权数据库被拒绝；正确配置后可同步、搜索、打开原文
- 验证备份、恢复以及恢复失败时的救援副本
- 验证升级安装不会删除本地索引
- 为安装包配置代码签名证书

## 发布前人工确认

- 确认许可证和第三方依赖声明
- 确认版本号同步于 `package.json`、`src-tauri/Cargo.toml` 和 `src-tauri/tauri.conf.json`
- 确认 Google OAuth 配置说明和隐私政策
- 确认变更记录和已知问题
## GitHub Release 步骤（开源后）

1. 版本号三处同步：`package.json` · `src-tauri/Cargo.toml` · `src-tauri/tauri.conf.json`
2. 提交并打 tag：`git tag v0.1.0 && git push origin v0.1.0`
3. 等 CI 全绿（fmt/clippy/双端测试/RustSec 审计）
4. 本地 `npm run tauri -- build --bundles nsis`，校验安装包能在干净环境启动
5. 在 GitHub Releases 新建 Release（用 tag），上传 NSIS/MSI 安装包与 SHA256 校验文件，正文写变更要点
6. **激活应用内更新检查**：把 `src-tauri/src/update.rs` 中 `UPDATE_REPO` 填为 `"你的用户名/searchdoc"`，随下个版本发布生效
7. 之后每个版本：用户在「设置 → 关于」点击检查更新即可看到新版并跳转下载页

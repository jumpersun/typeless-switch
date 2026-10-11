# Typeless Switch

一个免费、开源的 Typeless 桌面账号切换工具。把已登录的账号加密保存在本机，需要时一键切换。

保存当前账号 → 添加另一个账号 → 一键切换。

- **本地保存**：macOS 使用系统钥匙串保护加密密钥，Windows 使用当前用户的 DPAPI。
- **账号管理**：保存、添加、切换、更新登录状态和移除；异常中断时可恢复原账号。
- **简洁界面**：普通桌面窗口、字母头像，支持中文 / English 和浅色 / 深色 / 跟随系统。
- **开源透明**：基于 Tauri 2、Rust 和 React，采用 MIT 许可证，无自建服务器和遥测。

## 下载安装

**[下载 macOS Apple Silicon 安装包](https://github.com/jumpersun/typeless-switch/releases/download/v0.1.0/Typeless-Switch-0.1.0-20261011-131258-macOS-AppleSilicon.zip)** · [发布说明与校验文件](https://github.com/jumpersun/typeless-switch/releases/tag/v0.1.0)

适用于 M 系列 Mac。退出旧版，解压 ZIP，将 `Typeless Switch.app` 拖入「应用程序」即可；升级时保留原账号库。

当前为 **v0.1.0 预发布版**，使用 ad-hoc 签名，未经 Apple 公证。完整原生界面和真实双账号切换验收仍待完成；已通过的检查见 [VALIDATION.md](VALIDATION.md)。Windows 适配代码已包含，尚未提供经过验证的 Windows 安装包。

工具只管理已有账号，不包含词库同步、邮箱工具或菜单栏 / 托盘模式。

## 使用

1. 安装并登录官方 Typeless 客户端。
2. 打开 Typeless Switch，点击「保存当前账号」。
3. 点击「添加账号」，阅读三步引导，结束录音后点击「打开 Typeless」。使用已有邮箱在官方窗口注册或登录，按提示验证，再回到工具点击「我已登录，检测并保存」。检测针对本地登录文件，不代表服务端认证验收。
4. 在列表中点击「切换」。如果登录状态过期，通过「添加账号」重新登录同一账号即可更新。

切换会重启 Typeless。macOS 首次切换可能请求控制 Typeless 的自动化权限，用于正常退出客户端。Windows 托盘程序若未响应退出请求，请从托盘手动退出后再试。

## 本地数据

- macOS：账号库使用 AES-256-GCM 加密，密钥保存在系统钥匙串。
- Windows：使用当前 Windows 用户的 DPAPI 加密整个账号库。
- 账号库位于系统本地应用数据目录下的 `io.typeless-switch.app/vault.bin`。
- 登录仅在 Typeless 官方客户端完成。工具没有自建服务器，也不收集遥测。不调用词典接口，登录凭据不进入前端或日志。
- 切换只更新 Typeless 的 `user-data.json` 和 `app-storage.json` 中的登录缓存；历史数据库、录音、设置、设备标识保持原样。它不提供不同账号之间的历史数据隔离。
- 修改前先将原文件备份到加密账号库；中断时显示「恢复原账号」。不要在恢复完成前删除账号库。
- 账号库绑定本机系统用户，不适合作为跨设备同步文件。

「已登录」表示两个本地登录文件的身份一致，不代表服务器已接受令牌。切换后请在 Typeless 中确认账号并试用语音输入。

## 开发

需要 Node.js 22.12+、Rust stable，以及 [Tauri 系统依赖](https://v2.tauri.app/start/prerequisites/)。Windows 需要 MSVC C++ 工具链和 WebView2。

```sh
npm ci
npm run tauri -- dev
```

针对性验证：

```sh
npm exec tsc -- --noEmit
npm run test:core
npm run test:preferences
```

仅预览界面（使用明确标记的示例数据，不访问真实账号）：

```sh
npm run dev
# 打开 http://127.0.0.1:1420/?preview=accounts
# 也可使用 ?preview=empty 或 ?preview=recover
```

生成当前平台安装包：

```sh
npm run tauri -- build
```

macOS 也可运行 `bash scripts/package-macos.sh`，安装包统一放入 `releases/<构建标识>/`，构建缓存放入 `.work/`。脚本使用本机 ad-hoc 签名；磁盘镜像服务不可用时会生成 ZIP 安装包。这些生成文件不提交到 Git。

`.github/workflows/build.yml` 提供 macOS / Windows 编译和安装包工作流。未配置开发者签名或 Apple 公证，发布前请自行配置。当前验证记录见 [VALIDATION.md](VALIDATION.md)。

## 自定义 Typeless 路径

默认识别 macOS 的 `/Applications/Typeless.app` 或 `~/Applications/Typeless.app`；Windows 的 `%LOCALAPPDATA%/Programs/Typeless/Typeless.exe`、`%LOCALAPPDATA%/Typeless/Typeless.exe` 和 `%ProgramFiles%/Typeless/Typeless.exe`。

非标准安装可在启动前设置：

- `TYPELESS_APP_PATH`：Typeless **可执行文件**完整路径（macOS 为 `Typeless.app/Contents/MacOS/Typeless`）。
- `TYPELESS_DATA_DIR`：Typeless 用户数据目录；默认 macOS `~/Library/Application Support/Typeless`，Windows `%APPDATA%/Typeless`。

## 兼容性与范围

这是独立社区项目，与 Typeless 官方没有隶属关系。当前会话格式依据公开项目进行兼容性实现；官方更新可能使其失效，遇到未知格式会停止切换并保留原文件。

第一版聚焦个人已有账号管理，不提供批量注册、账号交易、设备标识重置、私有接口调用或额度自动轮换。功能保持精简，欢迎提交问题和兼容性修复；请不要在 issue 中上传登录文件或账号库。

## 结构

- `src/`：账号管理界面、语言与主题设置；IPC 不返回登录令牌。
- `src-tauri/src/engine.rs`：账号操作、事务日志、失败恢复。
- `files.rs` / `codec.rs`：文件边界与会话格式兼容。
- `vault.rs`：系统加密存储。
- `platform.rs`：客户端发现、退出、启动。
- `src-tauri/tests/switch_safety.rs`：独立临时目录中的核心安全测试。

参见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) 与 [LICENSE](LICENSE)。

## 参与开发

欢迎提交 issue 和 pull request。报告问题时请提供操作系统、Typeless 版本、复现步骤和脱敏后的报错；不要上传登录缓存、账号库、令牌或钥匙串导出文件。开发与提交约定见 [CONTRIBUTING.md](CONTRIBUTING.md)。

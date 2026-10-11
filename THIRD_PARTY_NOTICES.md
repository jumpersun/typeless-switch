# 来源与依赖

本项目是独立实现，不包含 Typeless 官方客户端、商标素材或服务端代码。

会话加密格式的兼容性研究参考：

- [fufu1209/Typeless](https://github.com/fufu1209/Typeless/tree/ece9790cf77462c47c60f8a5973a6af7896ef340)，MIT。参考公开描述的 PBKDF2 / AES-CBC 格式及官方客户端文件布局。
- [StatXzy7/typeless-switch](https://github.com/StatXzy7/typeless-switch/tree/93eb1b8a286f900f571f433fee680c7ee066b2a8)，Apache-2.0。参考 Windows 路径、DPAPI 保护及恢复流程。

未移植上述项目的 UI、批量注册、设备重置、私有 API 或解包脚本。`scripts/make-fixtures.mjs` 仅生成虚构账号的 Node.js 兼容性样本。


使用 Tauri、React、Vite、RustCrypto、security-framework、windows-sys、sysinfo 等开源依赖，各依赖仍遵循其各自许可证。准确版本记录在 package-lock.json 与 src-tauri/Cargo.lock。

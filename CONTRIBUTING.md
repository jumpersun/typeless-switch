# 参与开发

项目聚焦本机已有账号管理，欢迎兼容性修复、失败恢复、无障碍、语言和界面方面的改进。

## 本地运行

需要 Node.js 22.12+、Rust stable 和 [Tauri 系统依赖](https://v2.tauri.app/start/prerequisites/)。

```sh
npm ci
npm run tauri -- dev
```

只检查界面时，可运行 `npm run dev` 并打开 `http://127.0.0.1:1420/?preview=accounts`。这是示例数据，不访问真实账号。

## 验证

按修改范围执行相关检查：

```sh
npm exec tsc -- --noEmit
npm run test:preferences
npm run test:core
```

核心测试使用隔离临时目录和虚构会话，固定样本由 `scripts/make-fixtures.mjs` 生成。不要把真实账号用于测试夹具。

界面改动请检查中英文、浅深色和窄窗口；账号流程改动请覆盖失败回滚、重启恢复和文件边界。提交说明中区分模拟测试、原生客户端测试及服务端登录验证。

## 提交问题和变更

- 一个 pull request 聚焦一个问题，写明触发条件、修改行为和验证结果。
- 保留会话加密、操作互斥和事务恢复；不要把凭据传入前端、日志或命令行。
- issue、截图和补丁都不得包含真实邮箱、令牌、登录文件、账号库或签名证书。
- 本地依赖、构建产物、安装包、过程记录和调研副本不提交到源码仓库。

提交的代码按项目 [MIT 许可证](LICENSE) 分发。引入第三方代码时保留必要的许可证与署名，并更新 [来源说明](THIRD_PARTY_NOTICES.md)。

# Echo Switch 图标

以用户提供的白底、黑色弧形图标为视觉参考，两条相向弧线表达账号切换。原生应用图标采用珍珠白圆角底座与黑色主体。

## 资源

- `src-tauri/icon.svg`：统一矢量源。
- `src-tauri/icons/icon.png` 及尺寸 PNG：位图资源。
- `src-tauri/icons/icon.icns`：macOS 图标。
- `src-tauri/icons/icon.ico`：Windows 图标。

这些桌面资源由现有 Tauri CLI 从 SVG 生成。生成图仅用于早期构图探索，过程稿已清理，SVG 和最终桌面图标保留。

## 应用内显示

`src/main.tsx` 内联引用同一 SVG，`src/style.css` 移除应用内小标志的底板、阴影滤镜和立体变换，并根据主题设置前景色。账号头像显示邮箱首字母，不使用应用标志。

更新原生图标资源后需要重新构建并安装 App；只修改前端不会更新已安装的 Dock / Windows 图标。

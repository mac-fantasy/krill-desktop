# macOS 应用图标

`AppIcon.svg` 是可编辑源文件。主体参考 Krill 官网当前的几何磷虾品牌图形，使用珊瑚粉渐变；薄荷绿进度环与深色圆角底板延续桌面端的额度展示和配色。

品牌参考（2026-10-15）：

- <https://www.krill-code.com/app>
- <https://www.krill-code.com/brand/icon-160.png>

图标通过矢量路径绘制，不依赖字体、外链图像或第三方绘图库。SVG 保留透明边距与圆角，PNG 为 1024 × 1024 预览，ICNS 包含 16、32、128、256、512 点及对应 Retina 尺寸。该图标是桌面客户端的适配设计，并非从官网取得的官方 macOS 图标。

菜单栏的 22 点图标由 `Sources/KrillMac/App/MenuBarIcon.swift` 直接绘制，沿用相同的分段磷虾轮廓，将眼睛简化为镂空以改善小尺寸辨识度。Logo 和剩余额度圆环均使用系统单色模板渲染；修改品牌轮廓时，应同时检查应用图标与菜单栏路径。

修改 SVG 后，在 macOS 的仓库根目录执行：

```bash
swift scripts/macos/generate_icon.swift
bash scripts/macos/build.sh release
```

生成脚本使用系统 AppKit 渲染与 `iconutil` 打包，将 PNG 和 ICNS 写回本目录；中间 PNG 位于忽略版本控制的 `artifacts/macos-icon/`。应用构建直接复制已生成的 ICNS，构建时无需下载官网资源。

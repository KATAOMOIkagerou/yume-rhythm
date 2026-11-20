# 桌宠节奏师 · Rust 独立桌面版

这是原软件的独立重构版本。本目录包含自己的源码、素材、依赖锁文件、构建脚本和发布程序；复制本目录后可以独立构建，复制 `bin` 文件夹后可以直接运行。原项目无需同时存在。

## 直接运行

双击 **`bin/桌宠节奏师.exe`**，或 **`启动桌宠.cmd`**。

运行环境为 Windows 10/11 x64，需要安装 Microsoft Edge WebView2 运行时。运行发布程序不需要 Electron、Node.js、npm、Rust 或原项目文件。`bin` 内的 `character_assets` 供画手修改；程序还内嵌了默认界面和素材，因此删除外置素材后仍可使用默认素材。

控制台可以拖动、调整大小、关闭后重新打开；关闭控制台会隐藏窗口，音频和计时继续运行。桌宠尺寸为 540×590，控制台初始尺寸为 470×780，均为逻辑像素，随系统缩放。

桌宠背景完全透明。未锁定时，角色图层中透明的部分（含图片内部空隙和按钮圆角外侧）自动穿透到下方窗口，角色实体和按钮仍可操作；锁定后整个桌宠都穿透。命中区域随表情、图层和 24 fps 动画变化更新；原生侧每 16 毫秒读取鼠标位置，仅在命中状态变化时切换穿透。画面与命中判断分离，不裁切窗口。

| 操作 | 快捷键 |
| --- | --- |
| 打开控制台 | Ctrl＋Alt＋C |
| 锁定／解锁桌宠并切换点击穿透 | Ctrl＋Alt＋L |
| 音频开始／停止 | 空格 |
| 停止音频 | Q |
| 隐藏控制台 | H |

托盘右键菜单也可打开控制台、锁定／解锁或退出。同时运行原版时，全局快捷键可能已被原版占用；退出原版后重新启动本版即可。

## 重构范围

采用 **Rust + Tao + Wry + Windows WebView2**。Rust 负责透明原生窗口、可关闭悬浮控制台、拖动、锁定和点击穿透、托盘、全局快捷键、内嵌资源服务、系统声音设置入口，以及 WASAPI 系统输出音频回环。

为了保持原有 UI 对齐、角色动画和节拍判断行为，继续使用原有 React/TypeScript/CSS 界面，以及冻结的节拍检测算法。这是 Rust 原生桌面后端配合 WebView2 界面的版本，**不是全部界面和算法均改写为 Rust**。运行时已移除 Electron 和 Node.js。

主窗口和控制台共享同一棵 React 状态树，通过 portal 显示在独立原生窗口中，避免两个窗口各自维护计时器、音频引擎和全局设置。角色动画保持 24 fps。现有整体/配件表情、定时随机表情、待办、正计时、倒计时和提示音开关均保留。

音频同步使用系统默认输出设备的 WASAPI 回环，不再捕获屏幕视频。PCM 数据仅在内存中流转，原生侧最多排队三个 1024 采样块，前端使用固定环形缓冲区。切换输出设备后应停止并重新开始监听。保留拖入音频文件播放及快捷键控制功能。

设置和待办位于 `%LOCALAPPDATA%/DesktopPetRhythmRust` 中的 WebView2 用户数据目录，与原 Electron 版分开；不会自动迁移旧版设置。更换素材仍使用 `character_assets/manifest.json` 和 `expressions.json`，点击「重载素材」生效。

## 源码结构

| 路径 | 内容 |
| --- | --- |
| `native/src/main.rs` | Rust 窗口、资源服务、托盘、快捷键、桌面指令 |
| `native/src/audio.rs` | WASAPI 输出音频回环与捕获线程生命周期 |
| `native/src/hit_region.rs` | 存储图层透明度命中掩码，只控制输入穿透，不裁切画面 |
| `native/src/bridge.js` | WebView2 与 Rust 的请求／响应桥接 |
| `src/audio/native-system.ts` | 保留检测引擎，接入 Rust 提供的音频采样 |
| `public/native-pcm-worklet.js` | 有界音频缓冲和采样率适配 |
| `public/pet-hit-region.js` | 缓存静态图层，按实时变换生成可见像素命中区域 |
| `src/gui`、`src/character`、`src/state` | 原有界面、24 fps 角色动画、设置和计时 |
| `public/character_assets` | 独立复制的完整素材与表情配置 |
| `scripts` | 独立构建及验证脚本 |

## 从源码构建

需要 Node.js 20.19+、Rust stable（最低 1.88；本机验证 1.100.0）、MSVC x64 C++ 工具链、Windows SDK 和 WebView2 运行时。推荐安装 Visual Studio Build Tools 的「使用 C++ 的桌面开发」。

```powershell
npm ci
npm run verify
npm run build
npm start
```

本机临时安装的 Rust 工具位于 `.tools`，脚本会优先使用该目录；其他机器也可直接使用已安装的 Rust。`scripts/toolchain.mjs` 自动查找 MSVC 和 Windows SDK。`native/Cargo.lock` 与 `package-lock.json` 锁定依赖；构建不会读取上级项目。

`npm run build` 先编译界面，再编译 Rust 发布程序，将 EXE、素材、许可证和使用说明放到 `bin`。界面与默认素材由 Rust 在编译时嵌入 EXE，无需运行前端服务器。

## 验证

```powershell
npm run verify
npm run verify:desktop
npm run verify:saved-state
```

第一项验证计时和表情配置、音效逻辑、中文界面，并使用 `scripts/ui-baseline.json` 核对原有样式、组件、素材和冻结算法的一致性。

第二项实际启动发布程序，在两种底色及锁定／解锁状态下采样真实 Windows 桌面像素，比较半透明边缘与预期颜色合成，并检查素材加载、原生双窗口尺寸、透明背景、Windows 点击穿透标志、表情与随机恢复、双向计时、控制台隐藏后续计、完成音效与静音、待办保存、真实 WASAPI 回环和 120 BPM 检测、停止释放捕获以及四页溢出。测试会短暂播放节奏信号，截图和结果写入 `.tmp`，测试结束关闭其启动的程序。

第三项使用前一次验证的用户目录重启程序，检查设置与待办保存、外置素材更新和资源路径保护。

可通过 `PET_TEST_EXE` 指向复制到其他位置的 EXE，验证独立运行；通过 `PET_REFERENCE_LAYOUT` 传入原版控件位置记录，逐项比较四页控件坐标。测试开启的本机调试端口仅用于验证，正常启动不会开启。

本次验收记录见 `验证报告.md`。

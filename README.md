# 桌宠节奏师（YumeRhythm · 纯原生 Rust 版）

100% 纯原生 Rust Windows 桌面伴侣，彻底移除 Electron、Node.js、React、Vite、Tailwind 与 WebView2；单进程、零 GPU 进程、零白边逐像素透明穿透、低至个位数 MiB 内存与亚毫秒级渲染。

## 核心特性

- **原生分层透明桌宠窗口**（`crates/pet-render`）
  - 基于 Win32 `WS_EX_LAYERED` + `UpdateLayeredWindow`（32-bit 预乘 BGRA `DIBSection`）与 `tiny-skia` 紧致包围盒合成器。
  - Windows DWM 原生 O(1) 逐像素 Alpha 穿透（透明区域自然透传点击，保留柔和边缘），彻底告别 `getImageData` 全画布读回与 `SetWindowRgn`。
  - 31 张角色素材在加载阶段自动裁剪非零 Alpha 紧致包围盒并预乘，解码常驻内存由 75.7 MiB 压缩至 ~3.1 MiB（节省 95.9%）。
  - 悬停顶部悬浮工具栏（控制台 / 锁定 / 音频同步），支持拖动定位、拖入本地音频文件直接跟拍，锁定时自动切换 `WS_EX_TRANSPARENT` 整窗点击穿透。
- **零 GPU 独立浮动控制台**（`crates/pet-ui`）
  - 基于 `egui` 无头布局 + 纯 Rust CPU 软件光栅化器 + Win32 原生 `HWND`/`BitBlt`，自动加载系统微软雅黑（`msyh.ttc`）并支持原生中文 IME 输入。
  - 关闭/隐藏控制台时立即销毁窗口与像素缓冲释放内存，后台计时、音频同步与配置状态持续运行。
  - 包含「状态」「情绪」「音频同步」「设置」四个完整中文标签页、双向自习计时器（倒计时/正计时墙钟防漂移 + 三音完成提示音）、待办清单抽屉、随机动作情境矩阵（音乐期强制禁止打瞌睡）与二次确认弹窗。
- **纯原生 WASAPI 音频同步与节拍引擎**（`crates/pet-audio`）
  - `wasapi` 共享模式回环捕获（支持默认输出与枚举指定输出设备）→ 无锁 `ringbuf` → 独立 DSP 线程。
  - `rustfft` 2048 点 Blackman 窗 FFT + 16 频带对数通量节拍检测器，真正接入灵敏度参数（`1.05..=2.00`，基准 `1.22`）。
  - 支持拖入本地音乐（MP3 / WAV / FLAC / OGG / AAC / M4A，基于 `symphonia` 解码循环播放并同步跟拍）与内置 112 BPM 演示鼓点、倒计时三音完成提示音合成。
- **统一状态机与热重载画手素材**（`crates/pet-core` + `crates/pet-app`）
  - 默认素材内嵌于单文件 `yume-rhythm.exe`，开箱即用；可执行文件同级放置 `character_assets/` 目录（或设置 `YUME_ASSET_DIR`）即可热重载自定义画手素材包，兼容 `manifest.json` + `expressions.json`（支持 PNG / APNG / GIF / 动画 WebP）。
  - 本地配置、待办与随机事件矩阵原子持久化于 `%LOCALAPPDATA%\YumeRhythm\state.json`。

## 构建与运行

前置依赖：Windows 10/11 + Rust 1.88+（MSVC 工具链与 Windows SDK）。

```powershell
# 运行全部单元测试（含音频 120 BPM 锁拍、紧致裁剪、渲染器与控制台）
cargo test --workspace --release

# 构建单文件发布版（target\release\yume-rhythm.exe）
cargo build --release

# 启动桌宠节奏师
.\target\release\yume-rhythm.exe
```

### 内置自检与性能基准命令

```powershell
# 离线自检（素材裁剪、100 帧预乘合成、4 标签页中文光栅化、120 BPM 锁拍与内存报告）
.\target\release\yume-rhythm.exe --self-test

# 实机运行性能基准（支持 --with-console、--screenshot 与 --data-dir 隔离测试）
.\target\release\yume-rhythm.exe --bench 10 --screenshot .tmp\pet.png
.\target\release\yume-rhythm.exe --bench 10 --with-console --screenshot .tmp\pet_with_console.png
```

## 快捷键

| 快捷键 | 作用域 | 操作 |
| --- | --- | --- |
| `Ctrl+Alt+C` | 全局 | 打开控制台 |
| `Ctrl+Alt+L` | 全局 | 锁定 / 解锁桌宠（点击穿透） |
| `Space` | 窗口聚焦且未输入文本 | 开始 / 停止音频同步 |
| `Q` | 窗口聚焦且未输入文本 | 停止音频同步 |
| `H` | 窗口聚焦且未输入文本 | 隐藏控制台（并释放 UI 缓冲） |

若全局快捷键被其他程序占用，应用仍可正常启动，可通过系统托盘菜单随时打开控制台、解锁桌宠、启停音频同步或退出程序。

## Workspace 目录结构

```text
Cargo.toml                 # Workspace 根配置
assets/character/          # 内嵌默认角色素材包（manifest.json / expressions.json / PNGs）
crates/
├── pet-core/              # 领域模型、状态机、持久化、双向计时、素材紧致加载、m1 动画物理
├── pet-audio/             # WASAPI 回环/播放、ringbuf、rustfft 2048-FFT、节拍检测、symphonia 解码
├── pet-render/            # Win32 UpdateLayeredWindow 透明桌宠窗口与 tiny-skia 预乘合成器
├── pet-ui/                # egui 无头布局 + 纯 Rust CPU 软件光栅化 + Win32 浮动控制台
└── pet-app/               # 主入口 yume-rhythm.exe：消息循环、系统托盘、全局热键、--self-test / --bench
```

## 文档

- [纯原生 Rust 重构设计方案](docs/纯原生Rust重构设计方案.md)
- [画手素材替换说明](docs/画手素材替换说明.md)
- [历史模块重构说明](docs/源码重构说明.md)

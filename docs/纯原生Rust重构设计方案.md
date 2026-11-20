# 桌宠节奏师：100% 纯原生 Rust 重构设计方案

## 1. 重构目标与性能预算

本方案将「桌宠节奏师」（`yume-rhythm.exe`）从历史 Electron / WebView2 + React + Vite + Tailwind 架构彻底重构为 **100% 纯原生 Rust Windows 桌面应用**，在不丢失任何既有功能、视觉质感、中文交互与画手外置素材兼容性的前提下，根除 WebView2 7 进程 / 420+ MiB 内存与 24fps 全画布 `getImageData` 命中掩码瓶颈。

| 指标 | 旧 WebView2 混合版基线 | 纯原生 Rust 目标 |
| --- | --- | --- |
| 进程数 | 7（含 297 MiB GPU 进程） | **1**（单进程原生 EXE，0 GPU 进程） |
| 待机独占驻留内存（控制台关闭） | 501.6 MiB | **< 20 MiB** |
| 待机独占驻留内存（控制台打开） | 419.6 MiB | **< 35 MiB** |
| 桌宠每帧合成耗时（540×590 @ 24fps） | ~12–25 ms（含 `getImageData` + IPC） | **< 2.0 ms/帧** |
| 待机整机 CPU 占比 | 3.10% – 4.94% | **< 0.8%** |
| 透明穿透判定 | 1.22 MiB 全图读回 + 行矩形 JSON + 16ms 轮询 | **O(1) Win32 DWM 原生逐像素 Alpha 穿透** |
| 音频采集延迟与拷贝 | WASAPI → Vec → Base64 → `eval_script` → Worklet | **WASAPI → 无锁 SPSC RingBuf → 原生 DSP 线程（0 Base64 / 0 IPC）** |

---

## 2. 技术栈选型与架构论证

### 2.1 桌宠透明主窗口：`Win32 WS_EX_LAYERED + UpdateLayeredWindow + tiny-skia`

- **窗口机制**：直接调用 `windows-sys` 创建无边框、置顶、分层窗口（`WS_POPUP`，`WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED`），开启 `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2`。
- **呈现方式**：使用持久化 32-bit Top-Down `DIBSection`（预乘 BGRA8）配合 `UpdateLayeredWindow(..., ULW_ALPHA)` + `BLENDFUNCTION { AC_SRC_OVER, AC_SRC_ALPHA }` 提交每帧像素。
- **为什么选择此方案**：
  1. **原生 O(1) 逐像素 Alpha 命中测试**：Windows DWM 根据 `UpdateLayeredWindow` 提交的 Alpha 通道自动将 `alpha == 0` 的透明像素透传给下层桌面窗口，同时保留半透明柔边与内部透明空隙——彻底删除历史 `pet-hit-region.js`（曾占 42.8% CPU）与 `SetWindowRgn`。
  2. **零白边 / 零锯齿**：`tiny-skia` 内部采用 Premultiplied RGBA8 合成，提交前做一次紧循环 R↔B 通道交换（540×590 约 0.15ms）即可直接满足 Win32 `AC_SRC_ALPHA` 预乘 BGRA 约定。
  3. **极简锁定穿透**：锁定桌宠只需给 `GWL_EXSTYLE` 增删 `WS_EX_TRANSPARENT`，毫秒级生效。
  4. **零 GPU 进程**：完全在 CPU 紧致边界盒上完成合成，避免加载 D3D12/Vulkan/OpenGL 驱动 DLL（省去 40–300 MiB 显存与私有工作集）。
- **紧致边界盒裁剪（Tight-BBox）与静态层缓存**：
  - 历史版本 31 张 800×800 PNG 完整解码占 **75.7 MiB**；本架构在 `pet-core` 加载解码阶段即扫描非零 Alpha 紧致包围盒 `(offset_x, offset_y, width, height)` 并转为预乘 RGBA8，总素材常驻内存压缩至 **~8–14 MiB**，单帧仅绘制有效像素。
  - 将静止图层按前后景预合成两张缓存（`static_back`: 椅背；`static_front`: 桌面 + 电脑开关态 + 书本 + 休闲/自习手臂），仅在活动模式、DPI 或重载素材时失效重建。每帧仅动态合成：节拍脉冲环 → `static_back` → 身体呼吸层 → 头部统一刚体组（或整体动画帧）→ `static_front` → 计时器呼吸光效 → 配件叠加层 → 音符粒子 → 顶部悬浮工具栏按钮（`打开控制台` / `锁定`）。

### 2.2 独立浮动控制台窗口：`egui`（无头布局引擎）+ 自研纯 Rust 软件光栅化器 + 原生 Win32 `HWND`

在评估纯 Rust GUI 方案时，对比了 `eframe`（`wgpu`/`glow`）、原生 Win32 控件与 **`egui` + CPU 软件光栅化 + Win32 `DIBSection/BitBlt`**：

| 维度 | `eframe` (`wgpu`/`glow`) | 纯 Win32 控件 | **`egui` + CPU 软件光栅化 + Win32 `HWND`（选用）** |
| --- | --- | --- | --- |
| GPU 驱动内存开销 | +40 ~ 120 MiB | 0 MiB | **0 MiB** |
| 二次元糖果主题定制能力 | 极强 | 极差（需大量 Owner-Draw） | **极强（圆角卡片、柔和阴影、渐变、条纹进度条）** |
| 事件循环协同 | 强占主线程，难以与 `UpdateLayeredWindow` 共存 | 原生统一 | **原生统一（与桌宠共享同一 Win32 消息循环）** |
| 关闭控制台释放内存 | 难以彻底销毁 GL/WGPU 上下文 | 容易 | **关闭即销毁 `HWND` 与 DIB 缓冲，UI 表面内存归零，后台状态由 `pet-core` 保持** |
| 中文与 IME 支持 | 需配置字体 | 原生 | **加载 `C:\Windows\Fonts\msyh.ttc` + Win32 `WM_CHAR`/`WM_IME_CHAR` 完整支持中文输入与高 DPI** |
| 静态待机 CPU | 依赖驱动 VSync | 0% | **按需事件驱动重绘（静默时 0.0% CPU；计时/音频开启时 5–10Hz 轻量刷新）** |

因此 `crates/pet-ui` 采用 **`egui` 0.31（纯布局与控件计算）+ 纯 Rust 软件三角形光栅化器（支持纹理图集采样、顶点着色、裁剪矩形、sRGB 预乘混合，直接输出 BGRA DIB）+ Win32 无边框置顶 `HWND`**。支持：
- 4 个完整中文标签页：「状态」「情绪」「音频同步」「设置」；
- 顶部自定义二次元标题栏（可拖动、含「重载素材」「关闭」按钮）；
- 待办抽屉（`TodoDrawer`）及全部 4 类中文确认弹窗（暂停确认、离开自习确认、继续专注鼓励、保存清单确认）；
- 拖放音频文件到控制台或桌宠均可直接播放；
- 键盘快捷键（未聚焦输入框时 `Space` 开始/停止、`Q` 停止、`H` 隐藏控制台）。

### 2.3 音频捕获与节拍检测：`wasapi` + `ringbuf` + `rustfft` + `symphonia`

- **系统音频回环**：`wasapi`（0.17）直接打开 Windows 默认渲染端点（或指定端点）的共享模式 Loopback 捕获，采样写入 `ringbuf` 无锁 SPSC 环形缓冲，由独立高优先级 DSP 线程消费。
- **2048 点 Blackman FFT**：`rustfft` 计算 2048 点复数 FFT，复刻 Web Audio `AnalyserNode` 参数（`smoothingTimeConstant = 0.08`、`minDecibels = -90.0`、`maxDecibels = -3.0`、量化 `0..=255`），保证与原 `x1` 节拍检测器数值对齐。
- **节拍检测器与灵敏度修复**：完整移植 `x1`（16 对数频带 40Hz–12kHz、10ms ODF 环、12s/4s 加权自相关、相位折叠覆盖率、频谱周期性、110 BPM 对数正态先验、打击乐模式、节拍/倍半速/低频锚定修正、92/8 PLL 与 9 点中值 BPM 防抖），并真正接入 `sensitivity`（`1.05..=2.0`，基准 `1.22`）动态调节通量门限、起音阈值与静音门限。
- **本地音乐与完成提示音**：`symphonia` 解码拖入的 MP3/WAV/FLAC/OGG/AAC/M4A 循环播放，同时送入 WASAPI 渲染输出与 FFT 管线；倒计时结束提示音由原生三音正弦合成器（`660 / 880 / 1046 Hz`，间隔 220ms，20ms 线性起音 + 380ms 指数衰减）通过 WASAPI 渲染播放。

### 2.4 系统集成与持久化

- **持久化目录**：`%LOCALAPPDATA%/YumeRhythm/state.json`（支持环境变量 `YUME_DATA_DIR` 覆盖，用于自动化隔离测试），原子写盘（`.tmp` + 重命名）。
- **系统托盘与全局热键**：`tray-icon` + `global-hotkey`（`Ctrl+Alt+C` 打开控制台、`Ctrl+Alt+L` 锁定/解锁），托盘右键菜单包含「打开控制台」「锁定桌宠（点击穿透）/ 解除桌宠锁定」「退出桌宠节奏师」，双击托盘打开控制台。
- **素材加载与热重载**：默认素材通过 `include_dir` 内嵌在二进制内，零外部依赖即可单文件运行；若可执行文件同级存在 `character_assets/`（或环境变量 `YUME_ASSET_DIR`），优先热加载外置素材并严格校验相对路径（拒绝路径穿越），失败时输出中文错误并保留上一份已成功加载的素材。

---

## 3. Cargo Workspace 模块边界与并行协作契约

```text
Cargo.toml (workspace)
assets/character/                  # 内嵌默认角色素材（manifest.json, expressions.json, 31 PNGs）
crates/
├── pet-core/                      # 共享领域模型、状态机、持久化、计时、素材紧致加载、动画物理
│   └── src/
│       ├── lib.rs
│       ├── clock.rs               # monotonic_ms() 统一单调时钟 & wall_ms()
│       ├── audio_contract.rs      # AudioMode, AudioDeviceInfo, AudioTelemetry, AudioCommand
│       ├── settings.rs            # GlobalSettings, EventSwitchMatrix, TodoList, PersistedState
│       ├── storage.rs             # %LOCALAPPDATA%/YumeRhythm/state.json 原子读写
│       ├── timer.rs               # StudyTimer（倒计时/正计时墙钟锚点）
│       ├── manifest.rs            # CharacterManifest 解析校验
│       ├── expressions.rs         # ExpressionDefinition 解析校验 & 旧版兼容回退
│       ├── assets.rs              # TightSprite / AnimatedSprite 裁剪预乘加载器 + 内嵌默认包
│       ├── animation.rs           # m1()、弹簧阻尼点头物理、眨眼控制器、音符粒子
│       └── state.rs               # AppState、AppAction、AppEffect 统一状态机
├── pet-audio/                     # 音频引擎（归属 @audio-eng）
│   └── src/lib.rs                 # AudioEngine::spawn() -> AudioHandle
├── pet-render/                    # 桌宠透明分层窗口与 tiny-skia 合成器（归属 @render-eng）
│   └── src/lib.rs                 # PetWindow, PetCompositor, PetWindowEvent
├── pet-ui/                        # egui 软件光栅化浮动控制台（归属 @ui-eng）
│   └── src/lib.rs                 # ConsoleWindow, ConsoleEvent
└── pet-app/                       # 主程序 yume-rhythm.exe：Win32 消息循环、托盘、热键、--bench/--self-test
    └── src/main.rs
```

### 跨 Crate 接口契约（冻结边界）

1. **`pet-core` → `pet-audio`**：
   - `pet_core::clock::monotonic_ms() -> f64`：所有时间戳（`next_beat_at_ms`、`m1`、`animation::step`）共享同一进程级 `OnceLock<Instant>` 基准。
   - `pet_core::audio_contract::{AudioMode, AudioDeviceInfo, AudioTelemetry, AudioCommand, DEFAULT_SENSITIVITY, MIN_SENSITIVITY, MAX_SENSITIVITY, is_supported_audio_file}`。
   - `pet_audio::AudioEngineHandle`：提供 `spawn()`, `send(AudioCommand)`, `latest_telemetry() -> AudioTelemetry`, `devices() -> Vec<AudioDeviceInfo>`。
2. **`pet-core` → `pet-render`**：
   - `pet_core::assets::{LoadedCharacterAssets, TightSprite, AnimatedSprite}`。
   - `pet_core::animation::{CharacterAnimator, AnimationPose, NoteParticleRender}`。
   - `pet_core::state::{ActivityMode, ResolvedFaceLayers}`。
   - `pet_render::PetRenderInput`（纯值快照，避免渲染层持有应用锁）与 `pet_render::PetWindowEvent`（点击打开控制台、点击锁定、拖拽、拖入音频文件、按键、关闭）。
3. **`pet-core` → `pet-ui`**：
   - `pet_ui::ConsoleWindow` 接收 `&mut AppState`（或状态快照）并输出 `Vec<AppAction>`，由 `pet-app` 统一经 `AppState::dispatch()` 执行并处理 `AppEffect`（音频、持久化、窗口显隐、锁定、退出）。
   - 关闭控制台时 `ConsoleWindow::hide_and_release()` 销毁 `HWND` 与像素缓冲；重新打开时 `ConsoleWindow::show_near_pet(pet_rect)` 按工作区安全坐标重建。

---

## 4. 构建环境与验证计划

- **工具链**：Rust 1.88+（本机已验证 `rustc 1.100.0` / `cargo 1.100.0`，MSVC 14.42 + Windows SDK 10.0.22621）。
- **内置诊断模式**（`yume-rhythm.exe`）：
  - `--self-test`：自动化校验素材加载与紧致裁剪内存占用、表情校验与旧版回退、双向计时与暂停恢复、随机事件矩阵（含音乐期禁打瞌睡）、持久化读写、以及离线 120 BPM 合成信号节拍锁定精度。
  - `--bench`：连续合成 240 帧（10 秒 @ 24fps）统计 `tiny-skia` 每帧 P50/P95/Max 耗时、素材解码内存、当前进程私有工作集（Private Working Set）并输出 JSON 报告。
- **端到端桌面验收**：实机启动单进程 `yume-rhythm.exe`，验证透明逐像素穿透、锁定穿透、开关控制台内存回收、中文 4 标签页交互、系统音频同步与拖入音频播放。

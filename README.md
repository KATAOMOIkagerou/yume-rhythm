# 桌宠节奏师（Desktop Pet Rhythm）

一款基于 **Electron + React 19 + TypeScript + Vite + Tailwind CSS v4** 构建的二次元桌前人物音乐律动与自习陪伴桌面应用。

主角「**二次元学霸少女·樱音**」坐在书桌前陪伴你的日常学习与听歌时光：播放音乐时戴上耳机随真实节拍精准点头律动、飘出音符粒子；切换自习模式时自动点亮桌面电脑、进入握笔写字姿势并展示呼吸发光计时闹钟；所有人物图层均以独立透明 PNG + `manifest.json` 外部化配置驱动，支持画手热替换与即点重载。

---

## 核心特性与音频冻结保证

### 1. 音频识别核心绝对冻结（Zero-Modification Guarantee）

- 音频捕获、频谱能量通量、节拍检测器 `x1`、合成演示鼓机 `A1`、音频引擎控制器 `E1`、初始帧常量 `jh` 以及节拍锁定点头函数 `m1`（重导出为 `computeLockedBeatNod`）均直接从只读基线包原样字节提取，封装在 [`src/audio/frozen-audio-core.js`](src/audio/frozen-audio-core.js) 中。
- Electron 主进程 [`electron/main.cjs`](electron/main.cjs) 中的 `configureMediaPermissions()` 权限与系统音频回环捕获处理器同样保持逐字节一致。
- `src/audio/` 模块与 GUI 及人物组件完全解耦（不引入任何 React / 界面 / 人物依赖），可随时运行 `npm run verify:audio` 与只读基线 `app.asar` 进行长度及 SHA-256 逐字节核对。

### 2. 已落地功能清单

| 模块 | 已落地功能 |
|---|---|
| **二次元分层人物舞台** | - 32 张 `800×800` RGBA 分层 PNG（默认角色包：`二次元学霸少女·樱音`），采用 `headRig` 单一绑定组同步绕支点 `(400, 452)` 点头/歪头/弹性缩放，纯净无五官 `head_base.png` 配合互斥 `eyes_*` / `mouth_*` 渲染，零叠影、零断脖。<br>- 桌面场景联动：休闲（`chill`）电脑待机/放松双手 ↔ 自习（`study`）电脑亮屏/握笔学习；计时中显示桌面小闹钟；听歌时佩戴耳机并伴随音符粒子（`♪ ♫ ♩ ♬ ✦`）与节拍扩散光环。<br>- 自然眨眼（2.4s~5.2s 触发 150ms 闭眼）与随机事件（`doze` 打瞌睡 / `think` 思考 / `shock` 震惊 / `love` 喜欢，**音乐运行期间强制禁止并即时清除打瞌睡**）。<br>- 支持窗口内自由拖拽人物位置，支持通过右上角 **`🔄 重载素材`** 热更新画手新图。 |
| **Status 状态页 & 待办清单** | - `Study (自习)` / `Chill (休闲)` 陪伴模式切换。<br>- `Set time for [ 25 ] min`（1~999 分钟）自习倒计时控制与开始/暂停按钮。<br>- 倒计时中点击暂停或试图切换回休闲模式时，依次触发督促确认弹窗（`Time's not up yet! Do you still want to pause?` → 选 `No` 弹出 `Go back to study!`；选 `Yes` 进入 **`pausing...` 斜纹进度条**状态并完整保留剩余时间 `remainingMs`，点 `continue` 恢复）。<br>- 可折叠 **`⚙ 随机动作开关`** 矩阵（`chill` / `study` / `music` × 四大事件，支持 `localStorage` 持久化，其中 `music × doze` 强制禁用锁定）。<br>- 连接在主窗下方的 **`to do list / 待办清单`** 抽屉：支持清单标题、勾选框、增删行及保存确认弹窗（`Save list?`），持久化至 `localStorage`。 |
| **Emotion 情绪页** | - 提供 `Happy (开心)` 与 `Sad (难过)` 表情切换（再次点击当前已勾选项恢复默认 `Neutral` 自然神态）。 |
| **Metronome 节拍器页** | - 完整接入冻结音频引擎 `E1` 与四大音源：**🖥️ 电脑音频**（系统回环）、**🎵 本地音乐**（按钮选文件 + 窗口直接拖放音频文件）、**🎤 麦克风**、**🥁 演示鼓点**。<br>- 实时显示状态、大号 `BPM`、`🔒 节拍已锁定` / `🔍 搜索节拍…` 徽章、节拍闪光点、24 段电平条、`♪ N 拍`、低频与音量百分比、灵敏度滑块（1.05~2.00）。<br>- 提供 `Auto check BPM` 与 `Auto follow` 开关，支持 `Q` 急停、`Space` 启停音频、`H`（或 `[—]` 按钮 / 双击舞台）收起展开控制面板。 |

### 3. 草图规划待实现占位（音频冻结期未启用）

在 **Metronome 节拍器页**底部保留了草图构想的独立禁用占位模块，均明确标注 `待后续独立模块接入（音频冻结期未启用）`，当前版本严禁修改或伪造对应功能：

- `Set BPM for [ ___ ] [ ▶ ]`（手动设定固定 BPM 并启动跟拍）—— `disabled`
- `Tap to follow (手敲跟拍)`（手动按拍点驱动点头）—— `disabled`
- `Tap to check BPM (手敲测速)`（手动敲击测算 BPM）—— `disabled`

---

## 项目目录结构

```text
d:\antigravity_ideas\
├── .npmrc                               # 国内 npm & Electron 镜像配置
├── index.html                           # 应用入口 HTML（标题：桌宠节奏师）
├── package.json                         # 依赖与开发/构建/校验/打包脚本
├── tsconfig.json                        # TypeScript 严格模式配置
├── vite.config.ts                       # Vite + React + Tailwind v4 配置（base: './'）
├── electron/
│   ├── main.cjs                         # Electron 主进程（含冻结媒体权限与 pet-asset:// 外部素材协议）
│   └── preload.cjs                      # 只读上下文桥接 window.desktopPetBridge
├── public/
│   └── character_assets/                # 默认二次元人物 32 张 PNG 图层与 manifest.json
├── scripts/
│   ├── extract-and-freeze-audio.mjs     # 从基线 app.asar 字节级提取冻结音频核心
│   ├── verify-audio-frozen.mjs          # 校验冻结音频与权限函数 100% 字节/SHA-256 一致
│   ├── generate-anime-assets.mjs        # 使用 @resvg/resvg-js 生成默认 32 张 800×800 RGBA PNG
│   ├── verify-electron-e2e.mjs          # 真实 Electron 窗口 E2E 自动化验证脚本
│   └── build-share-package.mjs          # 构建 Windows x64 便携分享目录与 zip 包
├── src/
│   ├── App.tsx                          # 主控布局、快捷键、计时状态机、本地存储与舞台交互
│   ├── main.tsx                         # React 挂载入口
│   ├── styles.css                       # 二次元粉彩主题样式与斜纹进度条动效
│   ├── types/contracts.ts               # 全局共享 TS 类型与接口契约
│   ├── audio/                           # 冻结音频内核与 TS 类型封装（严禁引用 GUI/人物）
│   │   ├── frozen-audio-core.js         # 原样提取的 m1, x1, A1, E1, jh
│   │   ├── frozen-audio-core.d.ts
│   │   └── index.ts
│   ├── character/                       # 人物资源加载、事件状态机与跟拍动画渲染
│   │   ├── assetLoader.ts               # 清单加载、协议路径解析与全图层预加载
│   │   ├── useCharacterEvents.ts        # 自然眨眼与随机事件开关矩阵（含音乐禁打瞌睡规则）
│   │   ├── DeskCharacterScene.tsx       # 9 层 Z 轴叠层、headRig 绑定与 v1/m1 弹簧点头循环
│   │   └── index.ts
│   └── gui/                             # 二次元面板、页签、弹窗与抽屉组件
│       ├── Icons.tsx
│       ├── StatusTab.tsx
│       ├── EmotionTab.tsx
│       ├── MetronomeTab.tsx
│       ├── TodoDrawer.tsx
│       └── Modals.tsx
└── docs/
    ├── 画手素材替换说明.md               # 32 张图层规范、防重影/防断脖规则、开发/分享包热替换指南
    └── 倍半速变化方案评估分析.md         # 94 ↔ 188/187 BPM 倍半速跳变的技术可行性与风险评估
```

---

## 快速开始与常用命令

### 1. 安装依赖

```powershell
npm install
```

（工程已通过 `.npmrc` 配置国内 `npmmirror` 与 Electron 镜像；如需临时覆盖可在 PowerShell 设置 `$env:ELECTRON_MIRROR="https://npmmirror.com/mirrors/electron/"`。）

### 2. 开发模式运行

推荐在两个终端分别启动 Vite 前端热更新与 Electron 桌面窗口（或先构建再直接启动 Electron）：

```powershell
# 终端 1：启动 Vite 开发服务器 (http://127.0.0.1:5173)
npm run dev:vite

# 终端 2：启动 Electron 桌面客户端
npm start
```

### 3. 音频零修改校验（每次交付必跑）

```powershell
npm run verify:audio
```

该脚本会对照基线 `app.asar`，校验 `m1`（251 bytes）、`audio-engine`（21452 UTF-8 bytes）和 `configureMediaPermissions()`（945 bytes）的字节长度与 SHA-256 哈希，并检查 `src/audio/` 无任何 GUI/人物/React 依赖；全部一致才返回退出码 `0`。

### 4. 重新生成默认人物素材与类型构建

```powershell
# 如修改 scripts/generate-anime-assets.mjs 后重新生成 32 张默认 PNG
npm run gen:assets

# TypeScript 类型检查 + Vite 生产打包输出到 dist/
npm run build
```

### 5. 打包 Windows x64 分享版

```powershell
npm run build
npm run package:win
```

执行后将在 `release/` 下生成干净便携的 Windows 桌面目录与压缩包（`release/桌宠节奏师-Windows-x64-分享版/桌宠节奏师/桌宠节奏师.exe` 与对应 `.zip`），并附带独立外置的 `character_assets/` 目录与画手替换说明。

---

## 扩展阅读文档

- [画手素材替换说明](docs/画手素材替换说明.md)：面向美术合作方的 32 张图层全清单、800×800 RGBA 规范、无五官底图防叠影原理、颈部 30px+ 防露缝标准，以及**开发态 / 打包分享版无需重打包直接覆盖热重载**教程。
- [倍半速变化方案评估分析](docs/倍半速变化方案评估分析.md)：结合冻结音频核心 `x1` 源码对“94 BPM ↔ ~188/187 BPM 倍半速忽略与滞回方案”的纯技术可行性、分层设计与四大边界风险分析。

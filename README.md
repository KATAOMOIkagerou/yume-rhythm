# 桌宠节奏师

基于 Electron、React、TypeScript 和 Vite 的透明桌面伴侣，支持音乐跟拍、自习计时、配置式表情和待办清单。

## 运行

```powershell
npm ci
npm run build
npm start
```

开发模式：一个终端运行 `npm run dev:vite`，另一个运行 `npm start`。
如果依赖安装策略阻止 Electron 的安装脚本，需按本机策略批准 Electron 后再安装运行程序。项目使用的镜像可通过 `ELECTRON_MIRROR` 环境变量指定。

## 功能

- 桌宠采用透明无边框窗口，拖动角色区域可以移动整个桌宠窗口。
- 右侧控制台现在是独立可拖动窗口，包含「状态」「情绪」「音频同步」「设置」四个中文标签页。关闭控制台只隐藏窗口，计时与音频继续运行。
- 锁定时禁止拖动并启用点击穿透；可从控制台、系统托盘菜单或快捷键解除锁定。
- 状态页支持倒计时和正计时，暂停保留毫秒精度；倒计时完成可播放提示音，设置页可关闭。
- 情绪页由外置 `expressions.json` 自动生成选项；支持整体动画、眼嘴分层和配件叠加，默认每 120 秒随机播放一次。
- 配件保留已选基础表情；再次点击当前选项恢复自然表情。随机播放结束恢复手动所选状态。
- 音频同步保留系统音频监听、自动测速和自动跟拍；灵敏度位于高级设置中。移除了本地音乐、麦克风、演示鼓点按钮。拖入音频文件的原有路径仍可使用。
- 监听设备下拉框展示系统默认输出及可枚举的设备。当前 Electron 系统回环接口只支持默认输出；其他端点明确标为不可直接选择，可通过「打开系统声音设置」更换默认设备后重新监听。
- 待办清单和全局设置保存在本地。音乐播放期间仍强制禁止打瞌睡。

## 快捷键

| 快捷键 | 操作 |
|---|---|
| 空格键 | 开始／停止音频 |
| Q | 停止音频 |
| H | 隐藏控制台 |
| Ctrl＋Alt＋C | 全局打开控制台 |
| Ctrl＋Alt＋L | 全局锁定／解锁桌宠 |

普通快捷键在桌宠或控制台获得焦点、且未编辑输入框时生效。若全局快捷键被其他程序占用，仍可使用托盘菜单。退出应用可使用设置页或托盘菜单。

## 校验与打包

```powershell
npm run build
npm run verify:features
npm run verify:desktop
npm run package:win
```

- `verify:features` 验证计时公式、暂停续计、表情配置与素材、中文界面文本、提示音生成，以及音频冻结代码和媒体权限逻辑与当前仓库基准一致。
- `verify:desktop` 启动实际双窗口，验证锁定解锁、配置扩展、表情切换和随机恢复、后台计时、音效开关、待办保存、音频节拍输出与快捷键，并在 `.tmp/` 保存截图。提示音触发使用模拟音频接口验证；真实音频文件仍经过原始检测器。
- `verify:audio` 继续保留历史安装包逐字节校验，需要通过 `BASELINE_ASAR_PATH` 提供历史 `app.asar`；没有该文件时不能执行此项。
- `verify-electron-e2e.mjs` 是旧版单窗口界面脚本，仅作历史保留；本次重构使用 `verify:desktop`。
- `src/audio/frozen-audio-core.js` 与主进程 `configureMediaPermissions()` 的业务逻辑保持不变。

## 源码入口

- `electron/main.cjs` / `preload.cjs`：窗口、锁定、托盘、快捷键和受限通信接口。
- `src/App.tsx`：共享音频实例、计时状态、表情选择和随机调度。
- `src/gui/FloatingConsole.tsx`：将控制台挂载到独立原生窗口，同时共享 React 状态与上下文。
- `src/state/SettingsContext.tsx`：全局设置与本地持久化。
- `src/state/timer.ts`：双向计时公式与完成提示音。
- `src/character/expressions.ts` / `assetLoader.ts`：表情配置校验和资源加载。

[逐模块源码修改说明](docs/源码重构说明.md) · [画手素材替换说明](docs/画手素材替换说明.md)

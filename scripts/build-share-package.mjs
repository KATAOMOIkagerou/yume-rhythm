import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createPackage, listPackage } from '@electron/asar';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = path.resolve(__dirname, '..');

const SHARE_ROOT_NAME = '桌宠节奏师-Windows-x64-分享版';
const APP_FOLDER_NAME = '桌宠节奏师';
const EXE_NAME = '桌宠节奏师.exe';

const RELEASE_DIR = path.join(PROJECT_ROOT, 'release');
const OUT_SHARE_DIR = path.join(RELEASE_DIR, SHARE_ROOT_NAME);
const OUT_APP_DIR = path.join(OUT_SHARE_DIR, APP_FOLDER_NAME);
const OUT_ZIP_PATH = path.join(RELEASE_DIR, `${SHARE_ROOT_NAME}.zip`);

const ELECTRON_DIST_DIR = path.join(PROJECT_ROOT, 'node_modules', 'electron', 'dist');
const DIST_DIR = path.join(PROJECT_ROOT, 'dist');
const ASSETS_DIR = path.join(PROJECT_ROOT, 'public', 'character_assets');
const ARTIST_DOC_PATH = path.join(PROJECT_ROOT, 'docs', '画手素材替换说明.md');

function assertExists(targetPath, label) {
  if (!fs.existsSync(targetPath)) {
    throw new Error(`Missing required ${label}: ${targetPath}`);
  }
}

function copyDirSync(srcDir, dstDir, filter) {
  fs.mkdirSync(dstDir, { recursive: true });
  for (const entry of fs.readdirSync(srcDir, { withFileTypes: true })) {
    const srcPath = path.join(srcDir, entry.name);
    const dstPath = path.join(dstDir, entry.name);
    if (filter && !filter(srcPath, entry)) continue;
    if (entry.isDirectory()) {
      copyDirSync(srcPath, dstPath, filter);
    } else if (entry.isFile()) {
      fs.copyFileSync(srcPath, dstPath);
    }
  }
}

function shouldCopyElectronRootFile(name) {
  if (name === 'electron.exe') return false;
  if (name === 'default_app.asar') return false;
  if (name === 'version') return true;
  if (/^LICENSE/i.test(name)) return true;
  if (name === 'vk_swiftshader_icd.json') return true;
  return /\.(dll|pak|dat|bin)$/i.test(name);
}

function buildUserGuideText(version) {
  const buildDate = new Date().toISOString().slice(0, 10);
  const lines = [
    '桌宠节奏师 · Windows x64 体验分享版（二次元桌面伴侣）',
    `版本：v${version} | 打包日期：${buildDate}`,
    '适用环境：Windows 10 / 11（64 位）',
    '',
    '一、运行方式（免安装环境）',
    '1. 请先将压缩包完整解压到一个本地文件夹中（不要在压缩包内直接双击）。',
    `2. 双击运行「${EXE_NAME}」。`,
    '3. 无需安装 Node.js、Python 或额外运行库。',
    '4. 请务必保留同目录中的 resources、character_assets、locales、*.dll、*.pak、*.dat、*.bin 等文件，不要只单独复制 exe 文件。',
    '',
    '二、角色与三大功能页签说明',
    '本版本已全新重制为可分层替换的二次元桌前少女角色，支持自由切换三大功能页签：',
    '1. Status 状态页：',
    '   - 支持切换「Study (自习)」与「Chill (休闲)」模式：自习时桌面电脑亮屏发光、人物切换握笔书写姿势并出现桌面小闹钟。',
    '   - 支持设置自习时长（1~999 分钟）并开始/暂停倒计时；未到时间点击暂停或切回休闲模式会弹出贴心确认提示，确认暂停后保留剩余时间并显示 pausing... 斜纹进度条，点击 continue 可继续倒计时。',
    '   - 右下角「to do list / 待办清单」可展开填写、勾选任务并保存到本地。',
    '   - 「随机动作开关」可按休闲/专注/音乐三种情境控制打瞌睡、思考、震惊、喜欢等表情事件（音乐模式下强制禁止打瞌睡）。',
    '2. Emotion 情绪页：',
    '   - 可切换「Happy (开心)」与「Sad (难过)」表情（再次点击当前已选情绪可恢复自然表情），眼睛与嘴巴互斥联动，不产生重影。',
    '3. Metronome 节拍器页（四大音源 + 实时跟拍）：',
    '   - 电脑音频：采集当前系统正在播放的音乐（在共享弹窗中勾选音频即可）。',
    '   - 本地音乐：点击选择音频文件，或直接将音频文件拖放进软件窗口即可播放与测速跟拍。',
    '   - 麦克风：通过麦克风收听外放音乐。',
    '   - 演示鼓点：内置合成鼓机，无需外部音乐即可体验随拍点头与耳机/音符动效。',
    '   - 注：手动设定 BPM、手敲跟拍与手敲测速栏位当前为未启用占位态（本版本保持原版音频识别核心零改动）。',
    '',
    '三、常用快捷键（输入框未聚焦时生效）',
    '- Q：立即停止当前音频播放与音乐跟拍',
    '- Space（空格）：开始 / 停止音乐跟拍',
    '- H：收起 / 展开右侧控制面板（也可双击人物舞台切换）',
    '- 鼠标按住人物舞台拖拽：在窗口内自由移动人物位置',
    '',
    '四、画手素材热替换说明',
    '1. 打开本软件根目录下的「character_assets」文件夹。',
    '2. 直接覆盖同名 800×800 透明背景 32-bit PNG 文件，或修改该文件夹中的 manifest.json。',
    `3. 点击软件界面右上角「重载素材」按钮（或重启 ${EXE_NAME}）即可立即生效，无需重新打包或安装开发工具。`,
    '4. 详细图层顺序、头部支点与导出规范请查阅同目录下的「画手素材替换说明.md」。',
    '',
    '五、本分享版干净声明',
    '- 仅包含软件运行所需 Electron / Chromium 组件、应用包与角色素材，不含测试音频、截图、构建缓存、源码或开发依赖。',
    '- 本分享版尚未进行数字代码签名，首次运行时 Windows Defender SmartScreen 如弹出未知发布者提示，选择“更多信息 -> 仍要运行”即可。',
  ];
  return lines.join('\r\n') + '\r\n';
}

function collectAllFiles(dirPath, prefix = '') {
  const results = [];
  for (const entry of fs.readdirSync(dirPath, { withFileTypes: true })) {
    const rel = prefix ? `${prefix}/${entry.name}` : entry.name;
    const full = path.join(dirPath, entry.name);
    if (entry.isDirectory()) {
      results.push(...collectAllFiles(full, rel));
    } else {
      results.push({ rel, full, bytes: fs.statSync(full).size });
    }
  }
  return results;
}

function verifyPackageCleanliness(outAppDir, asarPath) {
  const forbiddenNames = [
    /DemoSong/i,
    /\.mp3$/i,
    /\.wav$/i,
    /\.ogg$/i,
    /\.flac$/i,
    /electron_shots/i,
    /\.ts$/i,
    /\.tsx$/i,
    /\.map$/i,
    /(^|\/)src(\/|$)/i,
    /(^|\/)scripts(\/|$)/i,
    /(^|\/)node_modules(\/|$)/i,
  ];

  const diskFiles = collectAllFiles(outAppDir);
  for (const file of diskFiles) {
    for (const pat of forbiddenNames) {
      if (pat.test(file.rel)) {
        throw new Error(`Forbidden file found in release directory: ${file.rel}`);
      }
    }
  }

  const asarEntries = listPackage(asarPath);
  for (const entry of asarEntries) {
    const normalized = entry.replace(/\\/g, '/').replace(/^\/+/, '');
    for (const pat of forbiddenNames) {
      if (pat.test(normalized)) {
        throw new Error(`Forbidden entry found inside app.asar: ${normalized}`);
      }
    }
  }

  const guideText = fs.readFileSync(path.join(outAppDir, '使用说明.txt'), 'utf8');
  const artistDocText = fs.readFileSync(path.join(outAppDir, '画手素材替换说明.md'), 'utf8');
  for (const text of [guideText, artistDocText]) {
    if (/桌宠节奏喵|小猫|猫耳/.test(text)) {
      throw new Error('Forbidden legacy cat terminology found in release documentation.');
    }
  }

  return { diskFiles, asarEntries };
}

function createZipArchive(sourceParentDir, folderToZip, outZipPath) {
  const pyScript = `
import os, sys, zipfile
parent_dir, folder_name, zip_path = sys.argv[1], sys.argv[2], sys.argv[3]
target_dir = os.path.join(parent_dir, folder_name)
with zipfile.ZipFile(zip_path, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=6) as zf:
    for root, dirs, files in os.walk(target_dir):
        dirs.sort()
        files.sort()
        for fn in files:
            full = os.path.join(root, fn)
            rel = os.path.relpath(full, parent_dir).replace('\\\\', '/')
            zf.write(full, rel)
print('ZIP_OK', zip_path)
`;
  execFileSync('python', ['-c', pyScript, sourceParentDir, folderToZip, outZipPath], {
    stdio: 'inherit',
  });
}

async function main() {
  assertExists(path.join(DIST_DIR, 'index.html'), 'built dist/index.html (run "npm run build" first)');
  assertExists(path.join(ELECTRON_DIST_DIR, 'electron.exe'), 'Electron runtime executable');
  assertExists(path.join(PROJECT_ROOT, 'electron', 'main.cjs'), 'electron/main.cjs');
  assertExists(path.join(PROJECT_ROOT, 'electron', 'preload.cjs'), 'electron/preload.cjs');
  assertExists(path.join(ASSETS_DIR, 'manifest.json'), 'public/character_assets/manifest.json');
  assertExists(ARTIST_DOC_PATH, 'docs/画手素材替换说明.md');

  const rootPkg = JSON.parse(fs.readFileSync(path.join(PROJECT_ROOT, 'package.json'), 'utf8'));
  const runtimePkg = {
    name: rootPkg.name || 'desktop-pet-rhythm',
    version: rootPkg.version || '1.0.0',
    private: true,
    type: 'module',
    main: 'electron/main.cjs',
  };

  console.log('Cleaning previous release outputs...');
  fs.rmSync(OUT_SHARE_DIR, { recursive: true, force: true });
  fs.rmSync(OUT_ZIP_PATH, { force: true });
  fs.mkdirSync(OUT_APP_DIR, { recursive: true });
  fs.mkdirSync(path.join(OUT_APP_DIR, 'resources'), { recursive: true });

  console.log('Copying Electron Windows x64 runtime...');
  for (const entry of fs.readdirSync(ELECTRON_DIST_DIR, { withFileTypes: true })) {
    const src = path.join(ELECTRON_DIST_DIR, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === 'locales') {
        copyDirSync(src, path.join(OUT_APP_DIR, 'locales'));
      }
      continue;
    }
    if (entry.name === 'electron.exe') {
      fs.copyFileSync(src, path.join(OUT_APP_DIR, EXE_NAME));
    } else if (shouldCopyElectronRootFile(entry.name)) {
      fs.copyFileSync(src, path.join(OUT_APP_DIR, entry.name));
    }
  }

  console.log('Packing runtime-only app.asar via @electron/asar...');
  const stagingRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'pet-asar-stage-'));
  const stagingDir = path.join(stagingRoot, 'pkg');
  const tempAsarPath = path.join(stagingRoot, 'app.asar');
  const asarPath = path.join(OUT_APP_DIR, 'resources', 'app.asar');
  try {
    fs.mkdirSync(stagingDir, { recursive: true });
    fs.writeFileSync(
      path.join(stagingDir, 'package.json'),
      JSON.stringify(runtimePkg, null, 2) + '\n',
      'utf8'
    );
    fs.mkdirSync(path.join(stagingDir, 'electron'), { recursive: true });
    fs.copyFileSync(
      path.join(PROJECT_ROOT, 'electron', 'main.cjs'),
      path.join(stagingDir, 'electron', 'main.cjs')
    );
    fs.copyFileSync(
      path.join(PROJECT_ROOT, 'electron', 'preload.cjs'),
      path.join(stagingDir, 'electron', 'preload.cjs')
    );
    copyDirSync(DIST_DIR, path.join(stagingDir, 'dist'), (srcPath) => !/\.map$/i.test(srcPath));

    await createPackage(stagingDir, tempAsarPath);
    fs.copyFileSync(tempAsarPath, asarPath);
  } finally {
    fs.rmSync(stagingRoot, { recursive: true, force: true });
  }

  console.log('Copying external character_assets & documentation...');
  copyDirSync(ASSETS_DIR, path.join(OUT_APP_DIR, 'character_assets'));
  fs.copyFileSync(ARTIST_DOC_PATH, path.join(OUT_APP_DIR, '画手素材替换说明.md'));
  fs.writeFileSync(
    path.join(OUT_APP_DIR, '使用说明.txt'),
    buildUserGuideText(runtimePkg.version),
    'utf8'
  );

  console.log('Verifying package cleanliness and app.asar contents...');
  const { diskFiles, asarEntries } = verifyPackageCleanliness(OUT_APP_DIR, asarPath);
  const totalDiskBytes = diskFiles.reduce((sum, f) => sum + f.bytes, 0);

  console.log('Creating ZIP archive:', OUT_ZIP_PATH);
  createZipArchive(OUT_SHARE_DIR, APP_FOLDER_NAME, OUT_ZIP_PATH);
  const zipBytes = fs.statSync(OUT_ZIP_PATH).size;

  console.log('=== Share Package Summary ===');
  console.log(`Directory : ${OUT_APP_DIR}`);
  console.log(`Files     : ${diskFiles.length} files (${(totalDiskBytes / (1024 * 1024)).toFixed(2)} MB uncompressed)`);
  console.log(`app.asar  : ${asarEntries.length} entries (${(fs.statSync(asarPath).size / 1024).toFixed(1)} KB)`);
  console.log(`ZIP file  : ${OUT_ZIP_PATH} (${(zipBytes / (1024 * 1024)).toFixed(2)} MB)`);
}

main().catch((err) => {
  console.error('[ERROR] build-share-package failed:', err);
  process.exit(1);
});

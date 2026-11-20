import fs from 'node:fs';
import path from 'node:path';
import { buildEnvironment, root, run } from './toolchain.mjs';

run(process.execPath, ['node_modules/typescript/bin/tsc', '--noEmit']);
run(process.execPath, ['node_modules/vite/bin/vite.js', 'build']);
const { env, cargo } = buildEnvironment();
run(cargo, ['build', '--manifest-path', 'native/Cargo.toml', '--release', '--locked'], env);
const bin = path.join(root, 'bin');
fs.mkdirSync(bin, { recursive: true });
fs.copyFileSync(path.join(root,'native/target/release/desktop-pet-rust.exe'), path.join(bin,'桌宠节奏师.exe'));
fs.cpSync(path.join(root,'public/character_assets'), path.join(bin,'character_assets'), { recursive:true });
for (const file of ['LICENSE','使用说明.txt']) fs.copyFileSync(path.join(root,file),path.join(bin,file));
console.log(`构建完成：${path.join(bin,'桌宠节奏师.exe')}\n整个 bin 文件夹可以单独复制运行。`);

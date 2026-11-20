import fs from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { root, run } from './toolchain.mjs';
const exe = path.join(root, 'bin', '桌宠节奏师.exe');
if (!fs.existsSync(exe)) run(process.execPath, ['scripts/build.mjs']);
const child = spawn(exe, [], { cwd:path.dirname(exe), detached:true, stdio:'ignore' });
child.on('error', error => { console.error(error.message); process.exitCode = 1; });
child.unref();

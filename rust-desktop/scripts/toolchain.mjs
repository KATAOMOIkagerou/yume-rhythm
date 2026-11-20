import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync, spawnSync } from 'node:child_process';

export const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
export function buildEnvironment() {
  const env = { ...process.env };
  const tools = path.join(root, '.tools');
  const cargo = path.join(tools, 'cargo', 'bin', 'cargo.exe');
  if (fs.existsSync(cargo)) {
    env.CARGO_HOME = path.join(tools, 'cargo');
    env.RUSTUP_HOME = path.join(tools, 'rustup');
    env.PATH = `${path.dirname(cargo)};${env.PATH}`;
  }
  // Discover an installed MSVC toolset, including machines with multiple VS versions.
  const programFiles = env['ProgramFiles(x86)'] || 'C:\\Program Files (x86)';
  const vswhere = path.join(programFiles, 'Microsoft Visual Studio', 'Installer', 'vswhere.exe');
  if (fs.existsSync(vswhere)) {
    const installations = JSON.parse(execFileSync(vswhere, ['-products', '*', '-requires', 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64', '-format', 'json'], { encoding: 'utf8' }));
    for (const installation of installations) {
      const msvc = path.join(installation.installationPath, 'VC', 'Tools', 'MSVC');
      if (!fs.existsSync(msvc)) continue;
      const versions = fs.readdirSync(msvc).sort((a,b) => b.localeCompare(a, undefined, { numeric:true }));
      const version = versions.find(v => fs.existsSync(path.join(msvc,v,'bin','Hostx64','x64','link.exe')));
      if (!version) continue;
      const vc = path.join(msvc, version);
      const sdk = path.join(programFiles, 'Windows Kits', '10');
      const sdkVersion = fs.readdirSync(path.join(sdk, 'Lib')).sort((a,b) => b.localeCompare(a, undefined, { numeric:true }))
        .find(v => fs.existsSync(path.join(sdk,'Lib',v,'um','x64','kernel32.lib')));
      if (!sdkVersion) throw new Error('请安装 Windows SDK（含桌面 C++ 库）。');
      env.CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = path.join(vc, 'bin', 'Hostx64', 'x64', 'link.exe');
      env.LIB = [path.join(vc,'lib','x64'),...['um','ucrt'].map(v=>path.join(sdk,'Lib',sdkVersion,v,'x64')),env.LIB].filter(Boolean).join(';');
      env.INCLUDE = [path.join(vc,'include'),...['um','ucrt','shared','winrt'].map(v=>path.join(sdk,'Include',sdkVersion,v)),env.INCLUDE].filter(Boolean).join(';');
      env.PATH = `${path.join(vc,'bin','Hostx64','x64')};${path.join(sdk,'bin',sdkVersion,'x64')};${env.PATH}`;
      break;
    }
  }
  return { env, cargo: fs.existsSync(cargo) ? cargo : 'cargo' };
}

export function run(command, args, env = process.env) {
  const result = spawnSync(command, args, { cwd: root, env, stdio: 'inherit', shell: false });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${path.basename(command)} 执行失败（${result.status}）`);
}

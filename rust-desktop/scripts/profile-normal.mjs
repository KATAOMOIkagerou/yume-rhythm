import fs from 'node:fs';import path from 'node:path';import {spawn,execFileSync} from 'node:child_process';import {root} from './toolchain.mjs';
const delay=ms=>new Promise(r=>setTimeout(r,ms)),dir=path.join(root,'verification',`normal-${Date.now()}`);fs.mkdirSync(dir,{recursive:true});
const env={...process.env,PET_DATA_DIR:path.join(dir,'profile')};delete env.PET_DEBUG_PORT;
const app=spawn(path.join(root,'bin','桌宠节奏师.exe'),[],{cwd:path.join(root,'bin'),windowsHide:true,stdio:'ignore',env});
const stopFile=path.join(dir,'stop');let sampler;
const result={rootPid:app.pid,debugging:false,startedAt:new Date().toISOString()};
try{
sampler=spawn('powershell.exe',['-NoProfile','-ExecutionPolicy','Bypass','-File',path.join(root,'scripts','sample-process-tree.ps1'),'-RootProcessId',String(app.pid),'-OutputFile',path.join(dir,'process-samples.jsonl'),'-StopFile',stopFile],{stdio:'ignore',windowsHide:true});
console.log('正常启动复核：不开调试端口，预热 15 秒');await delay(15000);result.start=new Date().toISOString();
console.log('正常启动占用采样：35 秒');await delay(35000);result.end=new Date().toISOString();
const lastSample=JSON.parse(fs.readFileSync(path.join(dir,'process-samples.jsonl'),'utf8').trim().split('\n').at(-1));const ownIds=lastSample.processes.map(p=>p.pid).join(',');
result.processes=JSON.parse(execFileSync('powershell.exe',['-NoProfile','-Command',`Get-CimInstance Win32_Process | Where-Object { @(${ownIds}) -contains $_.ProcessId } | Select-Object ProcessId,ParentProcessId,Name,CommandLine | ConvertTo-Json -Compress`],{encoding:'utf8',windowsHide:true}));
result.complete=true;
}catch(error){result.error=String(error);process.exitCode=1}
finally{
fs.writeFileSync(path.join(dir,'result.json'),JSON.stringify(result,null,2));fs.writeFileSync(stopFile,'stop');
execFileSync('powershell.exe',['-NoProfile','-Command',`$petProcess=Get-Process -Id ${app.pid} -ErrorAction SilentlyContinue;if($petProcess){$null=$petProcess.CloseMainWindow()}`],{windowsHide:true});
await delay(1500);if(app.exitCode===null)app.kill();if(sampler?.exitCode===null)sampler.kill();console.log(dir);
}

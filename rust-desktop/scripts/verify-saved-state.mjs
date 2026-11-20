import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { root } from './toolchain.mjs';

const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const previous=fs.readdirSync(path.join(root,'.tmp')).filter(name=>name.startsWith('desktop-'))
  .sort().reverse().map(name=>path.join(root,'.tmp',name))
  .find(dir=>fs.existsSync(path.join(dir,'report.json')));
if(!previous && !process.env.PET_TEST_PROFILE)throw new Error('请先运行一次 verify:desktop。');
const exe=path.resolve(process.env.PET_TEST_EXE || path.join(root,'bin','桌宠节奏师.exe'));
const profile=process.env.PET_TEST_PROFILE || path.join(previous,'profile');
const scratch=fs.mkdtempSync(path.join(root,'.tmp','saved-state-'));
const markerName=`__rust_probe_${Date.now()}.json`;
const marker=path.join(path.dirname(exe),'character_assets',markerName);
fs.writeFileSync(marker,JSON.stringify({version:1}));
const port=19328;
const child=spawn(exe,[],{cwd:path.dirname(exe),windowsHide:true,stdio:'ignore',env:{...process.env,
  PET_DATA_DIR:profile,PET_DEBUG_PORT:String(port),PET_LOG_FILE:path.join(scratch,'native.log')}});
let ws;const pending=new Map();let serial=0;
const limit=setTimeout(()=>{child.kill();process.exitCode=1;},45000);
try{
  let pages;
  for(let i=0;i<200;i++){
    try{pages=await fetch(`http://127.0.0.1:${port}/json/list`).then(r=>r.json());}catch{pages=[];}
    if(pages.some(p=>p.title==='桌宠节奏师 · 控制台'))break;
    await sleep(100);
  }
  const target=pages.find(p=>p.url==='https://pet.localhost/index.html');assert.ok(target,'重启后主窗口应加载');
  ws=new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject;});
  ws.onmessage=event=>{const message=JSON.parse(event.data);if(message.id){const task=pending.get(message.id);if(task){pending.delete(message.id);message.error?task.reject(new Error(JSON.stringify(message.error))):task.resolve(message.result);}}};
  const js=expression=>new Promise((resolve,reject)=>{
    const id=++serial;const timeout=setTimeout(()=>{pending.delete(id);reject(new Error('页面检查超时'));},10000);
    pending.set(id,{resolve:result=>{clearTimeout(timeout);result.exceptionDetails?reject(new Error(JSON.stringify(result.exceptionDetails))):resolve(result.result.value);},reject});
    ws.send(JSON.stringify({id,method:'Runtime.evaluate',params:{expression,returnByValue:true,awaitPromise:true,userGesture:true}}));
  });
  const settings=await js(`JSON.parse(localStorage.getItem('desk_pet_settings_v1'))`);
  const todo=await js(`JSON.parse(localStorage.getItem('desk_pet_todo_list_v1'))`);
  assert.equal(settings.endSoundEnabled,false);assert.equal(settings.randomIntervalSeconds,5);
  assert.equal(todo.listName,'Rust 独立版验证');assert.equal(todo.items[0].text,'保存成功');
  const readMarker=()=>js(`fetch('/character_assets/${markerName}').then(r=>r.json())`);
  assert.equal((await readMarker()).version,1);
  fs.writeFileSync(marker,JSON.stringify({version:2}));assert.equal((await readMarker()).version,2);
  assert.equal(await js(`fetch('/character_assets/%2e%2e%2f%2e%2e%2fLICENSE').then(r=>r.status)`),403);
  assert.equal(await js(`fetch('/character_assets/%43%3a%5cWindows%5cwin.ini').then(r=>r.status)`),403);
  assert.equal(await js(`fetch('/native-pcm-worklet.js').then(r=>r.status)`),200);
  const report={passed:true,executable:exe,profile,checks:['重启后设置保留','重启后待办保留','EXE 同级外置素材读取','修改素材后不使用旧缓存','目录穿越拒绝','Windows 绝对路径拒绝','内嵌音频工作模块可读取']};
  fs.writeFileSync(path.join(scratch,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report,null,2));
  await js('window.desktopPetBridge.quit()');
}catch(error){console.error(error.stack);process.exitCode=1;}
finally{clearTimeout(limit);ws?.close();await sleep(300);if(child.exitCode===null)child.kill();fs.rmSync(marker,{force:true});console.log(`验证文件：${scratch}`);}

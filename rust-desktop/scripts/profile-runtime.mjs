import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import crypto from 'node:crypto';
import { spawn, execFileSync } from 'node:child_process';
import assert from 'node:assert/strict';
import { TraceMap, originalPositionFor } from '@jridgewell/trace-mapping';
import { root } from './toolchain.mjs';

const delay=ms=>new Promise(r=>setTimeout(r,ms));
const scratch=path.join(root,'verification',`performance-${new Date().toISOString().replace(/[:.]/g,'-')}`);
fs.mkdirSync(scratch,{recursive:true});
const exe=path.join(root,'bin','桌宠节奏师.exe'),port=19335;
const seconds=Number(process.env.PET_PERF_SECONDS || 25);
const profileSeconds=Number(process.env.PET_PERF_PROFILE_SECONDS || 8);
const profileDir=path.join(scratch,'profile');
const stopFile=path.join(scratch,'stop-sampling');
const samplesFile=path.join(scratch,'process-samples.jsonl');
const metadata={date:new Date().toISOString(),exe,sha256:crypto.createHash('sha256').update(fs.readFileSync(exe)).digest('hex'),
  logicalProcessors:os.cpus().length,cpu:os.cpus()[0]?.model,totalRam:os.totalmem(),os:os.version(),release:os.release(),seconds,profileSeconds};
let child,sampler,pet,panel;const clients=[];const stages=[];const diagnostics=[];const profiles=[];
let processMetadata=[];
const write=()=>fs.writeFileSync(path.join(scratch,'result.json'),JSON.stringify({metadata,stages,diagnostics,profiles,processMetadata},null,2));
const status=label=>{console.log(label);fs.writeFileSync(path.join(scratch,'status.txt'),label);};
async function wait(check,label,ms=25000){const until=Date.now()+ms;while(Date.now()<until){const value=await check();if(value)return value;await delay(100);}throw Error('等待超时：'+label);}
async function pages(){try{return await fetch(`http://127.0.0.1:${port}/json/list`).then(r=>r.json());}catch{return [];}}
async function connect(page){
  const ws=new WebSocket(page.webSocketDebuggerUrl);await new Promise((r,j)=>{ws.onopen=r;ws.onerror=j;});let serial=0;const pending=new Map();
  ws.onmessage=e=>{const m=JSON.parse(e.data);if(m.id){const p=pending.get(m.id);if(p){pending.delete(m.id);m.error?p.reject(Error(JSON.stringify(m.error))):p.resolve(m.result);}}};
  const send=(method,params={})=>new Promise((resolve,reject)=>{const id=++serial;const timer=setTimeout(()=>{pending.delete(id);reject(Error('检查超时：'+method));},30000);
    pending.set(id,{resolve:x=>{clearTimeout(timer);resolve(x);},reject:e=>{clearTimeout(timer);reject(e);}});ws.send(JSON.stringify({id,method,params}));});
  const js=async expression=>{const result=await send('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true,userGesture:true});if(result.exceptionDetails)throw Error(JSON.stringify(result.exceptionDetails));return result.result.value;};
  const client={send,js,close:()=>ws.close()};clients.push(client);await send('Performance.enable');return client;
}
async function attach(){
  const main=await wait(async()=>(await pages()).find(p=>p.url==='https://pet.localhost/index.html'),'桌宠主窗口');pet=await connect(main);
  const consolePage=await wait(async()=>(await pages()).find(p=>p.title==='桌宠节奏师 · 控制台'),'控制台');panel=await connect(consolePage);
  await wait(()=>panel.js(`document.querySelector('[data-testid="asset-status"]')?.textContent==='角色素材已就绪'`),'素材就绪');
}
const click=id=>panel.js(`(()=>{const e=document.querySelector('[data-testid="${id}"]');if(!e)throw Error('缺少控件 ${id}');e.click()})()`);
const change=(id,value)=>panel.js(`(()=>{const e=document.querySelector('[data-testid="${id}"]');Object.getOwnPropertyDescriptor(e.tagName==='SELECT'?HTMLSelectElement.prototype:HTMLInputElement.prototype,'value').set.call(e,${JSON.stringify(value)});e.dispatchEvent(new Event(e.tagName==='SELECT'?'change':'input',{bubbles:true}))})()`);
async function captureState(){return pet.js(`({heap:null,maskUpdates:window.__petHitRegion?.updates,maskRects:window.__petHitRegion?.rects.length,
  imageCount:document.images.length,images:[...document.images].map(i=>({src:i.currentSrc.split('/').pop(),width:i.naturalWidth,height:i.naturalHeight,complete:i.complete})),
  canvases:[...document.querySelectorAll('canvas')].map(c=>[c.width,c.height]),viewport:[innerWidth,innerHeight,devicePixelRatio],
  musicRunning:document.querySelector('[data-testid="character-scene"]')?.dataset.musicRunning})`);}
async function heap(label){await pet.send('HeapProfiler.collectGarbage');await delay(200);const value={label,time:new Date().toISOString(),usage:await pet.send('Runtime.getHeapUsage'),dom:await pet.send('Memory.getDOMCounters'),state:await captureState()};diagnostics.push(value);write();return value;}
function mapProfile(profile){
  const nodes=new Map(profile.nodes.map(n=>[n.id,n]));const self=new Map();let total=0;
  for(let i=0;i<(profile.samples||[]).length;i++){const weight=profile.timeDeltas?.[i]||1000;total+=weight;self.set(profile.samples[i],(self.get(profile.samples[i])||0)+weight);}
  const inclusive=new Map();const parents=new Map();for(const n of profile.nodes)for(const c of n.children||[])parents.set(c,n.id);
  for(const [id,weight] of self){let at=id;while(at){inclusive.set(at,(inclusive.get(at)||0)+weight);at=parents.get(at);}}
  const mapFiles=fs.readdirSync(path.join(root,'.tmp','performance-map','assets')).filter(f=>f.endsWith('.js.map'));
  const sourceMaps=mapFiles.map(f=>new TraceMap(JSON.parse(fs.readFileSync(path.join(root,'.tmp','performance-map','assets',f)))));
  const locate=frame=>{if(!frame.url.includes('/assets/'))return null;const pos=originalPositionFor(sourceMaps[0],{line:frame.lineNumber+1,column:frame.columnNumber});return pos.source?pos:null;};
  const rows=[...nodes.values()].map(n=>({name:n.callFrame.functionName||'(匿名)',url:n.callFrame.url,line:n.callFrame.lineNumber+1,column:n.callFrame.columnNumber,
    source:locate(n.callFrame),selfMs:(self.get(n.id)||0)/1000,inclusiveMs:(inclusive.get(n.id)||0)/1000,selfPct:(self.get(n.id)||0)/total*100}));
  return {durationMs:total/1000,samples:profile.samples?.length,top:rows.sort((a,b)=>b.selfMs-a.selfMs).slice(0,45),
    inclusive:rows.filter(r=>r.url).sort((a,b)=>b.inclusiveMs-a.inclusiveMs).slice(0,30)};
}
async function profile(label){
  status(`热点采样：${label}（${profileSeconds} 秒）`);
  await pet.send('Profiler.enable');await pet.send('Profiler.setSamplingInterval',{interval:1000});
  await pet.send('HeapProfiler.startSampling',{samplingInterval:32768,includeObjectsCollectedByMajorGC:true,includeObjectsCollectedByMinorGC:true});
  await pet.send('Profiler.start');await delay(profileSeconds*1000);
  const cpu=(await pet.send('Profiler.stop')).profile;await pet.send('Profiler.disable');
  const allocation=(await pet.send('HeapProfiler.stopSampling')).profile;
  fs.writeFileSync(path.join(scratch,`${label}.cpuprofile`),JSON.stringify(cpu));fs.writeFileSync(path.join(scratch,`${label}.heapprofile`),JSON.stringify(allocation));
  const allocationRows=[];function visit(n){if(n.selfSize)allocationRows.push({frame:n.callFrame,bytes:n.selfSize});for(const c of n.children||[])visit(c);}visit(allocation.head);
  profiles.push({label,cpu:mapProfile(cpu),allocationTop:allocationRows.sort((a,b)=>b.bytes-a.bytes).slice(0,25),sampledAllocationBytes:allocationRows.reduce((s,x)=>s+x.bytes,0)});write();
}
async function measure(id,label,{hot=false}={}){
  status(`占用采样：${label}（${seconds} 秒）`);await delay(3000);
  const start=new Date().toISOString();const before=await pet.send('Performance.getMetrics');const stateBefore=await captureState();
  await delay(seconds*1000);
  const after=await pet.send('Performance.getMetrics');const stateAfter=await captureState();const end=new Date().toISOString();
  const a=Object.fromEntries(before.metrics.map(m=>[m.name,m.value])),b=Object.fromEntries(after.metrics.map(m=>[m.name,m.value]));
  const metrics={};for(const key of ['TaskDuration','ScriptDuration','LayoutDuration','RecalcStyleDuration','JSHeapUsedSize','JSHeapTotalSize','Documents','Nodes','LayoutCount','RecalcStyleCount'])metrics[key]={before:a[key],after:b[key],delta:b[key]-a[key]};
  stages.push({id,label,start,end,metrics,stateBefore,stateAfter});write();if(hot)await profile(id);
}
try{
  status('启动独立测试实例；使用专用设置目录');
  child=spawn(exe,[],{cwd:path.dirname(exe),stdio:'ignore',windowsHide:true,env:{...process.env,PET_DEBUG_PORT:String(port),PET_DATA_DIR:profileDir,PET_LOG_FILE:path.join(scratch,'native.log')}});
  metadata.rootPid=child.pid;metadata.startedAt=new Date().toISOString();write();
  sampler=spawn('powershell.exe',['-NoProfile','-ExecutionPolicy','Bypass','-File',path.join(root,'scripts','sample-process-tree.ps1'),'-RootProcessId',String(child.pid),'-OutputFile',samplesFile,'-StopFile',stopFile],{stdio:['ignore','pipe','pipe'],windowsHide:true});
  sampler.stderr.on('data',d=>console.error('采样器：'+d));
  await attach();
  await click('tab-emotion');await panel.js(`(()=>{const e=document.querySelector('[data-testid="random-enabled"]');if(e.checked)e.click()})()`);await click('tab-status');
  await delay(12000);await heap('首次预热后（强制回收）');
  await measure('idle_console','休闲待机，控制台显示',{hot:true});
  await pet.js('window.desktopPetBridge.hideConsole()');
  await measure('idle_hidden','休闲待机，控制台隐藏',{hot:true});
  await pet.js('window.desktopPetBridge.setLocked(true)');
  await measure('locked_hidden','休闲待机，锁定并隐藏控制台');
  await pet.js('window.desktopPetBridge.setLocked(false);window.desktopPetBridge.showConsole()');
  await click('mode-study');await change('timer-mode','stopwatch');await click('btn-timer-play');
  await measure('study_timer','自习正计时，控制台显示',{hot:true});
  await click('btn-timer-pause');await click('btn-timer-reset');await click('mode-chill');
  await click('tab-emotion');
  await panel.js(`window.__stress=setInterval(()=>{const ids=['happy','sad','think','love','shock','doze','neutral'];const id=ids[(window.__stressAt=(window.__stressAt||0)+1)%ids.length];document.querySelector('[data-testid="emotion-'+id+'"]')?.click()},800);void 0`);
  await measure('expression_stress','表情切换压力，每 0.8 秒切换',{hot:true});
  await panel.js('clearInterval(window.__stress);void 0');await click('emotion-neutral');
  await click('tab-metronome');
  await pet.js(`(()=>{const ctx=window.__perfAudio=new AudioContext();const rate=ctx.sampleRate;const buffer=ctx.createBuffer(1,rate*2,rate);const data=buffer.getChannelData(0);for(let i=0;i<data.length;i++){const t=i/rate;data[i]=Math.sin(2*Math.PI*80*t)*Math.exp(-(t%0.5)*35)*0.12;}const s=window.__perfSource=ctx.createBufferSource();s.buffer=buffer;s.loop=true;s.connect(ctx.destination);s.start();void ctx.resume()})()`);
  await click('btn-system');await wait(()=>panel.js(`document.querySelector('[data-testid="music-status"]')?.textContent==='正在音频同步'`),'WASAPI 同步');
  await delay(7000);await pet.js('window.desktopPetBridge.hideConsole()');
  await measure('audio_hidden','系统音频同步，120 BPM 测试信号，控制台隐藏',{hot:true});
  metadata.detectedBpm=await panel.js(`document.querySelector('[data-testid="bpm-value"]')?.textContent`);
  await panel.js(`window.dispatchEvent(new KeyboardEvent('keydown',{key:'q',bubbles:true}));void 0`);
  await pet.js('window.__perfSource.stop();window.__perfAudio.close();void 0');
  await delay(10000);await measure('recovered_idle','停止音频后回到休闲待机，控制台隐藏');
  await heap('全部功能运行后（强制回收）');
  // Run the focused A/B/A experiment separately with profile-isolation.mjs.
  try {
    const last=JSON.parse(fs.readFileSync(samplesFile,'utf8').trim().split('\n').at(-1));
    const ids=last.processes.map(p=>p.pid).join(',');
    processMetadata=JSON.parse(execFileSync('powershell.exe',['-NoProfile','-Command',`Get-CimInstance Win32_Process | Where-Object { @(${ids}) -contains $_.ProcessId } | Select-Object ProcessId,ParentProcessId,Name,CommandLine | ConvertTo-Json -Compress`],{encoding:'utf8',windowsHide:true}));
  } catch(error){metadata.processMetadataError=String(error);}
  metadata.finishedAt=new Date().toISOString();metadata.complete=true;write();
  status('测量完成：'+scratch);
} catch(error){metadata.error=error.stack;write();console.error(error);process.exitCode=1;}
finally{
  fs.writeFileSync(stopFile,'stop');if(pet)try{await pet.js('window.desktopPetBridge.quit()');}catch{}
  for(const c of clients)c.close();await delay(500);if(child?.exitCode===null)child.kill();
  if(sampler?.exitCode===null){await Promise.race([new Promise(r=>sampler.once('exit',r)),delay(2000)]);if(sampler.exitCode===null)sampler.kill();}
  console.log('原始数据：'+scratch);
}

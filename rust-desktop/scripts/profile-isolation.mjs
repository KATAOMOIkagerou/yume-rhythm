// Controlled A/B/A experiment. All changes exist only in a test WebView session.
import fs from 'node:fs';import path from 'node:path';import {spawn} from 'node:child_process';import {root} from './toolchain.mjs';
const delay=ms=>new Promise(r=>setTimeout(r,ms));
const dir=path.join(root,'verification',`isolation-${Date.now()}`);fs.mkdirSync(dir,{recursive:true});
const exe=path.join(root,'bin','桌宠节奏师.exe'),port=19336,stopFile=path.join(dir,'stop'),data={exe,stages:[],startedAt:new Date().toISOString()};
let app,sampler;const clients=[];
async function connect(page){const ws=new WebSocket(page.webSocketDebuggerUrl);await new Promise((r,j)=>{ws.onopen=r;ws.onerror=j});let n=0;const jobs=new Map(),events=new Map();
ws.onmessage=e=>{const m=JSON.parse(e.data),job=jobs.get(m.id);if(job){jobs.delete(m.id);m.error?job.reject(Error(JSON.stringify(m.error))):job.resolve(m.result)}else if(events.has(m.method)){events.get(m.method)(m.params);events.delete(m.method)}};
const send=(method,params={})=>new Promise((resolve,reject)=>{const id=++n;jobs.set(id,{resolve,reject});ws.send(JSON.stringify({id,method,params}))});
const js=async expression=>{const r=await send('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true,userGesture:true});if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails));return r.result.value};const c={send,js,event:name=>new Promise(r=>events.set(name,r)),close:()=>ws.close()};clients.push(c);return c;}
try{
app=spawn(exe,[],{cwd:path.dirname(exe),windowsHide:true,stdio:'ignore',env:{...process.env,PET_DEBUG_PORT:String(port),PET_DATA_DIR:path.join(dir,'profile')}});data.rootPid=app.pid;
sampler=spawn('powershell.exe',['-NoProfile','-ExecutionPolicy','Bypass','-File',path.join(root,'scripts','sample-process-tree.ps1'),'-RootProcessId',String(app.pid),'-OutputFile',path.join(dir,'process-samples.jsonl'),'-StopFile',stopFile],{windowsHide:true,stdio:'ignore'});
let pages=[];for(let i=0;i<200;i++){try{pages=await fetch(`http://127.0.0.1:${port}/json/list`).then(r=>r.json())}catch{}if(pages.some(p=>p.title==='桌宠节奏师 · 控制台'))break;await delay(100)}
const pet=await connect(pages.find(p=>p.url==='https://pet.localhost/index.html'));
const panel=await connect(pages.find(p=>p.title==='桌宠节奏师 · 控制台'));
await delay(3000);await panel.js(`document.querySelector('[data-testid="tab-emotion"]').click()`);await delay(100);
await panel.js(`(()=>{const e=document.querySelector('[data-testid="random-enabled"]');if(e.checked)e.click()})()`);
await pet.js('window.desktopPetBridge.hideConsole()');await delay(10000);
for(const [index,disabled] of [false,true,false].entries()){
  if(disabled){
    const proto=(await pet.send('Runtime.evaluate',{expression:'MutationObserver.prototype'})).result.objectId;
    const objects=(await pet.send('Runtime.queryObjects',{prototypeObjectId:proto})).objects.objectId;
    const result=await pet.send('Runtime.callFunctionOn',{objectId:objects,returnByValue:true,functionDeclaration:`function(){
      document.body.style.setProperty('--perf-probe','1');
      window.__perfMaskObservers=this.filter(o=>o.takeRecords().some(r=>r.target===document.body));
      window.__perfMaskObservers.forEach(o=>o.disconnect());document.body.style.removeProperty('--perf-probe');return window.__perfMaskObservers.length;
    }`});if(result.result.value!==1)throw Error('未能唯一定位掩码观察器：'+result.result.value);
    await pet.send('Runtime.releaseObject',{objectId:objects});await pet.send('Runtime.releaseObject',{objectId:proto});
  } else if(index===2)await pet.js(`window.__perfMaskObservers.forEach(o=>o.observe(document.body,{subtree:true,childList:true,attributes:true,attributeFilter:['src','class','style']}));void 0`);
  await pet.send('HeapProfiler.collectGarbage');await delay(3000);
  const stage={id:disabled?'mask_disabled':index===0?'baseline_before':'baseline_after',disabled,start:new Date().toISOString(),heapBefore:await pet.send('Runtime.getHeapUsage'),updatesBefore:await pet.js('window.__petHitRegion?.updates')};
  console.log(`对照采样：${stage.id}（20 秒）`);await delay(20000);
  stage.end=new Date().toISOString();stage.updatesAfter=await pet.js('window.__petHitRegion?.updates');stage.heapAfter=await pet.send('Runtime.getHeapUsage');data.stages.push(stage);fs.writeFileSync(path.join(dir,'result.json'),JSON.stringify(data,null,2));
}
// A short timeline identifies expensive browser rendering operations separately
// from application JS. This interval is excluded from OS CPU baseline averages.
console.log('采集浏览器绘制时间线（8 秒）');
await pet.send('Tracing.start',{categories:'devtools.timeline,disabled-by-default-devtools.timeline,blink.user_timing',transferMode:'ReturnAsStream'});
await delay(8000);
const completion=pet.event('Tracing.tracingComplete');await pet.send('Tracing.end');
const {stream}=await completion;let trace='';while(true){const chunk=await pet.send('IO.read',{handle:stream,size:1048576});trace+=chunk.base64Encoded?Buffer.from(chunk.data,'base64').toString():chunk.data;if(chunk.eof)break;}await pet.send('IO.close',{handle:stream});
fs.writeFileSync(path.join(dir,'browser-timeline.json'),trace);
const parsed=JSON.parse(trace),durations=new Map();for(const e of parsed.traceEvents)if(e.ph==='X' && e.dur){const key=e.name;const row=durations.get(key)||{name:key,count:0,ms:0};row.count++;row.ms+=e.dur/1000;durations.set(key,row);}
data.timelineTop=[...durations.values()].sort((a,b)=>b.ms-a.ms).slice(0,50);
data.complete=true;data.finishedAt=new Date().toISOString();fs.writeFileSync(path.join(dir,'result.json'),JSON.stringify(data,null,2));await pet.js('window.desktopPetBridge.quit()');
}catch(error){data.error=error.stack;fs.writeFileSync(path.join(dir,'result.json'),JSON.stringify(data,null,2));console.error(error);process.exitCode=1}
finally{fs.writeFileSync(stopFile,'stop');for(const c of clients)c.close();await delay(1000);if(app?.exitCode===null)app.kill();if(sampler?.exitCode===null)sampler.kill();console.log(dir)}

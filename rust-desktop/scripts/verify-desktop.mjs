import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { inflateSync } from 'node:zlib';
import { root } from './toolchain.mjs';

const sleep = ms => new Promise(resolve => setTimeout(resolve,ms));
const scratch = path.join(root, '.tmp', `desktop-${Date.now()}`);
fs.mkdirSync(scratch,{recursive:true});
const executable = path.resolve(process.env.PET_TEST_EXE || path.join(root,'bin','桌宠节奏师.exe'));
const port = Number(process.env.PET_TEST_PORT || 19327);
const child = spawn(executable, [], {cwd:path.dirname(executable), windowsHide:true, stdio:'ignore',env:{...process.env,
  PET_DEBUG_PORT:String(port),PET_DATA_DIR:process.env.PET_TEST_PROFILE || path.join(scratch,'profile'),PET_LOG_FILE:path.join(scratch,'native.log')}});
child.on('error', error => console.error(error));
const deadline = setTimeout(()=>{ child.kill(); process.exitCode=1; },150000);
const clients=[];
const errors=[];
async function wait(check,label,timeout=12000) {
  const until=Date.now()+timeout;
  while(Date.now()<until) { const result=await check(); if(result) return result; await sleep(100); }
  throw new Error(`等待超时：${label}`);
}
async function pages() { try {return await fetch(`http://127.0.0.1:${port}/json/list`).then(r=>r.json());}catch{return [];} }
async function connect(target) {
  const ws=new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject;});
  let serial=0; const pending=new Map();
  ws.onmessage=event=>{
    const message=JSON.parse(event.data);
    if(process.env.PET_VERBOSE && message.id)console.log('CDP response',target.title,message.id);
    if(message.id) {
      const task=pending.get(message.id);if(!task)return;pending.delete(message.id);
      message.error?task.reject(new Error(JSON.stringify(message.error))):task.resolve(message.result);
    } else if(message.method==='Runtime.exceptionThrown') errors.push(message.params.exceptionDetails);
  };
  const send=(method,params={})=>new Promise((resolve,reject)=>{const id=++serial;
    if(process.env.PET_VERBOSE)console.log('CDP request',target.title,id,method,params.expression?.slice(0,100)||'');
    const timeout=setTimeout(()=>{pending.delete(id);reject(new Error(`检查调用超时：${method} ${JSON.stringify(params).slice(0,180)}`));},20000);
    pending.set(id,{resolve:value=>{clearTimeout(timeout);resolve(value);},reject:error=>{clearTimeout(timeout);reject(error);}});
    ws.send(JSON.stringify({id,method,params}));});
  const js=async expression=>{
    const result=await send('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true,userGesture:true});
    if(result.exceptionDetails)throw new Error(JSON.stringify(result.exceptionDetails));
    return result.result.value;
  };
  await send('Runtime.enable');
  const client={send,js,close:()=>ws.close()};clients.push(client);return client;
}

let step='启动原生窗口';
try {
  const target=await wait(async()=> (await pages()).find(p=>p.url==='https://pet.localhost/index.html'),'主 WebView',30000);
  console.log('正在检查：主窗口与悬浮控制台');
  const pet=await connect(target);
  // WebView2 publishes the blank target before completing window.open. Wait for
  // React to name the child before attaching, avoiding evaluation during handoff.
  const panelTarget=await wait(async()=> (await pages()).find(p=>p.id!==target.id && p.type==='page' && p.title==='桌宠节奏师 · 控制台'),'悬浮控制台');
  const panel=await connect(panelTarget);
  const click=id=>panel.js(`document.querySelector('[data-testid="${id}"]').click();void 0`);
  const text=id=>panel.js(`document.querySelector('[data-testid="${id}"]')?.textContent`);
  const change=(id,value)=>panel.js(`(()=>{
    const input=document.querySelector('[data-testid="${id}"]');
    const proto=input.tagName==='SELECT'?HTMLSelectElement.prototype:HTMLInputElement.prototype;
    Object.getOwnPropertyDescriptor(proto,'value').set.call(input,${JSON.stringify(value)});
    input.dispatchEvent(new Event(input.tagName==='SELECT'?'change':'input',{bubbles:true}));
  })()`);
  const nativeCall=(command,payload={})=>pet.js(`new Promise((resolve,reject)=>{
    const previous=window.__nativeReply;const id=900000;
    window.__nativeReply=(i,value,error)=>{previous(i,value,error);if(i===id){window.__nativeReply=previous;error?reject(Error(error)):resolve(value);}};
    window.ipc.postMessage(JSON.stringify({id,command:${JSON.stringify(command)},...${JSON.stringify(payload)}}));
  })`);
  const inspect=()=>nativeCall('inspect');
  await wait(()=>text('tab-status'),'控制台渲染');
  await wait(()=>text('asset-status').then(t=>t==='角色素材已就绪'),'全部角色素材');
  const native=await inspect();
  assert.equal(native.windowShape,0,'透明穿透不能设置可见窗口裁切区域');
  assert.deepEqual(native.petSize.map(n=>Math.round(n/native.scale)),[540,590]);
  assert.deepEqual(native.consoleSize.map(n=>Math.round(n/native.scale)),[470,780]);
  assert.equal(await pet.js(`getComputedStyle(document.body).backgroundColor`),'rgba(0, 0, 0, 0)');
  assert.equal(await pet.js(`Boolean(document.querySelector('[data-testid="layer-scene-bg"]'))`),false);
  assert.equal(await panel.js('document.documentElement.scrollWidth<=innerWidth'),true,'控制台不能横向溢出');
  step='原生透明像素命中区域';
  await wait(()=>inspect().then(v=>v.regionRects>0),'透明区域已同步到 Windows');
  assert.equal(await pet.js(`getComputedStyle(document.documentElement).colorScheme`),'normal');
  await pet.js('window.desktopPetBridge.hideConsole()');await nativeCall('focus_pet');await sleep(150);
  const alphaPoints=await pet.js(`(()=>{
    const mask=window.__petHitRegion;const scene=document.querySelector('[data-testid="character-scene"]').getBoundingClientRect();
    const opaque={x:Math.floor(scene.x+scene.width/2),y:Math.floor(scene.bottom-60)};
    let hole=null;
    for(let y=Math.floor(scene.y+scene.height*.3);y<scene.bottom-100 && !hole;y+=3){
      const runs=mask.rects.filter(r=>r[1]===y);
      for(let i=1;i<runs.length;i++)if(runs[i][0]-runs[i-1][2]>12){hole={x:Math.floor((runs[i][0]+runs[i-1][2])/2),y};break;}
    }
    return {opaque,opaqueAlpha:mask.alphaAt(opaque.x,opaque.y),hole,corner:{x:2,y:2}};
  })()`);
  assert.ok(alphaPoints.opaqueAlpha>0);assert.ok(alphaPoints.hole,'应找到图片内部的透明空隙');
  step='实际 Windows 桌面透明合成';
  const composition=[];
  // Pause just the character while comparing the browser's full-alpha surface
  // against Windows output; otherwise movement between captures looks like clipping.
  await pet.js(`window.__savedRAF=requestAnimationFrame;window.__pausedFrames=[];window.requestAnimationFrame=cb=>{window.__pausedFrames.push(cb);return 0};
    window.__pauseStyle=document.createElement('style');window.__pauseStyle.textContent='*,*::before,*::after{animation-play-state:paused!important}';document.head.append(window.__pauseStyle);void 0`);
  await sleep(150);
  const expectedShot=await pet.send('Page.captureScreenshot',{format:'png'});
  const edgeSamples=await pet.js(`new Promise((resolve,reject)=>{
    const image=new Image();image.onerror=reject;image.onload=()=>{
      const c=document.createElement('canvas');c.width=image.width;c.height=image.height;
      const ctx=c.getContext('2d');ctx.drawImage(image,0,0);const d=ctx.getImageData(0,0,c.width,c.height).data;
      const sx=c.width/innerWidth,sy=c.height/innerHeight,at=(x,y)=>(Math.round(y*sy)*c.width+Math.round(x*sx))*4;
      const points=[];
      for(let y=80;y<innerHeight-4;y+=7)for(let x=4;x<innerWidth-4;x++){
        const i=at(x,y),a=d[i+3];
        if(a>0 && a<255 || a>240 && (d[at(x-2,y)+3]===0 || d[at(x+2,y)+3]===0))points.push({x,y,rgba:[...d.slice(i,i+4)]});
      }
      resolve(points.filter((p,i)=>i%Math.max(1,Math.floor(points.length/500))===0));
    };image.src=${JSON.stringify('data:image/png;base64,'+expectedShot.data)};
  })`);
  assert.ok(edgeSamples.length>100,'应覆盖角色、桌子和配件的轮廓像素');
  try {
    for (const locked of [false,true]) {
      await pet.js(`window.desktopPetBridge.setLocked(${locked})`);
      for (const color of [[24,36,52],[42,96,72]]) {
        await nativeCall('composition_probe',{operation:'show',color});await sleep(250);
        const points=[{x:20,y:100},alphaPoints.hole,alphaPoints.opaque];
        const pixels=await nativeCall('composition_probe',{operation:'sample',points});
        composition.push({locked,color,points,pixels});
        console.log('桌面合成采样',JSON.stringify(composition.at(-1)));
        for(let i=0;i<2;i++)assert.ok(pixels[i].every((v,c)=>Math.abs(v-color[c])<=2),`原生透明区域出现底色：${JSON.stringify(composition.at(-1))}`);
        assert.ok(pixels[2].some((v,c)=>Math.abs(v-color[c])>15),'实体素材未显示');
        const edgePixels=await nativeCall('composition_probe',{operation:'sample',points:edgeSamples});
        const errors=edgeSamples.map((p,i)=>Math.max(...color.map((bg,c)=>Math.abs(edgePixels[i][c]-Math.round(p.rgba[c]*p.rgba[3]/255+bg*(1-p.rgba[3]/255))))));
        const bad=errors.filter(e=>e>6).length;
        composition.at(-1).edgePixels=edgeSamples.length;composition.at(-1).maxEdgeError=Math.max(...errors);composition.at(-1).badEdgePixels=bad;
        assert.ok(bad<=edgeSamples.length*.01,`轮廓合成异常（白边或素材被裁掉）：${JSON.stringify(composition.at(-1))}`);
      }
    }
  } finally {
    await nativeCall('composition_probe',{operation:'hide'});
    await pet.js('window.desktopPetBridge.setLocked(false)');
    await pet.js(`window.requestAnimationFrame=window.__savedRAF;for(const cb of window.__pausedFrames)requestAnimationFrame(cb);window.__pauseStyle.remove();requestAnimationFrame(()=>dispatchEvent(new Event('resize')));void 0`);
  }
  const cornerHit=await nativeCall('hit_test',alphaPoints.corner),holeHit=await nativeCall('hit_test',alphaPoints.hole),bodyHit=await nativeCall('hit_test',alphaPoints.opaque);
  assert.equal(cornerHit.inRegion,false);assert.equal(cornerHit.hitsPet,false,'透明角落不应命中桌宠 HWND');
  assert.equal(holeHit.inRegion,false);assert.equal(holeHit.hitsPet,false,'图层内部透明空隙应直接穿透到下面窗口');
  assert.equal(bodyHit.inRegion,true);assert.equal(bodyHit.hitsPet,true,'角色实体部分应保留交互');
  await pet.js('window.desktopPetBridge.showConsole()');
  const referenceFile=process.env.PET_REFERENCE_LAYOUT;
  const reference=referenceFile?JSON.parse(fs.readFileSync(referenceFile,'utf8')):null;
  const initialLayout={};let alignedControls=0;
  for(const tab of ['status','emotion','metronome','settings']){
    await click(`tab-${tab}`);await sleep(100);
    initialLayout[tab]=await panel.js(`Object.fromEntries([...document.querySelectorAll('[data-testid]')].map(e=>{const r=e.getBoundingClientRect();return[e.dataset.testid,{x:r.x,y:r.y,width:r.width,height:r.height}]}))`);
    if(reference)for(const [id,expected] of Object.entries(reference[tab])){
      const actual=initialLayout[tab][id];assert.ok(actual,`${tab} 缺少原有控件 ${id}`);
      for(const axis of ['x','y','width','height'])assert.ok(Math.abs(actual[axis]-expected[axis])<=1,`${tab}/${id}/${axis} 对齐偏差：${actual[axis]} 与 ${expected[axis]}`);
      alignedControls++;
    }
    const shot=await panel.send('Page.captureScreenshot',{format:'png'});fs.writeFileSync(path.join(scratch,`initial-${tab}.png`),Buffer.from(shot.data,'base64'));
  }
  fs.writeFileSync(path.join(scratch,'initial-layout.json'),JSON.stringify(initialLayout,null,2));

  step='锁定与跨窗口共享';
  await pet.js('window.desktopPetBridge.setLocked(true)');
  const lockedNative=await inspect();assert.equal(lockedNative.locked,true);assert.equal(lockedNative.clickThrough,true,'原生窗口应实际开启点击穿透');
  await click('tab-settings');
  await wait(()=>text('btn-settings-lock').then(t=>t?.includes('解除')),'锁定按钮同步');
  await click('btn-settings-lock');
  await wait(async()=>!(await inspect()).locked,'控制台解锁');
  assert.equal((await inspect()).locked,false);
  assert.equal((await nativeCall('hit_test',alphaPoints.opaque)).hitsPet,true);

  step='逐层素材完整性';
  const requireLayers=async ids=>{
    await wait(async()=>{
    const layers=await pet.js(`JSON.stringify(${JSON.stringify(ids)}.map(id=>{
      const image=document.querySelector('[data-testid="'+id+'"]');return {id,exists:!!image,loaded:!!image?.complete && image.naturalWidth>0,visible:image?getComputedStyle(image).display!=='none':false};
    }))`);
    return JSON.parse(layers).every(layer=>layer.exists && layer.loaded && layer.visible);
    },`素材完整加载：${ids.join('、')}`);
  };
  await click('tab-status');
  await requireLayers(['layer-chair-back','layer-body-base','layer-hair-back','layer-head-base','layer-hair-front','layer-desk-surface','layer-books','layer-computer-off','layer-arms-chill']);
  await click('mode-study');
  await requireLayers(['layer-chair-back','layer-body-base','layer-computer-on','layer-arms-study']);
  await click('mode-chill');

  step='基础表情与配件';
  await click('tab-emotion');await click('random-enabled');
  await click('emotion-happy');
  await wait(()=>pet.js(`document.querySelector('[data-testid="character-head-rig"]')?.dataset.emotion==='happy'`),'开心');
  await click('emotion-happy');
  await wait(()=>pet.js(`document.querySelector('[data-testid="character-head-rig"]')?.dataset.emotion==='neutral'`),'再次点击恢复自然');
  await click('emotion-think');
  await wait(()=>pet.js(`document.querySelector('[data-testid="layer-event-fx"]')?.dataset.assetFile==='fx_think_bubble.png'`),'思考配件');
  assert.equal(await pet.js(`document.querySelectorAll('[data-testid="layer-eyes"]').length`),1);
  await click('emotion-think');
  step='画手扩展配置与整体动画';
  await pet.js(`window.__originalFetch=window.fetch;window.fetch=async(input,init)=>{
    const response=await window.__originalFetch(input,init);if(!String(input).includes('expressions.json'))return response;
    const config=await response.json();config.expressions.push(
      {id:'custom',name:'扩展整体动画',type:'base',resources:{animation:'head_base.png'},durationMs:1000,random:false},
      {id:'badge',name:'星光配件',type:'accessory',resources:{overlay:'fx_love_hearts.png'},durationMs:1000,random:false});
    return new Response(JSON.stringify(config),{headers:{'Content-Type':'application/json'}});
  };void 0`);
  await click('btn-reload-assets');await wait(()=>text('emotion-custom'),'扩展表情加载');
  await click('emotion-custom');
  await wait(()=>pet.js(`Boolean(document.querySelector('[data-testid="layer-base-animation"]'))`),'整体动画');
  assert.equal(await pet.js(`document.querySelectorAll('[data-testid="layer-eyes"]').length`),0);
  await click('emotion-badge');
  assert.equal(await pet.js(`Boolean(document.querySelector('[data-testid="layer-base-animation"]'))`),true,'配件应保留整体主体');
  await click('emotion-happy');await click('emotion-badge');
  await wait(()=>pet.js(`document.querySelector('[data-testid="layer-event-fx"]')?.dataset.assetFile==='fx_love_hearts.png'`),'自定义配件');
  assert.equal(await pet.js(`document.querySelector('[data-testid="layer-eyes"]')?.dataset.assetFile`),'eyes_happy.png');
  await click('emotion-badge');await pet.js('window.fetch=window.__originalFetch;void 0');
  await click('btn-reload-assets');await wait(()=>panel.js(`!document.querySelector('[data-testid="emotion-custom"]')`),'恢复原始素材');

  step='计时、隐藏续计与完成音效';
  await pet.js(`window.__realNow=Date.now;window.__clockOffset=0;Date.now=()=>window.__realNow()+window.__clockOffset;
    window.__oscillators=0;window.__RealAudioContext=AudioContext;
    window.AudioContext=class extends window.__RealAudioContext {createOscillator(){window.__oscillators++;return super.createOscillator();}};void 0`);
  await click('tab-status');await change('timer-mode','stopwatch');
  assert.equal(await text('timer-display'),'00:00');
  await click('btn-timer-play');await pet.js('window.__clockOffset+=5000');
  await wait(()=>text('timer-display').then(t=>t!=='00:00'),'正计时增长');
  await click('btn-close-console');
  assert.equal((await inspect()).consoleVisible,false);
  await pet.js('window.__clockOffset+=5000');await sleep(350);
  await pet.js('window.desktopPetBridge.showConsole()');
  await wait(async()=>(await inspect()).consoleVisible,'重新显示');
  await click('btn-timer-pause');await wait(()=>text('pausing-remaining'),'暂停');
  const paused=await text('timer-display');await pet.js('window.__clockOffset+=10000');await sleep(350);
  assert.equal(await text('timer-display'),paused);
  await click('btn-timer-continue');await pet.js('window.__clockOffset+=5000');
  await wait(()=>text('timer-display').then(t=>t!==paused),'继续计时');
  await click('btn-timer-pause');await click('btn-timer-reset');await change('timer-mode','countdown');
  await change('input-study-minutes','1');await click('btn-timer-play');await pet.js('window.__clockOffset+=61000');
  await wait(()=>text('timer-completed-banner'),'倒计时完成');
  assert.equal(await pet.js('window.__oscillators'),3);
  await click('tab-settings');await click('setting-end-sound');await click('tab-status');
  await click('btn-timer-play');await pet.js('window.__clockOffset+=61000');await wait(()=>text('timer-completed-banner'),'静音完成');
  assert.equal(await pet.js('window.__oscillators'),3,'关闭音效后不能播放提示音');
  await pet.js('window.AudioContext=window.__RealAudioContext;Date.now=window.__realNow;void 0');
  await click('btn-toggle-todo');await change('todo-list-name','Rust 独立版验证');await change('todo-input-0','保存成功');
  await click('btn-save-list');await click('modal-save-yes');
  assert.equal(await pet.js(`JSON.parse(localStorage.getItem('desk_pet_todo_list_v1')).listName`),'Rust 独立版验证');

  step='原生 WASAPI 输出监听及节拍检测';
  await click('tab-metronome');
  assert.equal(await panel.js(`document.querySelector('[data-testid="slider-sensitivity"]').closest('details').open`),false);
  assert.equal(await panel.js(`Boolean(document.querySelector('[data-testid="btn-file"],[data-testid="btn-mic"],[data-testid="btn-demo"]'))`),false);
  await pet.js(`window.__startLoopback=window.desktopPetBridge.startLoopback;window.__captureStarted=false;
    window.desktopPetBridge.startLoopback=async()=>{const info=await window.__startLoopback();window.__captureStarted=true;await new Promise(r=>setTimeout(r,200));return info;};void 0`);
  await click('btn-system');await wait(()=>pet.js('window.__captureStarted'),'连接中的原生捕获');
  await panel.js(`window.dispatchEvent(new KeyboardEvent('keydown',{key:'q',bubbles:true}));void 0`);
  await sleep(350);
  assert.equal((await inspect()).audioActive,false,'连接期间停止必须释放捕获');
  assert.equal(await text('music-status'),'未开始','迟到的启动响应不能重启音频');
  assert.equal(await text('audio-error'),undefined);
  await pet.js('window.desktopPetBridge.startLoopback=window.__startLoopback;void 0');
  // Generate an audible 120 BPM signal through the default device, then capture
  // it using the actual Rust WASAPI path (not a mocked MediaStream).
  await pet.js(`(()=>{
    const ctx=window.__testAudio=new AudioContext();const rate=ctx.sampleRate;
    const buffer=ctx.createBuffer(1,rate*2,rate),data=buffer.getChannelData(0);
    for(let i=0;i<data.length;i++){const t=i/rate;data[i]=Math.sin(2*Math.PI*80*t)*Math.exp(-(t%0.5)*35)*0.45;}
    const source=window.__testSource=ctx.createBufferSource();source.buffer=buffer;source.loop=true;source.connect(ctx.destination);source.start();void ctx.resume();
  })()`);
  await click('btn-system');
  await wait(()=>text('music-status').then(t=>t==='正在音频同步'),'原生音频启动',18000);
  await wait(()=>text('beat-count').then(t=>/[1-9]/.test(t)),'真实回环节拍',12000);
  const beats=await text('beat-count');
  const bpm=await wait(()=>text('bpm-value').then(t=>Number(t)>=100 && Number(t)<=140 && t),'120 BPM 测速',20000);
  await pet.js(`window.__frameChanges=0;window.__frameObserver=new MutationObserver(entries=>window.__frameChanges+=entries.filter(e=>e.attributeName==='style').length);
    window.__frameObserver.observe(document.querySelector('[data-testid="character-head-rig"]'),{attributes:true,attributeFilter:['style']});void 0`);
  await sleep(2100);
  const frameChanges=await pet.js('window.__frameObserver.disconnect();window.__frameChanges');
  assert.ok(frameChanges>15 && frameChanges<140,`24 fps 动画采样异常：${frameChanges}`);
  step='原生音频切换到拖入文件';
  await pet.js(`(()=>{
    const rate=44100,length=rate*4,buffer=new ArrayBuffer(44+length*2),view=new DataView(buffer);
    const str=(at,value)=>[...value].forEach((c,i)=>view.setUint8(at+i,c.charCodeAt(0)));
    str(0,'RIFF');view.setUint32(4,36+length*2,true);str(8,'WAVE');str(12,'fmt ');view.setUint32(16,16,true);
    view.setUint16(20,1,true);view.setUint16(22,1,true);view.setUint32(24,rate,true);view.setUint32(28,rate*2,true);
    view.setUint16(32,2,true);view.setUint16(34,16,true);str(36,'data');view.setUint32(40,length*2,true);
    for(let i=0;i<length;i++){const t=i/rate;view.setInt16(44+i*2,Math.sin(2*Math.PI*80*t)*Math.exp(-(t%0.5)*35)*22000,true);}
    const dt=new DataTransfer();dt.items.add(new File([buffer],'独立版节奏测试.wav',{type:'audio/wav'}));
    document.querySelector('.pet-root').dispatchEvent(new DragEvent('drop',{bubbles:true,cancelable:true,dataTransfer:dt}));
  })()`);
  await wait(()=>text('current-file-name').then(t=>t==='独立版节奏测试.wav'),'音频文件播放');
  await wait(async()=>!(await inspect()).audioActive,'切换文件应释放原生回环');
  assert.equal(await pet.js(`document.querySelector('[data-testid="character-scene"]')?.dataset.musicRunning`),'true');
  await panel.js(`window.dispatchEvent(new KeyboardEvent('keydown',{key:'q',bubbles:true}));void 0`);
  await wait(async()=>!(await inspect()).audioActive,'停止释放原生捕获');
  await wait(()=>text('music-status').then(t=>t==='未开始'),'快捷键停止');
  await pet.js('window.__testSource.stop();window.__testAudio.close();void 0');

  step='随机表情与界面对齐截图';
  await click('tab-emotion');await pet.js('window.__realRandom=Math.random;Math.random=()=>0.99;void 0');
  await change('random-interval','5');await click('random-enabled');
  await wait(()=>pet.js(`document.querySelector('[data-testid="character-head-rig"]')?.dataset.emotion==='love'`),'随机表情',7000);
  await wait(()=>pet.js(`document.querySelector('[data-testid="character-head-rig"]')?.dataset.emotion==='neutral'`),'随机恢复',5000);
  await click('random-enabled');await pet.js('Math.random=window.__realRandom;void 0');
  const geometry={native, tabs:{}};
  for(const tab of ['status','emotion','metronome','settings']) {
    await click(`tab-${tab}`);await sleep(100);
    assert.equal(await panel.js('document.documentElement.scrollWidth<=innerWidth'),true,`${tab} 横向溢出`);
    geometry.tabs[tab]=await panel.js(`(()=>{const r=document.querySelector('.console-scroll').getBoundingClientRect();return {width:innerWidth,height:innerHeight,content:{x:r.x,y:r.y,width:r.width,height:r.height}};})()`);
    const shot=await panel.send('Page.captureScreenshot',{format:'png'});fs.writeFileSync(path.join(scratch,`${tab}.png`),Buffer.from(shot.data,'base64'));
  }
  const petShot=await pet.send('Page.captureScreenshot',{format:'png'});fs.writeFileSync(path.join(scratch,'pet.png'),Buffer.from(petShot.data,'base64'));
  // Check the actual canvas; an Emulation background override would conceal the
  // WebView2 white-canvas bug that this regression is intended to catch.
  const png=Buffer.from(petShot.data,'base64'),chunks=[];
  for(let at=8;at<png.length;){const length=png.readUInt32BE(at),type=png.toString('ascii',at+4,at+8);if(type==='IDAT')chunks.push(png.subarray(at+8,at+8+length));at+=length+12;}
  const raw=inflateSync(Buffer.concat(chunks)),colorType=png[25];
  const cornerAlpha=colorType===6?raw[4]:colorType===4?raw[2]:255;
  assert.equal(cornerAlpha,0,'真实网页截图角落应透明，不能是白底');
  const nativeLog=fs.existsSync(path.join(scratch,'native.log'))?fs.readFileSync(path.join(scratch,'native.log'),'utf8'):'';
  assert.ok(!nativeLog.includes('界面错误'),nativeLog);
  assert.deepEqual(errors,[],'不能出现未处理的界面错误');
  const report={passed:true,executable,checks:['Windows 桌面实际合成（两种底色、锁定与解锁）','不裁切可见窗口','逐层素材完整性（休闲／自习）','独立双窗口尺寸','真实画布透明背景（无截图覆盖）','透明角落及 PNG 内部空隙原生命中穿透','实体部分保留交互','锁定与原生点击穿透','表情叠加与恢复','画手扩展整体动画与配件','正计时暂停续计','控制台隐藏后续计','倒计时提示音与静音','待办保存','真实 WASAPI 回环','连接期间停止并取消迟到的响应','120 BPM 检测','原生音频切换拖入文件并释放捕获','随机表情恢复','四页无横向溢出'],composition,alphaPoints,cornerHit,holeHit,bodyHit,cornerAlpha,alignedControls,beats,bpm,frameChanges,geometry,scratch};
  fs.writeFileSync(path.join(scratch,'report.json'),JSON.stringify(report,null,2));
  console.log(JSON.stringify(report,null,2));
  await pet.js('window.desktopPetBridge.quit()');
} catch(error) {
  console.error(`失败步骤：${step}\n${error.stack}`);process.exitCode=1;
} finally {
  clearTimeout(deadline);for(const client of clients)client.close();
  await sleep(300);if(child.exitCode===null)child.kill();
  console.log(`验证文件：${scratch}`);
}

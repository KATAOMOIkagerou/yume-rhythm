import fs from 'node:fs';import path from 'node:path';import crypto from 'node:crypto';import {root} from './toolchain.mjs';
const base=path.join(root,'verification'),latest=prefix=>fs.readdirSync(base).filter(f=>f.startsWith(prefix)&&fs.statSync(path.join(base,f)).isDirectory()).sort().at(-1);
const runDir=latest('performance-'),isolationDir=latest('isolation-'),normalDir=latest('normal-');
const read=d=>JSON.parse(fs.readFileSync(path.join(base,d,'result.json'))),run=read(runDir),isolation=read(isolationDir),normal=read(normalDir);
if(!isolation.complete||!normal.complete)throw Error('复核测试尚未完成');
const cpus=run.metadata.logicalProcessors,M=1048576;
const mean=a=>a.reduce((s,n)=>s+n,0)/a.length,quantile=(a,q)=>[...a].sort((a,b)=>a-b)[Math.min(a.length-1,Math.floor(a.length*q))];
const stats=a=>({mean:mean(a),p95:quantile(a,.95),max:Math.max(...a),min:Math.min(...a)});
const files=new Map();function samples(dir){if(!files.has(dir))files.set(dir,fs.readFileSync(path.join(base,dir,'process-samples.jsonl'),'utf8').trim().split('\n').map(JSON.parse));return files.get(dir);}
function summarize(dir,stage){
 const raw=samples(dir),rows=[],perProcess=new Map();
 for(let i=1;i<raw.length;i++){
  const a=raw[i-1],b=raw[i],at=Date.parse(b.utc);if(Date.parse(a.utc)<Date.parse(stage.start)||at>Date.parse(stage.end))continue;
  const dt=(at-Date.parse(a.utc))/1000,old=new Map(a.processes.map(p=>[p.pid,p]));
  const cpu=b.processes.reduce((s,p)=>s+Math.max(0,p.cpuSeconds-(old.get(p.pid)?.cpuSeconds??p.cpuSeconds)),0)/dt;
  const row={utc:b.utc,seconds:dt,cpu:cpu/cpus*100,cores:cpu,ws:b.processes.reduce((s,p)=>s+p.ws,0),privateWs:b.processes.reduce((s,p)=>s+(p.privateWs||0),0),commit:b.processes.reduce((s,p)=>s+p.commit,0),threads:b.processes.reduce((s,p)=>s+p.threads,0),handles:b.processes.reduce((s,p)=>s+p.handles,0),processes:b.processes.length,systemCpu:(1-(b.systemIdle-a.systemIdle)/(b.systemTotal-a.systemTotal))*100};rows.push(row);
  for(const p of b.processes){const values=perProcess.get(p.pid)||[];values.push({...p,cpu:Math.max(0,p.cpuSeconds-(old.get(p.pid)?.cpuSeconds??p.cpuSeconds))/dt/cpus*100});perProcess.set(p.pid,values);}
 }
 if(!rows.length)throw Error('缺少采样：'+stage.id);
 const summary={id:stage.id,label:stage.label,samples:rows.length,duration:(Date.parse(stage.end)-Date.parse(stage.start))/1000};
 for(const key of ['cpu','cores','ws','privateWs','commit','threads','handles','processes','systemCpu'])summary[key]=stats(rows.map(r=>r[key]));
 summary.perProcess=[...perProcess].map(([pid,v])=>({pid,parent:v[0].parent,name:v[0].name,cpu:mean(v.map(p=>p.cpu)),ws:mean(v.map(p=>p.ws)),privateWs:mean(v.map(p=>p.privateWs||0)),commit:mean(v.map(p=>p.commit)),threads:mean(v.map(p=>p.threads)),handles:mean(v.map(p=>p.handles))}));
 summary.maskUpdatesPerSec=stage.stateAfter?(stage.stateAfter.maskUpdates-stage.stateBefore.maskUpdates)/summary.duration:stage.updatesAfter!==undefined?(stage.updatesAfter-stage.updatesBefore)/summary.duration:null;
 return {summary,rows};
}
const phases=run.stages.map(st=>summarize(runDir,st)),ab=isolation.stages.map(st=>summarize(isolationDir,st));
const regular=summarize(normalDir,{id:'normal',label:'正常启动，不开调试端口，控制台显示',start:normal.start,end:normal.end});
const before=ab[0].summary,off=ab[1].summary,after=ab[2].summary,baseline=(before.cpu.mean+after.cpu.mean)/2,reduction=(1-off.cpu.mean/baseline)*100;
const roles=new Map(normal.processes.map(p=>{const c=p.CommandLine||'';return[p.ProcessId,p.ProcessId===normal.rootPid?'Rust 主进程':c.includes('--type=gpu-process')?'WebView2 GPU 进程':c.includes('--type=renderer')?'WebView2 页面渲染进程':c.includes('network.mojom.NetworkService')?'WebView2 网络服务':c.includes('audio.mojom.AudioService')?'WebView2 音频服务':c.includes('--type=')?'WebView2 其他辅助进程':'WebView2 浏览器主进程'];}));
const overall={testedAt:run.metadata.date,executable:run.metadata.exe,sha256:run.metadata.sha256,normal:regular.summary,phases:phases.map(p=>p.summary),ablation:ab.map(p=>p.summary),cpuReductionPct:reduction,diagnostics:run.diagnostics,profileDirectories:[runDir,isolationDir,normalDir]};
fs.writeFileSync(path.join(base,'performance-summary.json'),JSON.stringify(overall,null,2));
const csv=['场景,时间,CPU百分比,独占驻留字节,提交字节,工作集字节,线程,句柄,进程数'];
for(const p of [regular,...phases,...ab])for(const row of p.rows)csv.push([p.summary.id,row.utc,row.cpu,row.privateWs,row.commit,row.ws,row.threads,row.handles,row.processes].join(','));fs.writeFileSync(path.join(base,'performance-samples.csv'),'\ufeff'+csv.join('\n'));
const f=(n,d=1)=>n.toFixed(d),mb=n=>f(n/M),pct=n=>f(n,2)+'%';
const table=(heads,rows)=>['| '+heads.join(' | ')+' |','| '+heads.map(()=>'---').join(' | ')+' |',...rows.map(r=>'| '+r.join(' | ')+' |')].join('\n');
const memoryTable=items=>table(['场景','CPU 均值／P95／峰值','独占驻留均值／峰值 MiB','提交均值 MiB','工作集合计均值 MiB'],items.map(p=>{const s=p.summary;return[s.label||s.id,`${pct(s.cpu.mean)}／${pct(s.cpu.p95)}／${pct(s.cpu.max)}`,`${mb(s.privateWs.mean)}／${mb(s.privateWs.max)}`,mb(s.commit.mean),mb(s.ws.mean)]}));
const hot=label=>run.profiles.find(p=>p.label===label),idle=hot('idle_hidden'),audio=hot('audio_hidden');
const sourceLink=(label,file,line)=>`[${label}](${file}${line?'#L'+line:''})`;
const hotRows=[];
for(const [label,p] of [['待机',idle],['音频同步',audio]]){
 const idleMs=p.cpu.top.find(n=>n.name==='(idle)')?.selfMs||0,activeMs=p.cpu.durationMs-idleMs;
 for(const name of ['getImageData','render','estimate','pcmListener']){const n=p.cpu.top.find(n=>n.name===name);if(n)hotRows.push([label,name,f(n.selfMs),pct(n.selfPct),pct(n.selfMs/activeMs*100)]);}
}
const first=run.diagnostics[0],last=run.diagnostics[1],n=regular.summary,native=n.perProcess.find(p=>p.pid===normal.rootPid);
const assets=fs.readdirSync(path.join(root,'public','character_assets')).filter(f=>f.endsWith('.png')&&f!=='scene_bg.png').map(file=>{const b=fs.readFileSync(path.join(root,'public','character_assets',file));return{file,width:b.readUInt32BE(16),height:b.readUInt32BE(20),bytes:b.length}});
const assetDecoded=assets.reduce((s,a)=>s+a.width*a.height*4,0),assetPacked=assets.reduce((s,a)=>s+a.bytes,0);
const currentHash=crypto.createHash('sha256').update(fs.readFileSync(run.metadata.exe)).digest('hex');if(currentHash!==run.metadata.sha256)throw Error('测试后程序散列发生变化');
const lines=[];const add=s=>lines.push(s);
add('# Rust 版当前占用与热点测试报告');
add(`测试日期：2026-11-20；版本：当前透明修正版发布程序。正常启动复核：${normal.start} 至 ${normal.end}（UTC）。`);
add(`## 结论\n\n当前版本存在明确的性能优化空间，**第一优先级是透明区域命中掩码的全画布读回与重复构建**。正常启动、不开调试端口时，整个应用进程树平均 CPU 为 **${pct(n.cpu.mean)}**，独占驻留内存 **${mb(n.privateWs.mean)} MiB**，提交内存 **${mb(n.commit.mean)} MiB**。Rust 主进程本身平均 CPU 为 ${pct(native.cpu)}、独占驻留 ${mb(native.privateWs)} MiB；主要内存在 WebView2 子进程。`);
add(`同一实例 A/B/A 对照中，临时停用掩码更新、保留角色动画，CPU 从两次正常段平均 **${pct(baseline)}** 降至 **${pct(off.cpu.mean)}**，下降 **${f(reduction)}%**。这是定位实验，停止掩码会破坏随动画更新的精准穿透，不能作为可直接交付的优化方案。`);
add('## 1. 环境与统计口径');
add(`- 处理器：${run.metadata.cpu}；操作系统向进程暴露 ${cpus} 个逻辑处理器，物理核心拓扑不用于本报告的 CPU 换算。\n- 内存：${f(run.metadata.totalRam/1024**3)} GiB；${run.metadata.os}；WebView2 157.0.4341.60；缩放 200%。\n- 桌宠 540×590、控制台 470×780 逻辑像素；角色目标帧率 24 fps。\n- 发布 EXE SHA-256：\`${run.metadata.sha256}\`。全程未修改或重新编译应用功能代码。测试脚本和诊断操作均使用独立用户目录，不读取或改写日常设置。\n- 每秒采样本次 Rust 进程及其全部后代，自动排除其他应用的 WebView2、测试脚本和采样器。正常启动段预热 15 秒后采样 35 秒；各功能段预热 3 秒后采样 25 秒。函数热点另采样 8 秒，使用 1 ms 的 V8 采样间隔。OS 场景均值不包含函数采样时间。`);
add('CPU 是按全部逻辑处理器归一化后的整机百分比，100% 表示全部逻辑处理器占满；例如 3% 在本机约为 0.36 个逻辑核。P95 和峰值来自约 1 秒的采样窗口，无法代表毫秒级瞬时峰值。MiB=1,048,576 字节。');
add('**独占驻留**是本应用当前独占的物理页，可跨进程相加；**提交内存**是私有已提交虚拟内存，不等于全部都在物理 RAM 中；**工作集合计**含进程间及系统共享页，跨进程求和会重复计数，不能视作本软件独占 RAM。这三列不能相加。GPU 进程内存也不等于专用显存；本次没有单独测量显存、电量或 GPU 硬件利用率。');
add('## 2. 当前占用');add(memoryTable([regular,...phases]));
add('正常启动段是日常待机参考。其余功能段启用了本机调试连接以控制界面，且在同一实例按顺序执行，素材、浏览器及驱动缓存会逐步预热；这些数值是各场景的实测运行状态，不能将相邻两行内存差值直接当作某一功能的净内存需求。音频段包含程序内生成的低音量 120 BPM 测试信号，信号发生器本身也带来少量开销。表情压力段每 0.8 秒切换一次，强于日常随机表情使用。');
add(`正常启动段进程数 ${f(n.processes.mean,0)}，线程均值 ${f(n.threads.mean,0)}，句柄均值 ${f(n.handles.mean,0)}；系统整体 CPU 均值 ${pct(n.systemCpu.mean)}。后台应用仍可能造成频率和调度波动，本次不是独占测试机上的基准排名。`);
add('正常启动段的 CPU 高于先前调试实例待机段，这说明实例预热、后台负载、焦点及鼠标所处位置可能影响数据；本次没有固定频率、清空所有其他应用或固定鼠标轨迹，不能据此认定开启调试会降低占用，也不能把某一个均值当作所有机器上的恒定值。');
add('### 正常待机的进程分解');
add(table(['进程类别（PID）','CPU 均值','独占驻留 MiB','提交 MiB','工作集 MiB'],[...n.perProcess].sort((a,b)=>b.privateWs-a.privateWs).map(p=>[`${roles.get(p.pid)||p.name}（${p.pid}）`,pct(p.cpu),mb(p.privateWs),mb(p.commit),mb(p.ws)])));
add(`## 3. 函数热点\n\n${table(['场景','函数','自耗采样时长 ms','占采样窗口','占非 idle 样本'],hotRows)}`);
add('这里的比例来自主页面 V8 采样栈，包含浏览器原生调用及等待；它不是整机 CPU 百分比，不能直接与上一节相减或换算为所有线程 CPU。inclusive 包含子调用，会重叠，本表只列 self。未采集 Rust 的 ETW 原生函数栈，因此对原生函数仅作源码审查，不将其伪称为已实测函数热点。生产脚本的临时 source map 已核对为同一压缩代码（仅多出 sourceMappingURL 注释），用于回溯源码，不替换发布包。');
add(`### 热点一：透明命中掩码\n\n${sourceLink('public/pet-hit-region.js','public/pet-hit-region.js',109)} 的 render 每次重画并读取整幅 540×590 RGBA 画布，然后逐像素扫描、创建行矩形数组、JSON 序列化并发送 Rust。单次 ImageData 数据量为 **1.22 MiB**，本次待机约 ${f(phases[1].summary.maskUpdatesPerSec)} 次掩码发送／秒、音频同步约 ${f(phases[5].summary.maskUpdatesPerSec)} 次／秒，对应约 **${f(540*590*4*phases[1].summary.maskUpdatesPerSec/M)}～${f(540*590*4*phases[5].summary.maskUpdatesPerSec/M)} MiB/s** 的像素数据量级。这是依据尺寸与发送次数计算的量级，不是实测内存带宽；未发送的重复计算会使实际开销更高。getImageData 是最明显的单项采样热点。`);
add(`JS 分配采样（不是存活内存）：待机 ${f(idle.sampledAllocationBytes/M)} MiB／8 秒，其中 render 约 ${f(idle.allocationTop.filter(n=>n.frame.functionName==='render').reduce((s,n)=>s+n.bytes,0)/M)} MiB；音频同步 ${f(audio.sampledAllocationBytes/M)} MiB／8 秒。采样估计可能遗漏或放大短命分配，且不覆盖全部 Canvas／纹理／驱动分配，不能将这些值解释为内存泄漏速度。`);
add('### 热点二：音频采样的编码、解码与频繁分配');
add(`${sourceLink('native/src/audio.rs','native/src/audio.rs',65)} 以 1024 点块发送 48 kHz 单声道浮点采样，每秒约 46.875 次；有效载荷 192,000 B/s（0.183 MiB/s）。${sourceLink('native/src/main.rs','native/src/main.rs',461)} 将其 Base64 化后经脚本送入网页，${sourceLink('src/audio/native-system.ts','src/audio/native-system.ts',45)} 再使用 atob、Uint8Array.from 和逐字符回调转换，最后传入 AudioWorklet。原生采样线程还为每块新建 Vec。数据总量很小，但跨层包装和短命对象值得优化。`);
add(`${sourceLink('src/audio/frozen-audio-core.js','src/audio/frozen-audio-core.js',3)} 的 estimate／自相关是次级计算热点；在本次测试信号下小于透明命中计算。${sourceLink('public/native-pcm-worklet.js','public/native-pcm-worklet.js',4)} 的固定环形缓冲仅 64 KiB，原生排队上限 3×4 KiB，它们本身解释不了数百 MiB 的进程内存。AudioWorklet 线程未包含在主页面函数采样中。`);
add('### 热点三：图形资源及浏览器常驻内存');
add(`当前透明版本预加载 ${assets.length} 张 800×800 PNG（不含停用的场景背景），压缩文件共 ${mb(assetPacked)} MiB，单份 RGBA 解码理论数据量共 **${mb(assetDecoded)} MiB**。休闲时页面实际挂载 12 张图，对应单份解码数据约 29.30 MiB；解码缓存、GPU 纹理、合成表面、Canvas 缓存和浏览器运行时另计，不能把 PNG 压缩体积当作运行内存。两倍系统缩放下的原生表面也更大。`);
add(`${sourceLink('src/character/assetLoader.ts','src/character/assetLoader.ts',160)} 使用 Promise.all 预加载所有表情；${sourceLink('public/pet-hit-region.js','public/pet-hit-region.js',39)} 再缓存多组完整尺寸 Canvas。这里有按实际显示区域裁紧素材、合并静态层和缩减多份图形表面的空间。仅凭 OS 工作集无法精确断言每张纹理占用了哪一块内存，故不把 WebView2 GPU 进程所有内存归因于素材。`);
add('## 4. 因果对照：停用命中计算后再恢复');
add(table(['同一实例顺序','CPU 均值','独占驻留均值 MiB','掩码发送／秒'],ab.map(p=>[p.summary.id,pct(p.summary.cpu.mean),mb(p.summary.privateWs.mean),f(p.summary.maskUpdatesPerSec)])));
add(`A/B/A 仅断开负责 body 子树变化的命中观察器，确认唯一命中该观察器；之后按原参数恢复。没有暂停角色动画、移除素材或改写节拍逻辑。两次正常段均值 ${pct(baseline)}，停用段 ${pct(off.cpu.mean)}，减少 ${f(baseline-off.cpu.mean,2)} 个整机 CPU 百分点，约 **${f(reduction)}%**。对照段都在强制回收后短暂预热，内存差异仍受缓存和分配器保留影响，不能将其直接视作最终可回收空间。`);
add('停用观察器后仍保留图片 load、resize 等触发，因此掩码发送不必完全为零；其频率已显著下降。');
add(`两次正常段自身也有波动，分别为 ${pct(before.cpu.mean)} 与 ${pct(after.cpu.mean)}；停用段相对它们的降幅范围为 ${f((1-off.cpu.mean/after.cpu.mean)*100)}%～${f((1-off.cpu.mean/before.cpu.mean)*100)}%。约 43% 是本次短窗口的定位结果，不能直接当作未来正式优化后的保证收益。`);
add('锁定整窗本已穿透，但当前实现仍在生成并发送精细掩码；隐藏控制台也只是隐藏原生窗口，未卸载 WebView2 和 React portal，因此不能期待隐藏后内存立即大幅下降。');
add('## 5. 是否存在泄漏');
add(table(['时间点（均手动触发 GC）','JS 活堆 MiB','backing storage MiB','DOM 文档／节点／监听器'],[first,last].map(d=>[d.label,mb(d.usage.usedSize),mb(d.usage.backingStorageSize),`${d.dom.documents}／${d.dom.nodes}／${d.dom.jsEventListeners}`])));
add(`执行计时、表情压力、音频后回到待机，JS 活堆从 ${mb(first.usage.usedSize)} 增至 ${mb(last.usage.usedSize)} MiB，约增加 ${mb(last.usage.usedSize-first.usage.usedSize)} MiB；文档数保持 2。音频停止后的独占驻留从运行音频时 ${mb(phases[5].summary.privateWs.mean)} 降至 ${mb(phases[6].summary.privateWs.mean)} MiB，说明相当一部分是可释放或可修剪的运行资源。`);
add('当前证据支持“图形／浏览器缓存与频繁临时分配造成高占用”，不足以确认持续泄漏，也不能据此保证没有泄漏。该测试只覆盖数分钟场景；确诊应进行 30～60 分钟相同状态的循环、记录每轮回收后的进程与堆基线，并补充图形资源归属分析。');
add('## 6. 建议优先级');
add(table(['优先级','建议','依据与验收方式'],[
 ['P0','锁定时停止精细命中更新；解锁前立即刷新一次','锁定已整窗穿透，当前仍算掩码；验证锁定／解锁与边缘穿透不倒退'],
 ['P0','保存静态 alpha／各刚性图层 alpha，在 Rust 或统一缓冲中按变换判定光标点，减少整画布 getImageData','A/B/A 已确认是首要 CPU 来源；必须保留半透明边缘、动态配件和内部透明空隙，不能回退到 SetWindowRgn 裁切画面'],
 ['P1','复用掩码矩形和消息缓冲，只在需要时发变化；避免每帧数百数组与 JSON','降低临时对象和跨层解析；对比 render 采样、GC 与实际 CPU'],
 ['P1','同时检查 Rust 侧 JSON 解析、每行 Vec 构建和窗口穿透样式切换','Rust 内存小但本次 CPU 并非可忽略；用原生采样进一步区分 IPC、解析和系统窗口调用'],
 ['P1','合并不动的素材层，离线裁紧透明边界，按需加载表情','保持现有视觉坐标与缩放；测量私有驻留和 GPU 进程内存，不能只看 JS 堆'],
 ['P2','PCM 改为可复用二进制消息／共享缓冲，减少 Base64 和逐字符回调','有效音频数据仅约 0.183 MiB/s；优化跨层开销但保持采样率与节拍输出'],
 ['P2','控制台隐藏时降低无须显示的刷新；评估延迟创建控制台','隐藏不释放 WebView2；保留共享计时、音频和状态，避免重复实例'],
 ['P3','再评估单独降低鼠标轮询频率或重写节拍算法','estimate 次于掩码；单独降低轮询对整体的收益尚未测量，并可能损害快速进入角色时的交互']
 ]));
add('以上是定位结论与建议，本次没有把诊断停用当作优化交付，也没有改动发布 EXE。每项优化都应在同一台机器、同样缩放与场景下复测，并运行透明穿透、UI 对齐和音频功能回归。');
add('## 7. 时间线与证据');
add(`浏览器时间线另采集 8 秒，按事件名汇总见下表。事件可能跨线程、嵌套，时长不能相加为 CPU；它用于定位布局、绘制和读回路径，不代表 GPU 硬件利用率。\n\n${table(['事件','次数','累计事件时长 ms'],(isolation.timelineTop||[]).filter(e=>!/RunTask|ThreadController|TaskQueue|RunLoop|Receive mojo|IPC|MessageLoop/.test(e.name)).slice(0,12).map(e=>[e.name,e.count,f(e.ms)]))}`);
add(`- [汇总 JSON](verification/performance-summary.json)、[每秒场景采样 CSV](verification/performance-samples.csv)。\n- [功能场景原始记录](verification/${runDir}/result.json)、[完整进程采样](verification/${runDir}/process-samples.jsonl)。\n- [待机函数采样](verification/${runDir}/idle_hidden.cpuprofile)、[音频函数采样](verification/${runDir}/audio_hidden.cpuprofile)、[音频分配采样](verification/${runDir}/audio_hidden.heapprofile)。\n- [A/B/A 原始记录](verification/${isolationDir}/result.json)、[浏览器时间线](verification/${isolationDir}/browser-timeline.json)。\n- [无调试正常启动记录](verification/${normalDir}/result.json)、[对应进程采样](verification/${normalDir}/process-samples.jsonl)。`);
add('方法备注：首轮 7 个功能场景均已完成；随后尝试通过页面重载插入诊断时，控制台未重新就绪，该重载后的数据全部排除。因果对照改为独立实例内直接断开／恢复观察器并完整完成。测试不宣称该诊断重载路径属于常规用户流程。');
add('复测脚本依次为 `node scripts/profile-runtime.mjs`、`node scripts/profile-isolation.mjs`、`node scripts/profile-normal.mjs`、`node scripts/report-performance.mjs`。运行脚本需要本机有桌面会话和读取自身子进程的权限；第一个脚本会短暂播放低音量测试节拍。临时 source map 可用 `node node_modules/vite/bin/vite.js build --outDir .tmp/performance-map --sourcemap` 生成。');
const out=path.join(root,'Rust版占用与热点测试报告.md');fs.writeFileSync(out,lines.join('\n\n')+'\n');
console.log(JSON.stringify({report:out,normal:n,ablation:{baseline,off:off.cpu.mean,reduction}},null,2));

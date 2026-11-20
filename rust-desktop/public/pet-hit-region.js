// Input-only alpha mask: this must never clip or repaint the WebView surface.
// Composite static rigs once; animation only moves their cached alpha surfaces.
const surface = document.createElement('canvas');
const context = surface.getContext('2d', { willReadFrequently:true });
const groups = new Map();
let scheduled = false, lastSent = '', pixels = null, updates = 0;

function localMatrix(element) {
  const style = getComputedStyle(element);
  const [ox, oy] = style.transformOrigin.split(' ').map(parseFloat);
  return new DOMMatrix().translate(element.offsetLeft, element.offsetTop)
    .translate(ox || 0, oy || 0)
    .multiply(style.transform === 'none' ? new DOMMatrix() : new DOMMatrix(style.transform))
    .translate(-(ox || 0), -(oy || 0));
}

function sceneMatrix(element, scene) {
  let matrix = new DOMMatrix();
  while (element && element !== scene) {
    matrix = localMatrix(element).multiply(matrix);
    element = element.offsetParent;
  }
  return matrix;
}

function transformed(ctx, matrix, draw) {
  ctx.save(); ctx.transform(matrix.a, matrix.b, matrix.c, matrix.d, matrix.e, matrix.f);
  draw(); ctx.restore();
}

function imageGroup(node, scene) {
  // CSS animated FX / timer glow need their own surface. A head/body rig's
  // static layers share one surface, without caching every source PNG again.
  const style = getComputedStyle(node);
  if (style.animationName !== 'none' || style.transform !== 'none') return node;
  return node.parentElement === scene ? scene : node.parentElement;
}

function drawGroup(node, images, width, height) {
  const states = images.map(image => ({ image, style:getComputedStyle(image) }));
  const animated = states.some(({image}) => /\.(?:gif|webp)(?:[?#]|$)/i.test(image.currentSrc));
  const signature = states.map(({image, style}) => [image.currentSrc, image.naturalWidth,
    image.naturalHeight, image.complete, style.opacity, style.filter].join(':')).join('|');
  let cached = groups.get(node);
  if (!cached || cached.width !== width || cached.height !== height) {
    const canvas = document.createElement('canvas'); canvas.width=width; canvas.height=height;
    cached = {canvas, ctx:canvas.getContext('2d'), width, height, signature:null};
    groups.set(node, cached);
  }
  if (animated || cached.signature !== signature) {
    const ctx = cached.ctx; ctx.clearRect(0,0,width,height);
    for (const {image, style} of states) {
      if (!image.complete || !image.naturalWidth || style.visibility === 'hidden' || style.display === 'none') continue;
      ctx.save(); ctx.globalAlpha=Number(style.opacity); ctx.filter=style.filter;
      const w=image.clientWidth, h=image.clientHeight;
      const scale=Math.min(w/image.naturalWidth,h/image.naturalHeight);
      const iw=image.naturalWidth*scale, ih=image.naturalHeight*scale;
      const x=node===image?0:image.offsetLeft, y=node===image?0:image.offsetTop;
      ctx.drawImage(image,x+(w-iw)/2,y+(h-ih)/2,iw,ih); ctx.restore();
    }
    cached.signature=signature;
  }
  return cached.canvas;
}

function paintScene(scene) {
  const rect=scene.getBoundingClientRect();
  const width=scene.clientWidth, height=scene.clientHeight;
  const current=new Map();
  for (const image of scene.querySelectorAll('img')) {
    const group=imageGroup(image,scene);
    if (!current.has(group)) current.set(group,[]);
    current.get(group).push(image);
  }
  for (const node of groups.keys()) if (!current.has(node)) groups.delete(node);
  context.save(); context.translate(rect.x,rect.y);
  context.beginPath(); context.rect(0,0,width,height); context.clip();
  // Alpha union is independent of the original z order.
  for (const [node,images] of current) {
    const canvas=drawGroup(node,images,width,height);
    transformed(context,sceneMatrix(node,scene),()=>context.drawImage(canvas,0,0));
  }
  const ring=scene.querySelector('svg circle');
  if (ring) {
    const style=getComputedStyle(ring), opacity=Number(style.opacity);
    if (opacity>0) {
      context.save(); context.scale(width/800,height/800); context.globalAlpha=opacity;
      context.strokeStyle=style.stroke; context.lineWidth=parseFloat(style.strokeWidth);
      context.setLineDash(style.strokeDasharray==='none'?[]:style.strokeDasharray.split(/[ ,]+/).map(parseFloat));
      context.lineDashOffset=parseFloat(style.strokeDashoffset)||0;
      context.beginPath(); context.arc(Number(ring.getAttribute('cx')),Number(ring.getAttribute('cy')),Number(ring.getAttribute('r')),0,Math.PI*2);
      context.stroke(); context.restore();
    }
  }
  for (const note of scene.querySelectorAll('[data-testid="beat-note-particle"]')) {
    const style=getComputedStyle(note);
    transformed(context,sceneMatrix(note,scene),()=>{
      context.globalAlpha=Number(style.opacity); context.fillStyle=style.color;
      context.font=style.font; context.filter=style.filter;
      const metrics=context.measureText(note.textContent);
      const ascent=metrics.fontBoundingBoxAscent || parseFloat(style.fontSize);
      const descent=metrics.fontBoundingBoxDescent || 0;
      context.fillText(note.textContent,0,(note.clientHeight+ascent-descent)/2);
    });
  }
  context.restore();
}

function render() {
  scheduled=false;
  const width=innerWidth, height=innerHeight;
  if (!width || !height || width>4096 || height>4096) return;
  if (surface.width!==width || surface.height!==height) { surface.width=width; surface.height=height; }
  context.clearRect(0,0,width,height);
  const scene=document.querySelector('[data-testid="character-scene"]');
  if (scene) paintScene(scene);
  for (const button of document.querySelectorAll('.pet-toolbar button')) {
    const rect=button.getBoundingClientRect(), style=getComputedStyle(button);
    context.fillStyle='#fff'; context.beginPath();
    context.roundRect(rect.x,rect.y,rect.width,rect.height,parseFloat(style.borderRadius)||0); context.fill();
  }
  pixels=context.getImageData(0,0,width,height).data;
  const rects=[];
  // Rust stores these intervals for native cursor hit testing only.
  for (let y=0;y<height;y++) {
    let start=-1;
    for (let x=0;x<=width;x++) {
      const opaque=x<width && pixels[(y*width+x)*4+3]>0;
      if (opaque && start<0) start=x;
      if (!opaque && start>=0) { rects.push([start,y,x,y+1]); start=-1; }
    }
  }
  const encoded=JSON.stringify({command:'hit_region',width,height,rects});
  if (encoded!==lastSent) { window.ipc.postMessage(encoded); lastSent=encoded; updates++; }
  window.__petHitRegion={width,height,rects,updates,alphaAt:(x,y)=>pixels[(Math.floor(y)*width+Math.floor(x))*4+3] || 0};
}

function schedule() { if (!scheduled) { scheduled=true; requestAnimationFrame(render); } }
new MutationObserver(schedule).observe(document.body,{subtree:true,childList:true,attributes:true,attributeFilter:['src','class','style']});
addEventListener('resize',schedule);
document.addEventListener('load',schedule,true);
schedule();

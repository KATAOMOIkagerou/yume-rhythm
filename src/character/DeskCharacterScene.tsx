import React, { useEffect, useMemo, useRef, useState } from 'react';
import { jh, m1 } from '../audio';
import type { DeskCharacterSceneProps, MotionFrame } from '../types/contracts';
import { useCharacterAssets } from './assetLoader';
import { useCharacterEvents } from './useCharacterEvents';

interface NoteParticle {
  id: number;
  leftPct: number;
  topPct: number;
  glyph: string;
  hue: number;
  driftX: number;
}

const NOTE_GLYPHS = ['♪', '♫', '♩', '♬', '✦'] as const;
let noteIdCounter = 0;

const LAYER_IMG_CLASS =
  'pointer-events-none absolute inset-0 h-full w-full select-none object-contain';

const SCENE_KEYFRAMES = `
@keyframes charNoteFloat {
  0% {
    opacity: 0;
    transform: translate3d(0px, 8px, 0px) scale(0.65) rotate(-8deg);
  }
  18% {
    opacity: 1;
    transform: translate3d(calc(var(--note-drift, 0px) * 0.25), -14px, 0px) scale(1.08) rotate(6deg);
  }
  100% {
    opacity: 0;
    transform: translate3d(var(--note-drift, 0px), -74px, 0px) scale(0.92) rotate(-10deg);
  }
}
@keyframes charFxFloat {
  0%, 100% {
    transform: translate3d(0px, 0px, 0px) scale(1);
  }
  50% {
    transform: translate3d(0px, -6px, 0px) scale(1.03);
  }
}
@keyframes charTimerGlow {
  0%, 100% {
    filter: drop-shadow(0 0 6px rgba(255, 162, 198, 0.45));
  }
  50% {
    filter: drop-shadow(0 0 15px rgba(255, 132, 184, 0.88));
  }
}
`;

export const DeskCharacterScene: React.FC<DeskCharacterSceneProps> = ({
  motionRef,
  musicRunning,
  autoFollow,
  activityMode,
  timerActive,
  emotion,
  eventSwitches,
  assetReloadToken,
  forcedEventPreview = null,
  expression,
  baseExpression,
  onExpressionsLoaded,
  onAssetStatus,
}) => {
  const { assets, error } = useCharacterAssets(assetReloadToken, onAssetStatus);
  useEffect(() => {
    if (assets) onExpressionsLoaded?.(assets.expressions);
  }, [assets, onExpressionsLoaded]);
  const { isBlinking, activeEvent } = useCharacterEvents({
    musicRunning,
    activityMode,
    emotion: baseExpression?.id === 'happy' ? 'happy' : emotion,
    eventSwitches,
    forcedEventPreview,
  });

  const bodyRef = useRef<HTMLDivElement | null>(null);
  const headRigRef = useRef<HTMLDivElement | null>(null);
  const beatRingRef = useRef<SVGCircleElement | null>(null);
  const accessoryRigRef = useRef<HTMLDivElement | null>(null);
  const [notes, setNotes] = useState<NoteParticle[]>([]);
  const noteTimersRef = useRef<Set<number>>(new Set());

  const manifest = assets?.manifest ?? null;
  const urls = assets?.urls ?? null;

  const animStateRef = useRef({
    musicRunning,
    autoFollow,
    isDozing: !musicRunning && activeEvent === 'doze',
    nodAmplitudeScale: manifest?.nodAmplitudeScale ?? 1,
    nodTiltScale: manifest?.nodTiltScale ?? 1,
  });

  animStateRef.current = {
    musicRunning,
    autoFollow,
    isDozing: !musicRunning && activeEvent === 'doze',
    nodAmplitudeScale: manifest?.nodAmplitudeScale ?? 1,
    nodTiltScale: manifest?.nodTiltScale ?? 1,
  };

  // Beat-following and idle breathing animation loop (v1 physics + locked m1)
  useEffect(() => {
    if (!manifest || !headRigRef.current) return;

    let headY = 0;
    let headVy = 0;
    let headRot = 0;
    let headVRot = 0;
    let scalePulse = 1;
    let scaleV = 0;
    let squashY = 1;
    let dozeBlend = 0;
    let hitStrength = 1.8;
    let ringProgress = 1;
    let prevBeats = (motionRef.current ?? jh).beats;
    let lastTime = performance.now();
    let rafId = 0;
    let unmounted = false;

    const spawnNote = () => {
      const id = ++noteIdCounter;
      const particle: NoteParticle = {
        id,
        leftPct: 30 + Math.random() * 40,
        topPct: 16 + Math.random() * 10,
        glyph: NOTE_GLYPHS[Math.floor(Math.random() * NOTE_GLYPHS.length)],
        hue: 190 + Math.random() * 130,
        driftX: (Math.random() - 0.5) * 34,
      };
      setNotes((prev) => [...prev.slice(-11), particle]);

      const timer = window.setTimeout(() => {
        noteTimersRef.current.delete(timer);
        if (!unmounted) {
          setNotes((prev) => prev.filter((item) => item.id !== id));
        }
      }, 1500);
      noteTimersRef.current.add(timer);
    };

    const tick = (now: number) => {
      rafId = requestAnimationFrame(tick);

      const dtMs = Math.min(48, Math.max(0, now - lastTime));
      lastTime = now;
      const dtNorm = dtMs / 16.667;

      const j: MotionFrame = motionRef.current ?? jh;
      const cfg = animStateRef.current;
      const idle = !(cfg.musicRunning && cfg.autoFollow);

      const beatTriggered = !idle && j.beats > prevBeats;
      prevBeats = j.beats;

      if (beatTriggered) {
        const strength = 0.85 + Math.min(2, j.strength) * 1.15;
        hitStrength = strength;
        headVy += strength * 1.2;
        headVRot += (Math.random() - 0.4) * strength * 0.6;
        scaleV += strength * 0.09;
        ringProgress = 0;
        if (Math.random() < 0.85) {
          spawnNote();
        }
      }

      const levelVal = idle && j.level <= 0.03 ? 0.1 : j.level;

      // Exact v1 vertical spring + m1 locked beat nod
      const dampY = Math.pow(0.86, dtNorm);
      headVy += -0.115 * headY * dtNorm;
      headVy *= dampY;
      headY += headVy * dtNorm;

      const lockedY =
        !idle && j.locked ? m1(now, j.nextBeatAt, j.periodMs, hitStrength) : null;
      if (lockedY !== null) {
        headY = lockedY;
        headVy = 0;
      }

      // Exact v1 tilt & bounce springs
      headVRot += -0.09 * headRot * dtNorm;
      headVRot *= Math.pow(0.9, dtNorm);
      headRot += headVRot * dtNorm;

      scaleV += -0.22 * (scalePulse - 1) * dtNorm;
      scaleV *= Math.pow(0.84, dtNorm);
      scalePulse = Math.max(0.6, Math.min(1.6, scalePulse + scaleV * dtNorm));

      const targetSquash = 1 - Math.max(-0.1, Math.min(0.1, headY * 0.004));
      squashY += (targetSquash - squashY) * Math.min(1, 0.25 * dtNorm);

      // Gentle idle breathing / music sway
      const breathY = Math.sin(now / 340) * (1.1 + levelVal * 2.6);
      const swayX = Math.sin(now / 520) * (1.3 + levelVal * 2.4);

      // Smooth non-music doze droop
      const targetDoze = idle && cfg.isDozing ? 1 : 0;
      dozeBlend += (targetDoze - dozeBlend) * Math.min(1, 0.06 * dtNorm);
      const dozeY = dozeBlend * (9.5 + Math.sin(now / 650) * 3.5);
      const dozeRot = dozeBlend * (-3.2 + Math.sin(now / 900) * 1.6);

      const nodVisualScale = 1.45;
      const finalHeadY =
        (headY * nodVisualScale * cfg.nodAmplitudeScale) + breathY + dozeY;
      const finalHeadRot = headRot * cfg.nodTiltScale + dozeRot;
      const pulseBoost = Math.max(0, scalePulse - 1) * 0.03;
      const headScaleX = 2 - squashY + pulseBoost;
      const headScaleY = squashY + pulseBoost;

      const headEl = headRigRef.current;
      if (headEl) {
        headEl.style.transform = `translate3d(0px, ${finalHeadY.toFixed(2)}px, 0px) rotate(${finalHeadRot.toFixed(2)}deg) scale(${headScaleX.toFixed(3)}, ${headScaleY.toFixed(3)})`;
        if (accessoryRigRef.current) accessoryRigRef.current.style.transform = headEl.style.transform;
      }

      const bodyEl = bodyRef.current;
      if (bodyEl) {
        const bodyX = swayX * 0.65;
        const bodyY =
          -headY * 0.22 * nodVisualScale * cfg.nodAmplitudeScale + breathY * 0.4;
        bodyEl.style.transform = `translate3d(${bodyX.toFixed(2)}px, ${bodyY.toFixed(2)}px, 0px) scale(${squashY.toFixed(3)}, ${(2 - squashY).toFixed(3)})`;
      }

      const ringEl = beatRingRef.current;
      if (ringEl) {
        if (ringProgress < 1) {
          ringProgress = Math.min(1, ringProgress + dtMs / 620);
          const eased = 1 - Math.pow(1 - ringProgress, 3);
          ringEl.setAttribute('r', (140 + eased * 185).toFixed(1));
          ringEl.style.opacity = ((1 - eased) * 0.42).toFixed(3);
          ringEl.style.strokeWidth = (12 * (1 - eased) + 1.5).toFixed(2);
        } else {
          ringEl.style.opacity = '0';
        }
      }
    };

    rafId = requestAnimationFrame(tick);

    return () => {
      unmounted = true;
      cancelAnimationFrame(rafId);
      noteTimersRef.current.forEach((timer) => window.clearTimeout(timer));
      noteTimersRef.current.clear();
    };
  }, [motionRef, manifest, baseExpression?.resources.animation]);

  const faceLayers = useMemo(() => {
    if (!manifest) return null;
    const base = baseExpression ?? (expression?.type === 'base' ? expression : null);
    const exprDef = base?.resources ?? manifest.expressions[emotion];
    const accessory = expression?.type === 'accessory' ? expression.resources : undefined;
    const eyesFile = accessory?.eyes ?? (isBlinking && !base?.resources.animation
      ? manifest.blinkEyes : exprDef.eyes ?? manifest.expressions.neutral.eyes);
    const mouthFile = accessory?.mouth ?? exprDef.mouth ?? manifest.expressions.neutral.mouth;
    return { eyesFile, mouthFile, fxFile: accessory?.overlay ?? accessory?.animation,
      baseFile: base?.resources.animation };
  }, [manifest, expression, baseExpression, emotion, isBlinking]);

  if (!manifest || !urls || !faceLayers) {
    return (
      <div
        data-testid="character-scene"
        className="relative flex aspect-square w-full items-center justify-center bg-transparent p-6 text-center text-sm text-purple-700/80"
      >
        {error ? (
          <div data-testid="character-asset-error" className="space-y-1.5">
            <div className="font-semibold text-rose-600">人物素材加载失败</div>
            <div className="text-xs opacity-80">{error}</div>
          </div>
        ) : (
          <div className="animate-pulse font-medium">正在加载二次元人物素材...</div>
        )}
      </div>
    );
  }

  const headOriginX = `${(manifest.headPivot.x / manifest.canvasWidth) * 100}%`;
  const headOriginY = `${(manifest.headPivot.y / manifest.canvasHeight) * 100}%`;
  const computerFile =
    activityMode === 'study' ? manifest.layers.computerOn : manifest.layers.computerOff;
  const armsFile =
    activityMode === 'study' ? manifest.layers.armsStudy : manifest.layers.armsChill;

  return (
    <div
      data-testid="character-scene"
      data-pack-name={manifest.packName}
      data-activity-mode={activityMode}
      data-music-running={String(musicRunning)}
      data-auto-follow={String(autoFollow)}
      data-timer-active={String(timerActive)}
      className="relative aspect-square w-full select-none overflow-hidden"
    >
      <style>{SCENE_KEYFRAMES}</style>

      {/* Beat pulse ring behind character */}
      <svg
        viewBox="0 0 800 800"
        aria-hidden="true"
        className="pointer-events-none absolute inset-0 h-full w-full"
      >
        <circle
          ref={beatRingRef}
          cx="400"
          cy="340"
          r="140"
          fill="none"
          stroke="#c084fc"
          strokeWidth="12"
          opacity="0"
        />
      </svg>

      {faceLayers.baseFile ? <div ref={headRigRef} className="absolute inset-0"
        data-testid="character-head-rig" data-emotion={expression?.id} data-expression-type="base"
        style={{ transformOrigin: `${headOriginX} ${headOriginY}` }}>
        <img src={urls[faceLayers.baseFile]} alt="" draggable={false}
          data-testid="layer-base-animation" className={LAYER_IMG_CLASS} />
      </div> : <>
      {/* 2) Chair back */}
      {urls[manifest.layers.chairBack] ? (
        <img
          src={urls[manifest.layers.chairBack]}
          alt=""
          draggable={false}
          data-testid="layer-chair-back"
          className={LAYER_IMG_CLASS}
        />
      ) : null}

      {/* 3) Body & neck base (subtle breathing + soft counter-recoil) */}
      <div
        ref={bodyRef}
        data-testid="character-body-rig"
        className="pointer-events-none absolute inset-0"
        style={{ transformOrigin: '50% 88%', willChange: 'transform' }}
      >
        {urls[manifest.layers.bodyBase] ? (
          <img
            src={urls[manifest.layers.bodyBase]}
            alt=""
            draggable={false}
            data-testid="layer-body-base"
            className={LAYER_IMG_CLASS}
          />
        ) : null}
      </div>

      {/* 4) Unified Head Rig: all head parts move & rotate 100% in sync */}
      <div
        ref={headRigRef}
        data-testid="character-head-rig"
        data-active-event={activeEvent ?? ''}
        data-emotion={expression?.id ?? emotion}
        data-expression-type={expression?.type ?? 'base'}
        data-blinking={String(isBlinking)}
        className="pointer-events-none absolute inset-0"
        style={{
          transformOrigin: `${headOriginX} ${headOriginY}`,
          willChange: 'transform',
        }}
      >
        {urls[manifest.layers.hairBack] ? (
          <img
            src={urls[manifest.layers.hairBack]}
            alt=""
            draggable={false}
            data-testid="layer-hair-back"
            className={LAYER_IMG_CLASS}
          />
        ) : null}

        {urls[manifest.layers.headBase] ? (
          <img
            src={urls[manifest.layers.headBase]}
            alt=""
            draggable={false}
            data-testid="layer-head-base"
            className={LAYER_IMG_CLASS}
          />
        ) : null}

        {urls[manifest.layers.blushBase] ? (
          <img
            src={urls[manifest.layers.blushBase]}
            alt=""
            draggable={false}
            data-testid="layer-blush-base"
            className={LAYER_IMG_CLASS}
          />
        ) : null}

        {/* Exactly ONE eyes layer */}
        {urls[faceLayers.eyesFile] ? (
          <img
            src={urls[faceLayers.eyesFile]}
            alt=""
            draggable={false}
            data-testid="layer-eyes"
            data-asset-file={faceLayers.eyesFile}
            className={LAYER_IMG_CLASS}
          />
        ) : null}

        {/* Exactly ONE mouth layer */}
        {urls[faceLayers.mouthFile] ? (
          <img
            src={urls[faceLayers.mouthFile]}
            alt=""
            draggable={false}
            data-testid="layer-mouth"
            data-asset-file={faceLayers.mouthFile}
            className={LAYER_IMG_CLASS}
          />
        ) : null}

        {urls[manifest.layers.hairFront] ? (
          <img
            src={urls[manifest.layers.hairFront]}
            alt=""
            draggable={false}
            data-testid="layer-hair-front"
            className={LAYER_IMG_CLASS}
          />
        ) : null}

        {/* Headphones appear only when music is running and enabled in manifest */}
        {musicRunning &&
        manifest.showMusicHeadphones &&
        urls[manifest.layers.headphones] ? (
          <img
            src={urls[manifest.layers.headphones]}
            alt=""
            draggable={false}
            data-testid="layer-headphones"
            className={LAYER_IMG_CLASS}
          />
        ) : null}


      </div>

      {/* 5) Desk surface */}
      {urls[manifest.layers.deskSurface] ? (
        <img
          src={urls[manifest.layers.deskSurface]}
          alt=""
          draggable={false}
          data-testid="layer-desk-surface"
          className={LAYER_IMG_CLASS}
        />
      ) : null}

      {/* 6) Computer prop (on in study mode, off in chill mode) */}
      {urls[computerFile] ? (
        <img
          src={urls[computerFile]}
          alt=""
          draggable={false}
          data-testid={activityMode === 'study' ? 'layer-computer-on' : 'layer-computer-off'}
          data-asset-file={computerFile}
          className={LAYER_IMG_CLASS}
        />
      ) : null}

      {/* 7) Books on the right */}
      {urls[manifest.layers.books] ? (
        <img
          src={urls[manifest.layers.books]}
          alt=""
          draggable={false}
          data-testid="layer-books"
          className={LAYER_IMG_CLASS}
        />
      ) : null}

      {/* 8) Timer clock (shown when timerActive with gentle breathing glow) */}
      {timerActive && urls[manifest.layers.timerClock] ? (
        <img
          src={urls[manifest.layers.timerClock]}
          alt=""
          draggable={false}
          data-testid="layer-timer-clock"
          className={LAYER_IMG_CLASS}
          style={{ animation: 'charTimerGlow 2.4s ease-in-out infinite' }}
        />
      ) : null}

      {/* 9) Arms foreground (study vs chill) */}
      {urls[armsFile] ? (
        <img
          src={urls[armsFile]}
          alt=""
          draggable={false}
          data-testid={activityMode === 'study' ? 'layer-arms-study' : 'layer-arms-chill'}
          data-asset-file={armsFile}
          className={LAYER_IMG_CLASS}
        />
      ) : null}

      </>}
        {/* Head event FX with gentle floating animation */}
        {faceLayers.fxFile && urls[faceLayers.fxFile] ? (
          <div ref={accessoryRigRef} className="pointer-events-none absolute inset-0"
            style={{ transformOrigin: `${headOriginX} ${headOriginY}`, willChange: 'transform' }}>
          <img
            src={urls[faceLayers.fxFile]}
            alt=""
            draggable={false}
            data-testid="layer-event-fx"
            data-asset-file={faceLayers.fxFile}
            className={LAYER_IMG_CLASS}
            style={{ animation: 'charFxFloat 2.2s ease-in-out infinite' }}
          />
          </div>
        ) : null}
      {/* Floating beat musical note particles */}
      {notes.map((note) => (
        <span
          key={note.id}
          aria-hidden="true"
          data-testid="beat-note-particle"
          className="pointer-events-none absolute text-2xl font-bold drop-shadow-[0_2px_6px_rgba(255,255,255,0.85)]"
          style={
            {
              left: `${note.leftPct.toFixed(1)}%`,
              top: `${note.topPct.toFixed(1)}%`,
              color: `hsl(${note.hue.toFixed(0)} 88% 68%)`,
              '--note-drift': `${note.driftX.toFixed(1)}px`,
              animation: 'charNoteFloat 1.5s cubic-bezier(0.22, 1, 0.36, 1) forwards',
            } as React.CSSProperties
          }
        >
          {note.glyph}
        </span>
      ))}
    </div>
  );
};

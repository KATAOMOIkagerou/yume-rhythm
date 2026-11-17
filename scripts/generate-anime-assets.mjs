import { Resvg } from '@resvg/resvg-js';
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT_DIR = path.resolve(__dirname, '..');
const OUT_DIR = path.join(ROOT_DIR, 'public', 'character_assets');

const LINE = '#4A3246';
const LINE_SOFT = '#6D4963';
const LINE_HAIR = '#603B50';

const SHARED_DEFS = `
  <defs>
    <!-- Skin Gradients -->
    <linearGradient id="skinGrad" x1="0.5" y1="0" x2="0.5" y2="1">
      <stop offset="0%" stop-color="#FFF8F3"/>
      <stop offset="58%" stop-color="#FFEBE2"/>
      <stop offset="100%" stop-color="#FFE0D7"/>
    </linearGradient>
    <linearGradient id="neckGrad" x1="0.5" y1="0" x2="0.5" y2="1">
      <stop offset="0%" stop-color="#F1BCB5"/>
      <stop offset="48%" stop-color="#FAD5CB"/>
      <stop offset="100%" stop-color="#FFEAE0"/>
    </linearGradient>
    <radialGradient id="cheekWarmth" cx="0.5" cy="0.5" r="0.5">
      <stop offset="0%" stop-color="#FF9CB2" stop-opacity="0.28"/>
      <stop offset="100%" stop-color="#FF9CB2" stop-opacity="0"/>
    </radialGradient>

    <!-- Hair Gradients (Milk-tea Sakura Peach) -->
    <linearGradient id="hairFrontGrad" x1="0.3" y1="0" x2="0.7" y2="1">
      <stop offset="0%" stop-color="#FCE4E2"/>
      <stop offset="42%" stop-color="#ECAAB2"/>
      <stop offset="85%" stop-color="#D78698"/>
      <stop offset="100%" stop-color="#BE6D81"/>
    </linearGradient>
    <linearGradient id="hairTwinGrad" x1="0.2" y1="0" x2="0.8" y2="1">
      <stop offset="0%" stop-color="#F8CBCD"/>
      <stop offset="45%" stop-color="#E69EA8"/>
      <stop offset="100%" stop-color="#C26E82"/>
    </linearGradient>
    <linearGradient id="hairBackGrad" x1="0.5" y1="0" x2="0.5" y2="1">
      <stop offset="0%" stop-color="#E29AA5"/>
      <stop offset="55%" stop-color="#CA798B"/>
      <stop offset="100%" stop-color="#A65A6F"/>
    </linearGradient>
    <linearGradient id="hairHaloGrad" x1="0" y1="0" x2="1" y2="0">
      <stop offset="0%" stop-color="#FFF9FA" stop-opacity="0.1"/>
      <stop offset="28%" stop-color="#FFF9FA" stop-opacity="0.85"/>
      <stop offset="50%" stop-color="#FFFFFF" stop-opacity="0.94"/>
      <stop offset="72%" stop-color="#FFF9FA" stop-opacity="0.85"/>
      <stop offset="100%" stop-color="#FFF9FA" stop-opacity="0.1"/>
    </linearGradient>

    <!-- Eye Iris Gradients (Violet -> Sakura Pink -> Amber Gold) -->
    <linearGradient id="irisGrad" x1="0.5" y1="0" x2="0.5" y2="1">
      <stop offset="0%" stop-color="#4A2C70"/>
      <stop offset="32%" stop-color="#8A52B5"/>
      <stop offset="68%" stop-color="#EF86B1"/>
      <stop offset="90%" stop-color="#FFC87A"/>
      <stop offset="100%" stop-color="#FFE8A3"/>
    </linearGradient>
    <linearGradient id="irisSadGrad" x1="0.5" y1="0" x2="0.5" y2="1">
      <stop offset="0%" stop-color="#4A3678"/>
      <stop offset="36%" stop-color="#8A64BC"/>
      <stop offset="72%" stop-color="#DDA2CC"/>
      <stop offset="100%" stop-color="#FCE0AE"/>
    </linearGradient>

    <!-- Blush Gradient -->
    <radialGradient id="blushRadial" cx="0.5" cy="0.5" r="0.5">
      <stop offset="0%" stop-color="#FF7F9F" stop-opacity="0.64"/>
      <stop offset="55%" stop-color="#FF97B1" stop-opacity="0.34"/>
      <stop offset="100%" stop-color="#FF97B1" stop-opacity="0"/>
    </radialGradient>

    <!-- Outfit Gradients (Strawberry Cream Cardigan + Sailor Collar) -->
    <linearGradient id="cardiganGrad" x1="0.5" y1="0" x2="0.5" y2="1">
      <stop offset="0%" stop-color="#FFE5ED"/>
      <stop offset="52%" stop-color="#FFD2DF"/>
      <stop offset="100%" stop-color="#F7B7C8"/>
    </linearGradient>
    <linearGradient id="sleeveGrad" x1="0" y1="0" x2="0.8" y2="1">
      <stop offset="0%" stop-color="#FFE7EF"/>
      <stop offset="60%" stop-color="#FFD2DF"/>
      <stop offset="100%" stop-color="#F4B2C4"/>
    </linearGradient>
    <linearGradient id="collarGrad" x1="0.5" y1="0" x2="0.5" y2="1">
      <stop offset="0%" stop-color="#FFFFFF"/>
      <stop offset="100%" stop-color="#FFF0F5"/>
    </linearGradient>
    <linearGradient id="ribbonGrad" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#FF8DA9"/>
      <stop offset="55%" stop-color="#EB6687"/>
      <stop offset="100%" stop-color="#C84A6B"/>
    </linearGradient>

    <!-- Headphones Gradients -->
    <linearGradient id="hpBandGrad" x1="0" y1="0" x2="1" y2="0.3">
      <stop offset="0%" stop-color="#D9B7FF"/>
      <stop offset="50%" stop-color="#F7D5EE"/>
      <stop offset="100%" stop-color="#FFB6CE"/>
    </linearGradient>
    <linearGradient id="hpCupGrad" x1="0.2" y1="0" x2="0.8" y2="1">
      <stop offset="0%" stop-color="#F6E2FF"/>
      <stop offset="52%" stop-color="#E7C2FC"/>
      <stop offset="100%" stop-color="#FFA9C6"/>
    </linearGradient>

    <!-- Desk & Props Gradients -->
    <linearGradient id="deskTopGrad" x1="0.5" y1="0" x2="0.5" y2="1">
      <stop offset="0%" stop-color="#FFF5EA"/>
      <stop offset="65%" stop-color="#FCE4D2"/>
      <stop offset="100%" stop-color="#F5D0BC"/>
    </linearGradient>
    <linearGradient id="deskFrontGrad" x1="0.5" y1="0" x2="0.5" y2="1">
      <stop offset="0%" stop-color="#ECC1AE"/>
      <stop offset="100%" stop-color="#DCA896"/>
    </linearGradient>
    <linearGradient id="deskMatGrad" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#FFF7FA"/>
      <stop offset="100%" stop-color="#FFE6EF"/>
    </linearGradient>
    <linearGradient id="laptopBodyGrad" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#F7F2FF"/>
      <stop offset="60%" stop-color="#E6DCF7"/>
      <stop offset="100%" stop-color="#CFC2E8"/>
    </linearGradient>
    <linearGradient id="laptopScreenGlow" x1="0.2" y1="0" x2="0.8" y2="1">
      <stop offset="0%" stop-color="#6B8CFF"/>
      <stop offset="48%" stop-color="#9F8CFF"/>
      <stop offset="100%" stop-color="#DFD7FF"/>
    </linearGradient>
    <radialGradient id="screenAura" cx="0.5" cy="0.5" r="0.5">
      <stop offset="0%" stop-color="#8BA8FF" stop-opacity="0.55"/>
      <stop offset="55%" stop-color="#A68EFF" stop-opacity="0.22"/>
      <stop offset="100%" stop-color="#A68EFF" stop-opacity="0"/>
    </radialGradient>

    <!-- Scene Backdrop Gradients -->
    <radialGradient id="bgAuraGrad" cx="0.5" cy="0.48" r="0.48">
      <stop offset="0%" stop-color="#FFF7FA" stop-opacity="0.96"/>
      <stop offset="62%" stop-color="#F7ECFA" stop-opacity="0.9"/>
      <stop offset="84%" stop-color="#F2E2F8" stop-opacity="0.45"/>
      <stop offset="100%" stop-color="#F2E2F8" stop-opacity="0"/>
    </radialGradient>
    <linearGradient id="windowSkyGrad" x1="0.5" y1="0" x2="0.5" y2="1">
      <stop offset="0%" stop-color="#CFE7FF"/>
      <stop offset="55%" stop-color="#E7E6FF"/>
      <stop offset="100%" stop-color="#FFE8F0"/>
    </linearGradient>
    <linearGradient id="curtainGrad" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#FFF8FA" stop-opacity="0.95"/>
      <stop offset="100%" stop-color="#F9DCEB" stop-opacity="0.9"/>
    </linearGradient>

    <filter id="tinyGlow" x="-20%" y="-20%" width="140%" height="140%">
      <feGaussianBlur stdDeviation="3.5"/>
    </filter>
  </defs>
`;

function wrapSvg(body) {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="800" height="800" viewBox="0 0 800 800">${SHARED_DEFS}${body}</svg>`;
}

// ---------- Helpers for cute anime eyes ----------
function drawSingleIris(cx, cy, rx = 16, ry = 20, gradId = 'irisGrad', dx = 0, dy = 0) {
  const ix = cx + dx;
  const iy = cy + dy;
  return `
    <g>
      <ellipse cx="${ix}" cy="${iy}" rx="${rx}" ry="${ry}" fill="url(#${gradId})" stroke="${LINE}" stroke-width="1.5"/>
      <!-- Upper iris deep shadow -->
      <path d="M ${ix - rx} ${iy - 2} A ${rx} ${ry} 0 0 1 ${ix + rx} ${iy - 2} C ${ix + rx * 0.6} ${iy - ry * 0.35}, ${ix - rx * 0.6} ${iy - ry * 0.35}, ${ix - rx} ${iy - 2} Z" fill="#371D4E" opacity="0.48"/>
      <!-- Pupil -->
      <ellipse cx="${ix}" cy="${iy + 1}" rx="${rx * 0.43}" ry="${ry * 0.44}" fill="#331B45"/>
      <!-- Lower amber glow crescent -->
      <path d="M ${ix - rx * 0.68} ${iy + ry * 0.35} C ${ix - rx * 0.3} ${iy + ry * 0.85}, ${ix + rx * 0.3} ${iy + ry * 0.85}, ${ix + rx * 0.68} ${iy + ry * 0.35} C ${ix + rx * 0.3} ${iy + ry * 0.58}, ${ix - rx * 0.3} ${iy + ry * 0.58}, ${ix - rx * 0.68} ${iy + ry * 0.35} Z" fill="#FFEAA7" opacity="0.74"/>
      <!-- Glossy anime catchlights -->
      <ellipse cx="${ix - rx * 0.38}" cy="${iy - ry * 0.38}" rx="${rx * 0.35}" ry="${ry * 0.31}" transform="rotate(-18 ${ix - rx * 0.38} ${iy - ry * 0.38})" fill="#FFFFFF"/>
      <circle cx="${ix + rx * 0.38}" cy="${iy + ry * 0.38}" r="${rx * 0.17}" fill="#FFFFFF" opacity="0.95"/>
      <circle cx="${ix - rx * 0.48}" cy="${iy + ry * 0.25}" r="${rx * 0.1}" fill="#FFFFFF" opacity="0.88"/>
      <circle cx="${ix + rx * 0.32}" cy="${iy - ry * 0.35}" r="${rx * 0.08}" fill="#FFF5D6" opacity="0.9"/>
    </g>
  `;
}

function buildEyesNeutral() {
  const drawEye = (side) => {
    const isLeft = side === 'left';
    const cx = isLeft ? 357 : 443;
    const s = isLeft ? 1 : -1;
    const clipId = `clip_eye_n_${side}`;
    return `
      <defs>
        <clipPath id="${clipId}">
          <path d="M ${cx - 24} ${367} C ${cx - 22} ${342}, ${cx + 22} ${342}, ${cx + 24} ${367} C ${cx + 23} ${390}, ${cx - 23} ${390}, ${cx - 24} ${367} Z"/>
        </clipPath>
      </defs>
      <g>
        <!-- Sclera -->
        <path d="M ${cx - 24} ${367} C ${cx - 22} ${342}, ${cx + 22} ${342}, ${cx + 24} ${367} C ${cx + 23} ${390}, ${cx - 23} ${390}, ${cx - 24} ${367} Z" fill="#FFFDFB"/>
        <g clip-path="url(#${clipId})">
          ${drawSingleIris(cx, 369, 16.2, 20.2, 'irisGrad')}
          <path d="M ${cx - 27} ${341} L ${cx + 27} ${341} L ${cx + 27} ${355} C ${cx + 12} ${349}, ${cx - 12} ${349}, ${cx - 27} ${355} Z" fill="#E3D7EE" opacity="0.56"/>
        </g>
        <!-- Upper eyelash & outer wing curls -->
        <path d="M ${cx - 25 * s} ${366} C ${cx - 22 * s} ${342}, ${cx + 19 * s} ${341}, ${cx + 25 * s} ${365}" fill="none" stroke="${LINE}" stroke-width="4.4" stroke-linecap="round"/>
        <path d="M ${cx - 24 * s} ${362} C ${cx - 28 * s} ${358}, ${cx - 31 * s} ${354}, ${cx - 31 * s} ${354}" fill="none" stroke="${LINE}" stroke-width="2.4" stroke-linecap="round"/>
        <path d="M ${cx - 20 * s} ${352} C ${cx - 25 * s} ${348}, ${cx - 27 * s} ${345}, ${cx - 27 * s} ${345}" fill="none" stroke="${LINE}" stroke-width="1.8" stroke-linecap="round"/>
        <!-- Double eyelid fold -->
        <path d="M ${cx - 19 * s} ${341} C ${cx - 8 * s} ${334}, ${cx + 10 * s} ${334}, ${cx + 19 * s} ${342}" fill="none" stroke="#9B6E87" stroke-width="1.35" stroke-linecap="round" opacity="0.72"/>
        <!-- Lower lash -->
        <path d="M ${cx - 16 * s} ${387} C ${cx - 5 * s} ${391}, ${cx + 6 * s} ${391}, ${cx + 15 * s} ${387}" fill="none" stroke="${LINE_SOFT}" stroke-width="1.65" stroke-linecap="round"/>
        <!-- Eyebrow (clearly visible below bangs arch!) -->
        <path d="M ${cx - 21 * s} ${329} C ${cx - 8 * s} ${322}, ${cx + 9 * s} ${323}, ${cx + 19 * s} ${329}" fill="none" stroke="#623B52" stroke-width="2.4" stroke-linecap="round"/>
      </g>
    `;
  };
  return drawEye('left') + drawEye('right');
}

function buildEyesBlink() {
  const drawClosed = (side) => {
    const isLeft = side === 'left';
    const cx = isLeft ? 357 : 443;
    const s = isLeft ? 1 : -1;
    return `
      <g>
        <path d="M ${cx - 24 * s} ${371} C ${cx - 12 * s} ${379}, ${cx + 12 * s} ${379}, ${cx + 22 * s} ${371}" fill="none" stroke="${LINE}" stroke-width="3.9" stroke-linecap="round"/>
        <path d="M ${cx - 23 * s} ${371} L ${cx - 31 * s} ${367}" fill="none" stroke="${LINE}" stroke-width="2.3" stroke-linecap="round"/>
        <path d="M ${cx - 19 * s} ${375} L ${cx - 25 * s} ${375}" fill="none" stroke="${LINE}" stroke-width="1.7" stroke-linecap="round"/>
        <path d="M ${cx - 17 * s} ${362} C ${cx - 6 * s} ${358}, ${cx + 8 * s} ${358}, ${cx + 17 * s} ${363}" fill="none" stroke="#9B6E87" stroke-width="1.2" stroke-linecap="round" opacity="0.6"/>
        <path d="M ${cx - 20 * s} ${334} C ${cx - 7 * s} ${329}, ${cx + 8 * s} ${329}, ${cx + 18 * s} ${334}" fill="none" stroke="#623B52" stroke-width="2.2" stroke-linecap="round"/>
      </g>
    `;
  };
  return drawClosed('left') + drawClosed('right');
}

function buildEyesHappy() {
  const drawHappy = (side) => {
    const isLeft = side === 'left';
    const cx = isLeft ? 357 : 443;
    const s = isLeft ? 1 : -1;
    return `
      <g>
        <!-- Joyful upturned smiling crescent eyes -->
        <path d="M ${cx - 23 * s} ${375} C ${cx - 14 * s} ${351}, ${cx + 14 * s} ${351}, ${cx + 22 * s} ${374}" fill="none" stroke="${LINE}" stroke-width="4.5" stroke-linecap="round"/>
        <path d="M ${cx - 21 * s} ${373} C ${cx - 12 * s} ${355}, ${cx + 12 * s} ${355}, ${cx + 19 * s} ${372}" fill="none" stroke="#A65E7E" stroke-width="1.4" stroke-linecap="round" opacity="0.45"/>
        <path d="M ${cx - 22 * s} ${370} L ${cx - 31 * s} ${363}" fill="none" stroke="${LINE}" stroke-width="2.4" stroke-linecap="round"/>
        <path d="M ${cx - 18 * s} ${363} L ${cx - 26 * s} ${355}" fill="none" stroke="${LINE}" stroke-width="1.8" stroke-linecap="round"/>
        <!-- Tiny clean sparkle near outer corner (not overlapping ear/blush) -->
        ${vectorSparkle(cx - 28 * s, 348, 5.5, '#FFE07D')}
        <!-- Cheerful raised eyebrow -->
        <path d="M ${cx - 21 * s} ${325} C ${cx - 8 * s} ${317}, ${cx + 9 * s} ${318}, ${cx + 19 * s} ${325}" fill="none" stroke="#623B52" stroke-width="2.4" stroke-linecap="round"/>
      </g>
    `;
  };
  return drawHappy('left') + drawHappy('right');
}

function buildEyesSad() {
  const drawSad = (side) => {
    const isLeft = side === 'left';
    const cx = isLeft ? 357 : 443;
    const s = isLeft ? 1 : -1;
    const clipId = `clip_eye_sad_${side}`;
    return `
      <defs>
        <clipPath id="${clipId}">
          <path d="M ${cx - 23} ${370} C ${cx - 20} ${350}, ${cx + 20} ${350}, ${cx + 23} ${370} C ${cx + 22} ${390}, ${cx - 22} ${390}, ${cx - 23} ${370} Z"/>
        </clipPath>
      </defs>
      <g>
        <path d="M ${cx - 23} ${370} C ${cx - 20} ${350}, ${cx + 20} ${350}, ${cx + 23} ${370} C ${cx + 22} ${390}, ${cx - 22} ${390}, ${cx - 23} ${370} Z" fill="#FFFDFB"/>
        <g clip-path="url(#${clipId})">
          ${drawSingleIris(cx, 372, 15, 18, 'irisSadGrad', 0, 1)}
          <path d="M ${cx - 25} ${346} L ${cx + 25} ${346} L ${cx + 25} ${360} C ${cx + 10} ${354}, ${cx - 10} ${354}, ${cx - 25} ${360} Z" fill="#DED4EC" opacity="0.65"/>
          <!-- Shimmering tear pool -->
          <path d="M ${cx - 21} ${379} C ${cx - 10} ${384}, ${cx + 10} ${384}, ${cx + 21} ${379} L ${cx + 21} ${392} L ${cx - 21} ${392} Z" fill="#B8E3FF" opacity="0.85"/>
          <ellipse cx="${cx - 6}" cy="${384}" rx="4.5" ry="1.8" fill="#FFFFFF" opacity="0.92"/>
          <ellipse cx="${cx + 7}" cy="${384}" rx="3.2" ry="1.4" fill="#FFFFFF" opacity="0.88"/>
        </g>
        <!-- Tear droplet -->
        <path d="M ${cx - 19 * s} ${385} C ${cx - 25 * s} ${389}, ${cx - 25 * s} ${398}, ${cx - 19 * s} ${398} C ${cx - 14 * s} ${398}, ${cx - 14 * s} ${389}, ${cx - 19 * s} ${385} Z" fill="#BEE7FF" stroke="#75A8CE" stroke-width="1.1"/>
        <circle cx="${cx - 20 * s}" cy="${392}" r="1.4" fill="#FFFFFF"/>
        <path d="M ${cx - 24 * s} ${372} C ${cx - 18 * s} ${351}, ${cx + 13 * s} ${347}, ${cx + 23 * s} ${363}" fill="none" stroke="${LINE}" stroke-width="4.2" stroke-linecap="round"/>
        <path d="M ${cx - 17 * s} ${387} C ${cx - 5 * s} ${391}, ${cx + 6 * s} ${391}, ${cx + 16 * s} ${387}" fill="none" stroke="${LINE_SOFT}" stroke-width="1.6" stroke-linecap="round"/>
        <!-- Worried tilted eyebrows clearly visible below bangs arch -->
        <path d="M ${cx - 20 * s} ${334} C ${cx - 8 * s} ${331}, ${cx + 8 * s} ${326}, ${cx + 19 * s} ${321}" fill="none" stroke="#623B52" stroke-width="2.4" stroke-linecap="round"/>
      </g>
    `;
  };
  return drawSad('left') + drawSad('right');
}

function buildEyesDoze() {
  const drawDoze = (side) => {
    const isLeft = side === 'left';
    const cx = isLeft ? 357 : 443;
    const s = isLeft ? 1 : -1;
    const clipId = `clip_eye_doze_${side}`;
    return `
      <defs>
        <clipPath id="${clipId}">
          <path d="M ${cx - 23} ${369} C ${cx - 14} ${364}, ${cx + 14} ${364}, ${cx + 23} ${369} C ${cx + 21} ${386}, ${cx - 21} ${386}, ${cx - 23} ${369} Z"/>
        </clipPath>
      </defs>
      <g>
        <path d="M ${cx - 23} ${369} C ${cx - 14} ${364}, ${cx + 14} ${364}, ${cx + 23} ${369} C ${cx + 21} ${386}, ${cx - 21} ${386}, ${cx - 23} ${369} Z" fill="#FFFDFB"/>
        <g clip-path="url(#${clipId})">
          ${drawSingleIris(cx, 374, 15, 16.5, 'irisGrad')}
          <path d="M ${cx - 25} ${360} L ${cx + 25} ${360} L ${cx + 25} ${373} L ${cx - 25} ${373} Z" fill="#E3D7EE" opacity="0.5"/>
        </g>
        <path d="M ${cx - 24} ${368} C ${cx - 14} ${350}, ${cx + 14} ${350}, ${cx + 24} ${368} C ${cx + 13} ${365}, ${cx - 13} ${365}, ${cx - 24} ${368} Z" fill="#FCE2D8" opacity="0.9"/>
        <path d="M ${cx - 24 * s} ${369} C ${cx - 12 * s} ${365}, ${cx + 12 * s} ${365}, ${cx + 22 * s} ${369}" fill="none" stroke="${LINE}" stroke-width="4.1" stroke-linecap="round"/>
        <path d="M ${cx - 23 * s} ${369} L ${cx - 29 * s} ${366}" fill="none" stroke="${LINE}" stroke-width="2" stroke-linecap="round"/>
        <path d="M ${cx - 16 * s} ${385} C ${cx - 5 * s} ${388}, ${cx + 5 * s} ${388}, ${cx + 15 * s} ${385}" fill="none" stroke="${LINE_SOFT}" stroke-width="1.4" stroke-linecap="round"/>
        <path d="M ${cx - 20 * s} ${339} C ${cx - 8 * s} ${335}, ${cx + 8 * s} ${336}, ${cx + 18 * s} ${340}" fill="none" stroke="#623B52" stroke-width="2.2" stroke-linecap="round"/>
      </g>
    `;
  };
  return drawDoze('left') + drawDoze('right');
}

function buildEyesThink() {
  const drawThink = (side) => {
    const isLeft = side === 'left';
    const cx = isLeft ? 357 : 443;
    const s = isLeft ? 1 : -1;
    const clipId = `clip_eye_think_${side}`;
    const browY = isLeft ? 321 : 331;
    const browCurve = isLeft
      ? `M ${cx - 20} ${browY + 4} C ${cx - 8} ${browY - 5}, ${cx + 8} ${browY - 4}, ${cx + 19} ${browY + 2}`
      : `M ${cx - 18} ${browY + 2} C ${cx - 6} ${browY - 1}, ${cx + 8} ${browY - 1}, ${cx + 20} ${browY}`;
    return `
      <defs>
        <clipPath id="${clipId}">
          <path d="M ${cx - 23} ${368} C ${cx - 21} ${346}, ${cx + 21} ${346}, ${cx + 23} ${368} C ${cx + 22} ${389}, ${cx - 22} ${389}, ${cx - 23} ${368} Z"/>
        </clipPath>
      </defs>
      <g>
        <path d="M ${cx - 23} ${368} C ${cx - 21} ${346}, ${cx + 21} ${346}, ${cx + 23} ${368} C ${cx + 22} ${389}, ${cx - 22} ${389}, ${cx - 23} ${368} Z" fill="#FFFDFB"/>
        <g clip-path="url(#${clipId})">
          ${drawSingleIris(cx, 368, 15, 19, 'irisGrad', 4.5, -4)}
          <path d="M ${cx - 26} ${343} L ${cx + 26} ${343} L ${cx + 26} ${355} C ${cx + 12} ${349}, ${cx - 12} ${349}, ${cx - 26} ${355} Z" fill="#E3D7EE" opacity="0.52"/>
        </g>
        <path d="M ${cx - 24 * s} ${367} C ${cx - 20 * s} ${346}, ${cx + 18 * s} ${345}, ${cx + 23 * s} ${366}" fill="none" stroke="${LINE}" stroke-width="4.1" stroke-linecap="round"/>
        <path d="M ${cx - 23 * s} ${363} L ${cx - 30 * s} ${356}" fill="none" stroke="${LINE}" stroke-width="2.2" stroke-linecap="round"/>
        <path d="M ${cx - 16 * s} ${387} C ${cx - 5 * s} ${390}, ${cx + 6 * s} ${390}, ${cx + 15 * s} ${387}" fill="none" stroke="${LINE_SOFT}" stroke-width="1.6" stroke-linecap="round"/>
        <path d="${browCurve}" fill="none" stroke="#623B52" stroke-width="2.4" stroke-linecap="round"/>
      </g>
    `;
  };
  return drawThink('left') + drawThink('right');
}

function buildEyesShock() {
  const drawShock = (side) => {
    const isLeft = side === 'left';
    const cx = isLeft ? 357 : 443;
    const s = isLeft ? 1 : -1;
    const clipId = `clip_eye_shock_${side}`;
    return `
      <defs>
        <clipPath id="${clipId}">
          <path d="M ${cx - 24} ${367} C ${cx - 23} ${340}, ${cx + 23} ${340}, ${cx + 24} ${367} C ${cx + 23} ${393}, ${cx - 23} ${393}, ${cx - 24} ${367} Z"/>
        </clipPath>
      </defs>
      <g>
        <path d="M ${cx - 24} ${367} C ${cx - 23} ${340}, ${cx + 23} ${340}, ${cx + 24} ${367} C ${cx + 23} ${393}, ${cx - 23} ${393}, ${cx - 24} ${367} Z" fill="#FFFDFB"/>
        <g clip-path="url(#${clipId})">
          <ellipse cx="${cx}" cy="${368}" rx="10.8" ry="13.8" fill="url(#irisGrad)" stroke="${LINE}" stroke-width="1.6"/>
          <ellipse cx="${cx}" cy="${368}" rx="6.4" ry="8.2" fill="none" stroke="#FFF4FA" stroke-width="1.4" opacity="0.85"/>
          <ellipse cx="${cx}" cy="${368.5}" rx="3.4" ry="4.4" fill="#331B45"/>
          <circle cx="${cx - 4}" cy="${363}" r="3.2" fill="#FFFFFF"/>
          <circle cx="${cx + 4}" cy="${373}" r="1.8" fill="#FFFFFF"/>
        </g>
        <path d="M ${cx - 24 * s} ${367} C ${cx - 23 * s} ${341}, ${cx + 23 * s} ${341}, ${cx + 24 * s} ${367}" fill="none" stroke="${LINE}" stroke-width="4.2" stroke-linecap="round"/>
        <path d="M ${cx - 18 * s} ${388} C ${cx - 6 * s} ${393}, ${cx + 6 * s} ${393}, ${cx + 18 * s} ${388}" fill="none" stroke="${LINE_SOFT}" stroke-width="1.8" stroke-linecap="round"/>
        <!-- High shocked eyebrows clearly framed above eyes -->
        <path d="M ${cx - 20 * s} ${324} C ${cx - 8 * s} ${314}, ${cx + 9 * s} ${315}, ${cx + 19 * s} ${324}" fill="none" stroke="#623B52" stroke-width="2.4" stroke-linecap="round"/>
      </g>
    `;
  };
  return drawShock('left') + drawShock('right');
}

// ---------- Vector FX symbols ----------
function vectorHeart(cx, cy, size, fill, stroke = LINE, rotateDeg = 0) {
  const s = size;
  return `
    <g transform="translate(${cx} ${cy}) rotate(${rotateDeg}) scale(${s})">
      <path d="M 0 10 C -12 2, -18 -7, -11 -15 C -6 -20, 0 -14, 0 -9 C 0 -14, 6 -20, 11 -15 C 18 -7, 12 2, 0 10 Z" fill="${fill}" stroke="${stroke}" stroke-width="${2.2 / s}" stroke-linejoin="round"/>
      <ellipse cx="-6.5" cy="-10.5" rx="2.8" ry="1.5" transform="rotate(-30 -6.5 -10.5)" fill="#FFFFFF" opacity="0.88"/>
    </g>
  `;
}

function vectorSparkle(cx, cy, r, fill = '#FFE37F') {
  return `
    <path d="M ${cx} ${cy - r} Q ${cx + r * 0.16} ${cy - r * 0.16} ${cx + r} ${cy} Q ${cx + r * 0.16} ${cy + r * 0.16} ${cx} ${cy + r} Q ${cx - r * 0.16} ${cy + r * 0.16} ${cx - r} ${cy} Q ${cx - r * 0.16} ${cy - r * 0.16} ${cx} ${cy - r} Z" fill="${fill}" stroke="#E3976B" stroke-width="0.9"/>
  `;
}

function vectorStar(cx, cy, rOuter, rInner, fill, stroke = LINE, rotDeg = 0) {
  const pts = [];
  for (let i = 0; i < 10; i++) {
    const r = i % 2 === 0 ? rOuter : rInner;
    const ang = ((i * 36 - 90) * Math.PI) / 180;
    pts.push(`${(Math.cos(ang) * r).toFixed(2)},${(Math.sin(ang) * r).toFixed(2)}`);
  }
  return `
    <g transform="translate(${cx} ${cy}) rotate(${rotDeg})">
      <polygon points="${pts.join(' ')}" fill="${fill}" stroke="${stroke}" stroke-width="2" stroke-linejoin="round"/>
      <circle cx="${-rOuter * 0.22}" cy="${-rOuter * 0.25}" r="${rOuter * 0.16}" fill="#FFFFFF" opacity="0.85"/>
    </g>
  `;
}

function vectorEighthNote(cx, cy, scale, fill, stroke = LINE, rotDeg = 0) {
  return `
    <g transform="translate(${cx} ${cy}) rotate(${rotDeg}) scale(${scale})">
      <path d="M 3 -14 L 3 7 C 3 12, -4 15, -9 13 C -14 11, -14 4, -8 2 C -4 0.5, 0 2, 1 4 L 1 -15 C 6 -13, 12 -10, 12 -3 C 10 -6, 6 -8, 3 -9 Z" fill="${fill}" stroke="${stroke}" stroke-width="${2.1 / scale}" stroke-linejoin="round" stroke-linecap="round"/>
      <ellipse cx="-6.5" cy="6.2" rx="2.6" ry="1.6" transform="rotate(-24 -6.5 6.2)" fill="#FFFFFF" opacity="0.78"/>
    </g>
  `;
}

// =========================================================================
// 32 INDIVIDUAL LAYER GENERATORS
// =========================================================================

const LAYER_BUILDERS = {
  // 1. scene_bg.png: Cozy sunlit pastel anime study window vignette (transparent corners!)
  'scene_bg.png': () => `
    <g>
      <ellipse cx="400" cy="405" rx="355" ry="335" fill="url(#bgAuraGrad)"/>
      <g opacity="0.92">
        <rect x="178" y="110" width="444" height="445" rx="36" fill="url(#windowSkyGrad)" stroke="#E3CFE8" stroke-width="5"/>
        <path d="M 210 250 C 225 225, 265 225, 280 245 C 296 226, 342 232, 348 258 L 210 258 Z" fill="#FFFFFF" opacity="0.78"/>
        <path d="M 468 206 C 482 184, 520 184, 535 202 C 550 186, 592 192, 598 214 L 468 214 Z" fill="#FFFFFF" opacity="0.75"/>
        <line x1="400" y1="112" x2="400" y2="552" stroke="#F6EBF8" stroke-width="7"/>
        <line x1="180" y1="308" x2="620" y2="308" stroke="#F6EBF8" stroke-width="7"/>
        <rect x="178" y="110" width="444" height="445" rx="36" fill="none" stroke="#E0C8E6" stroke-width="4"/>
      </g>
      <path d="M 164 106 C 218 110, 268 106, 268 106 C 254 205, 238 295, 210 356 C 232 418, 250 485, 254 555 L 164 555 Z" fill="url(#curtainGrad)" stroke="#D9BCD4" stroke-width="2.4" stroke-linejoin="round"/>
      <path d="M 192 110 C 194 210, 186 295, 198 355" fill="none" stroke="#E8CBE0" stroke-width="2" opacity="0.7"/>
      <path d="M 222 110 C 216 208, 206 290, 204 352" fill="none" stroke="#E8CBE0" stroke-width="2" opacity="0.7"/>
      <path d="M 176 355 Q 196 364 214 354" fill="none" stroke="#EC9DB8" stroke-width="6" stroke-linecap="round"/>

      <path d="M 636 106 C 582 110, 532 106, 532 106 C 546 205, 562 295, 590 356 C 568 418, 550 485, 546 555 L 636 555 Z" fill="url(#curtainGrad)" stroke="#D9BCD4" stroke-width="2.4" stroke-linejoin="round"/>
      <path d="M 608 110 C 606 210, 614 295, 602 355" fill="none" stroke="#E8CBE0" stroke-width="2" opacity="0.7"/>
      <path d="M 578 110 C 584 208, 594 290, 596 352" fill="none" stroke="#E8CBE0" stroke-width="2" opacity="0.7"/>
      <path d="M 624 355 Q 604 364 586 354" fill="none" stroke="#EC9DB8" stroke-width="6" stroke-linecap="round"/>

      <path d="M 162 128 Q 280 172 400 135 Q 520 172 638 128" fill="none" stroke="#C8A8C2" stroke-width="2.2"/>
      ${[
        [205, 142], [268, 154], [335, 148], [400, 136], [465, 148], [532, 154], [595, 142],
      ]
        .map(
          ([x, y]) => `
        <circle cx="${x}" cy="${y + 4}" r="11" fill="#FFE69A" opacity="0.45" filter="url(#tinyGlow)"/>
        <circle cx="${x}" cy="${y + 4}" r="5.2" fill="#FFF7CC" stroke="#E3B375" stroke-width="1.3"/>
      `,
        )
        .join('')}

      ${vectorSparkle(250, 190, 7, '#FFF7CC')}
      ${vectorSparkle(552, 178, 8, '#FFF7CC')}
      ${vectorSparkle(155, 290, 6, '#FFD2E4')}
      ${vectorSparkle(646, 295, 6, '#FFD2E4')}
    </g>
  `,

  // 2. chair_back.png
  'chair_back.png': () => `
    <g>
      <path d="M 278 398 C 276 364, 304 348, 344 348 L 456 348 C 496 348, 524 364, 522 398 L 534 558 C 534 574, 520 582, 496 582 L 304 582 C 280 582, 266 574, 266 558 Z" fill="#FFE8F0" stroke="${LINE}" stroke-width="3.2" stroke-linejoin="round"/>
      <path d="M 294 405 C 294 378, 316 365, 350 365 L 450 365 C 484 365, 506 378, 506 405 L 515 552 L 285 552 Z" fill="#FFD3E1" stroke="#E8A9BE" stroke-width="2" opacity="0.72"/>
      <path d="M 304 362 C 334 354, 466 354, 496 362" fill="none" stroke="#FFFFFF" stroke-width="4" stroke-linecap="round" opacity="0.78"/>
    </g>
  `,

  // 3. body_base.png: Neck overlapping y=406..475 (42px+ behind chin y=448!) + Sailor collar + Ribbon + Cardigan
  'body_base.png': () => `
    <g>
      <!-- Neck extending up to y=406 behind head_base -->
      <path d="M 375 406 L 425 406 L 426 456 C 427 468, 444 474, 460 478 L 340 478 C 356 474, 373 468, 374 456 Z" fill="url(#neckGrad)" stroke="${LINE}" stroke-width="3" stroke-linejoin="round"/>
      <path d="M 376 408 L 424 408 L 424 446 C 414 460, 386 460, 376 446 Z" fill="#EEB6AF" opacity="0.68"/>
      <path d="M 366 480 Q 382 486 392 484" fill="none" stroke="#D99892" stroke-width="1.6" stroke-linecap="round"/>
      <path d="M 434 480 Q 418 486 408 484" fill="none" stroke="#D99892" stroke-width="1.6" stroke-linecap="round"/>

      <!-- Cardigan Torso & Upper Shoulders -->
      <path d="M 354 472 C 324 478, 306 495, 298 532 L 292 595 L 508 595 L 502 532 C 494 495, 476 478, 446 472 C 430 480, 370 480, 354 472 Z" fill="url(#cardiganGrad)" stroke="${LINE}" stroke-width="3.4" stroke-linejoin="round"/>

      <!-- Crisp white blouse center V -->
      <path d="M 362 474 L 438 474 L 418 595 L 382 595 Z" fill="#FFFDFC" stroke="${LINE_SOFT}" stroke-width="2.2" stroke-linejoin="round"/>

      <!-- Cardigan front placket folds & pearl buttons -->
      <path d="M 360 482 L 383 594" fill="none" stroke="${LINE}" stroke-width="2.4" stroke-linecap="round"/>
      <path d="M 440 482 L 417 594" fill="none" stroke="${LINE}" stroke-width="2.4" stroke-linecap="round"/>
      <circle cx="385" cy="544" r="4.2" fill="#FFF6FA" stroke="#D58FA6" stroke-width="1.5"/>
      <circle cx="389" cy="566" r="4.2" fill="#FFF6FA" stroke="#D58FA6" stroke-width="1.5"/>

      <!-- Subtle knit lines & shoulder highlights -->
      <path d="M 326 520 C 324 548, 322 572, 322 588" fill="none" stroke="#F0A5BA" stroke-width="1.8" stroke-linecap="round" opacity="0.65"/>
      <path d="M 474 520 C 476 548, 478 572, 478 588" fill="none" stroke="#F0A5BA" stroke-width="1.8" stroke-linecap="round" opacity="0.65"/>
      <path d="M 316 496 C 330 484, 344 479, 354 477" fill="none" stroke="#FFF6F9" stroke-width="3.2" stroke-linecap="round" opacity="0.8"/>
      <path d="M 484 496 C 470 484, 456 479, 446 477" fill="none" stroke="#FFF6F9" stroke-width="3.2" stroke-linecap="round" opacity="0.8"/>

      <!-- Cream Sailor Collar -->
      <path d="M 358 469 C 344 473, 330 483, 325 499 L 368 525 L 400 505 L 432 525 L 475 499 C 470 483, 456 473, 442 469 C 430 486, 412 498, 400 505 C 388 498, 370 486, 358 469 Z" fill="url(#collarGrad)" stroke="${LINE}" stroke-width="2.8" stroke-linejoin="round"/>
      <path d="M 332 496 L 368 518 L 395 501" fill="none" stroke="#C58FB2" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/>
      <path d="M 468 496 L 432 518 L 405 501" fill="none" stroke="#C58FB2" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/>
      <path d="M 336 490 L 368 511" fill="none" stroke="#F59BB7" stroke-width="1.5" stroke-linecap="round"/>
      <path d="M 464 490 L 432 511" fill="none" stroke="#F59BB7" stroke-width="1.5" stroke-linecap="round"/>

      <!-- Cherry-Strawberry Sailor Bow -->
      <g>
        <path d="M 396 508 L 380 544 L 393 538 L 400 512 Z" fill="url(#ribbonGrad)" stroke="${LINE}" stroke-width="2.2" stroke-linejoin="round"/>
        <path d="M 404 508 L 420 544 L 407 538 L 400 512 Z" fill="url(#ribbonGrad)" stroke="${LINE}" stroke-width="2.2" stroke-linejoin="round"/>
        <path d="M 395 504 C 382 490, 365 493, 366 506 C 367 518, 383 517, 396 509 Z" fill="url(#ribbonGrad)" stroke="${LINE}" stroke-width="2.4" stroke-linejoin="round"/>
        <path d="M 405 504 C 418 490, 435 493, 434 506 C 433 518, 417 517, 404 509 Z" fill="url(#ribbonGrad)" stroke="${LINE}" stroke-width="2.4" stroke-linejoin="round"/>
        <path d="M 374 505 L 391 506" fill="none" stroke="#B63C5D" stroke-width="1.6" stroke-linecap="round"/>
        <path d="M 426 505 L 409 506" fill="none" stroke="#B63C5D" stroke-width="1.6" stroke-linecap="round"/>
        <rect x="393" y="499" width="14" height="14" rx="5" fill="#FF7E9E" stroke="${LINE}" stroke-width="2.3"/>
        <ellipse cx="398" cy="503" rx="3" ry="1.6" fill="#FFFFFF" opacity="0.65"/>
      </g>
    </g>
  `,

  // 4. hair_back.png: Tapered natural nape + fluffy short twin-tails with ribbon ties clear of headphones!
  'hair_back.png': () => `
    <g>
      <!-- Left fluffy short twin-tail -->
      <g>
        <path d="M 294 316 C 254 310, 226 346, 222 396 C 218 434, 232 460, 250 468 C 248 453, 252 444, 258 440 C 258 453, 267 464, 280 469 C 280 456, 286 447, 294 441 C 304 449, 312 445, 316 428 C 317 386, 309 342, 294 316 Z" fill="url(#hairTwinGrad)" stroke="${LINE}" stroke-width="3.2" stroke-linejoin="round"/>
        <path d="M 282 334 C 254 362, 245 404, 254 442" fill="none" stroke="${LINE_HAIR}" stroke-width="1.8" stroke-linecap="round" opacity="0.58"/>
        <path d="M 292 342 C 275 378, 274 416, 282 451" fill="none" stroke="${LINE_HAIR}" stroke-width="1.6" stroke-linecap="round" opacity="0.52"/>
        <path d="M 268 348 C 250 370, 245 390, 247 408" fill="none" stroke="#FFF2F4" stroke-width="4" stroke-linecap="round" opacity="0.5"/>
      </g>

      <!-- Right fluffy short twin-tail -->
      <g>
        <path d="M 506 316 C 546 310, 574 346, 578 396 C 582 434, 568 460, 550 468 C 552 453, 548 444, 542 440 C 542 453, 533 464, 520 469 C 520 456, 514 447, 506 441 C 496 449, 488 445, 484 428 C 483 386, 491 342, 506 316 Z" fill="url(#hairTwinGrad)" stroke="${LINE}" stroke-width="3.2" stroke-linejoin="round"/>
        <path d="M 518 334 C 546 362, 555 404, 546 442" fill="none" stroke="${LINE_HAIR}" stroke-width="1.8" stroke-linecap="round" opacity="0.58"/>
        <path d="M 508 342 C 525 378, 526 416, 518 451" fill="none" stroke="${LINE_HAIR}" stroke-width="1.6" stroke-linecap="round" opacity="0.52"/>
        <path d="M 532 348 C 550 370, 555 390, 553 408" fill="none" stroke="#FFF2F4" stroke-width="4" stroke-linecap="round" opacity="0.5"/>
      </g>

      <!-- Tapered bob back hair hugging behind ears & tapering neatly at nape (NO boxy block below chin!) -->
      <path d="M 295 344
        C 294 252, 342 222, 400 222
        C 458 222, 506 252, 505 344
        C 507 394, 504 428, 494 448
        C 488 458, 476 456, 468 444
        C 454 424, 428 424, 400 424
        C 372 424, 346 424, 332 444
        C 324 456, 312 458, 306 448
        C 296 428, 293 394, 295 344 Z"
        fill="url(#hairBackGrad)" stroke="${LINE}" stroke-width="3.1" stroke-linejoin="round"/>

      <!-- Cute soft nape wispy curls tucked cleanly beside jawline (well above collar!) -->
      <path d="M 316 402 C 318 426, 330 444, 354 446 C 340 434, 336 420, 336 404 Z" fill="#A95D73" opacity="0.72"/>
      <path d="M 484 402 C 482 426, 470 444, 446 446 C 460 434, 464 420, 464 404 Z" fill="#A95D73" opacity="0.72"/>

      <!-- Cute Twin-tail Ribbon Ties positioned slightly outward so headphones never cover them -->
      <g>
        <path d="M 284 316 C 273 304, 263 310, 268 320 C 272 327, 281 324, 285 320 Z" fill="#FF87A7" stroke="${LINE}" stroke-width="2.1"/>
        <path d="M 287 317 C 296 307, 304 313, 300 322 C 296 328, 288 324, 286 320 Z" fill="#FF87A7" stroke="${LINE}" stroke-width="2.1"/>
        <circle cx="286" cy="319" r="4.6" fill="#FFE0EC" stroke="${LINE}" stroke-width="2"/>

        <path d="M 516 316 C 527 304, 537 310, 532 320 C 528 327, 519 324, 515 320 Z" fill="#FF87A7" stroke="${LINE}" stroke-width="2.1"/>
        <path d="M 513 317 C 504 307, 496 313, 500 322 C 504 328, 512 324, 514 320 Z" fill="#FF87A7" stroke="${LINE}" stroke-width="2.1"/>
        <circle cx="514" cy="319" r="4.6" fill="#FFE0EC" stroke="${LINE}" stroke-width="2"/>
      </g>
    </g>
  `,

  // 5. head_base.png: PURE FACELESS HEAD BASE (Refined moe anime chin curve, 3D skin shading & ears ONLY!)
  'head_base.png': () => `
    <g>
      <!-- Left & Right Cute Ears -->
      <path d="M 317 363 C 301 360, 295 377, 301 396 C 305 407, 315 407, 321 398 Z" fill="url(#skinGrad)" stroke="${LINE}" stroke-width="2.8" stroke-linejoin="round"/>
      <path d="M 313 374 C 304 375, 303 390, 313 395" fill="none" stroke="#E0A59D" stroke-width="2.2" stroke-linecap="round"/>

      <path d="M 483 363 C 499 360, 505 377, 499 396 C 495 407, 485 407, 479 398 Z" fill="url(#skinGrad)" stroke="${LINE}" stroke-width="2.8" stroke-linejoin="round"/>
      <path d="M 487 374 C 496 375, 497 390, 487 395" fill="none" stroke="#E0A59D" stroke-width="2.2" stroke-linecap="round"/>

      <!-- Refined Moe Anime Face Silhouette (softly tapered cheeks & dainty rounded chin at y=448) -->
      <path d="M 315 344 C 315 252, 485 252, 485 344 C 485 388, 472 419, 440 437 C 420 447, 408 448, 400 448 C 392 448, 380 447, 360 437 C 328 419, 315 388, 315 344 Z" fill="url(#skinGrad)" stroke="${LINE}" stroke-width="3.2" stroke-linejoin="round"/>

      <!-- Soft Skin Cel Shading -->
      <path d="M 317 328 C 319 256, 481 256, 483 328 C 468 312, 446 310, 434 319 C 420 310, 408 314, 400 329 C 392 314, 380 310, 366 319 C 354 310, 332 312, 317 328 Z" fill="#F6C8BF" opacity="0.54"/>
      <path d="M 320 390 C 328 417, 353 436, 386 444 C 357 433, 335 415, 326 390 Z" fill="#F7C7C0" opacity="0.48"/>
      <path d="M 480 390 C 472 417, 447 436, 414 444 C 443 433, 465 415, 474 390 Z" fill="#F7C7C0" opacity="0.48"/>
      <ellipse cx="335" cy="396" rx="27" ry="15" fill="url(#cheekWarmth)"/>
      <ellipse cx="465" cy="396" rx="27" ry="15" fill="url(#cheekWarmth)"/>
      <!-- Soft 3D nose shading only (no dark line, zero eyebrows/eyes/mouth!) -->
      <path d="M 399 390 C 397 394, 398 397, 401.5 397.5 C 403 396.5, 402 392, 399 390 Z" fill="#E9B0A9" opacity="0.72"/>
      <circle cx="399" cy="392.5" r="1.4" fill="#FFFFFF" opacity="0.72"/>
    </g>
  `,

  // 6. blush_base.png
  'blush_base.png': () => `
    <g>
      <ellipse cx="333" cy="396" rx="24" ry="11.5" fill="url(#blushRadial)"/>
      <ellipse cx="467" cy="396" rx="24" ry="11.5" fill="url(#blushRadial)"/>
      <g stroke="#FF6B8E" stroke-width="1.6" stroke-linecap="round" opacity="0.55">
        <line x1="323" y1="399" x2="328" y2="393"/>
        <line x1="331" y1="400" x2="336" y2="394"/>
        <line x1="339" y1="399" x2="344" y2="393"/>
        <line x1="456" y1="399" x2="461" y2="393"/>
        <line x1="464" y1="400" x2="469" y2="394"/>
        <line x1="472" y1="399" x2="477" y2="393"/>
      </g>
      <ellipse cx="329" cy="393" rx="3.4" ry="1.8" fill="#FFFFFF" opacity="0.74"/>
      <ellipse cx="463" cy="393" rx="3.4" ry="1.8" fill="#FFFFFF" opacity="0.74"/>
    </g>
  `,

  // 7. hair_front.png: Wispy bangs raised neatly above brows + cute ahoge + angel halo + star & eighth-note hairpins
  'hair_front.png': () => `
    <g>
      <!-- Cheek-framing hime side locks curving smoothly beside cheeks (seamless root merge!) -->
      <path d="M 318 300 C 296 346, 296 416, 316 458 C 321 428, 323 380, 326 334 Z" fill="url(#hairFrontGrad)" stroke="${LINE}" stroke-width="2.9" stroke-linejoin="round"/>
      <path d="M 482 300 C 504 346, 504 416, 484 458 C 479 428, 477 380, 474 334 Z" fill="url(#hairFrontGrad)" stroke="${LINE}" stroke-width="2.9" stroke-linejoin="round"/>

      <!-- Crown & Wispy Feathered Bangs (higher arch over brows y=306..316 so ALL eyebrow expressions stay 100% crisp!) -->
      <path d="M 299 354
        C 296 260, 342 224, 400 224
        C 458 224, 504 260, 501 354
        C 495 335, 488 318, 480 307
        C 476 322, 470 330, 464 336
        C 463 318, 456 306, 444 303
        C 439 312, 433 320, 428 326
        C 424 313, 418 306, 411 304
        C 409 324, 405 340, 400 350
        C 395 340, 391 324, 389 304
        C 382 306, 376 313, 372 326
        C 367 320, 361 312, 356 303
        C 344 306, 337 318, 336 336
        C 330 330, 324 322, 320 307
        C 312 318, 305 335, 299 354 Z"
        fill="url(#hairFrontGrad)" stroke="${LINE}" stroke-width="3.2" stroke-linejoin="round"/>

      <!-- Cute Ahoge sitting slightly taller so it peeks playfully over headphones too -->
      <path d="M 398 225 C 390 198, 414 180, 434 196 C 422 195, 411 208, 406 226 Z" fill="url(#hairFrontGrad)" stroke="${LINE}" stroke-width="2.7" stroke-linejoin="round"/>

      <!-- Delicate inner hair strand curves -->
      <g fill="none" stroke="${LINE_HAIR}" stroke-width="1.6" stroke-linecap="round" opacity="0.58">
        <path d="M 342 254 C 330 278, 325 296, 324 316"/>
        <path d="M 366 242 C 357 268, 354 288, 356 303"/>
        <path d="M 392 238 C 388 264, 388 286, 389 304"/>
        <path d="M 408 238 C 412 264, 412 286, 411 304"/>
        <path d="M 434 242 C 443 268, 446 288, 444 303"/>
        <path d="M 458 254 C 470 278, 475 296, 476 316"/>
        <path d="M 311 352 C 308 386, 309 416, 315 442"/>
        <path d="M 489 352 C 492 386, 491 416, 485 442"/>
      </g>

      <!-- Anime Angel Halo Highlight Arc -->
      <g opacity="0.92">
        <path d="M 326 278 C 344 262, 366 258, 386 262 C 378 270, 366 273, 354 274 C 344 276, 334 283, 326 278 Z" fill="url(#hairHaloGrad)"/>
        <path d="M 394 262 C 400 261, 406 261, 410 262 C 407 270, 397 270, 394 262 Z" fill="#FFFFFF" opacity="0.86"/>
        <path d="M 416 262 C 436 258, 458 262, 474 278 C 466 283, 456 276, 446 274 C 434 273, 422 270, 416 262 Z" fill="url(#hairHaloGrad)"/>
        <ellipse cx="348" cy="266" rx="8" ry="2.6" transform="rotate(-12 348 266)" fill="#FFFFFF" opacity="0.72"/>
        <ellipse cx="452" cy="266" rx="8" ry="2.6" transform="rotate(12 452 266)" fill="#FFFFFF" opacity="0.72"/>
      </g>

      <!-- Star Hairpin & Eighth-Note Hairpin -->
      <g>
        <rect x="319" y="287" width="24" height="6" rx="3" transform="rotate(-16 331 290)" fill="#FF92B3" stroke="${LINE}" stroke-width="1.8"/>
        ${vectorStar(335, 288, 11.2, 5.1, '#FFE07D', LINE, -12)}
        <rect x="456" y="286" width="24" height="6" rx="3" transform="rotate(16 468 289)" fill="#C9A8FF" stroke="${LINE}" stroke-width="1.8"/>
        ${vectorEighthNote(464, 287, 0.92, '#FFB0D0', LINE, 10)}
      </g>
    </g>
  `,

  // 8. head_headphones.png: Sleek over-ear anime headphones arcing gracefully over crown
  'head_headphones.png': () => `
    <g>
      <path d="M 294 344 C 293 238, 344 206, 400 206 C 456 206, 507 238, 506 344" fill="none" stroke="${LINE}" stroke-width="16" stroke-linecap="round"/>
      <path d="M 294 344 C 293 238, 344 206, 400 206 C 456 206, 507 238, 506 344" fill="none" stroke="url(#hpBandGrad)" stroke-width="10.5" stroke-linecap="round"/>
      <path d="M 332 222 C 362 210, 438 210, 468 222" fill="none" stroke="#FFF6FA" stroke-width="3.8" stroke-linecap="round" opacity="0.88"/>

      <!-- Left Earcup -->
      <g>
        <rect x="284" y="340" width="24" height="66" rx="12" fill="#FFF4F8" stroke="${LINE}" stroke-width="2.8"/>
        <rect x="275" y="344" width="28" height="58" rx="14" fill="url(#hpCupGrad)" stroke="${LINE}" stroke-width="3"/>
        <ellipse cx="289" cy="373" rx="8.5" ry="18.5" fill="#FFF8FC" stroke="#C598DE" stroke-width="1.6"/>
        ${vectorEighthNote(289, 373, 0.54, '#FF7DA6', LINE, -4)}
        <ellipse cx="284" cy="355" rx="3" ry="7" fill="#FFFFFF" opacity="0.72"/>
      </g>

      <!-- Right Earcup -->
      <g>
        <rect x="492" y="340" width="24" height="66" rx="12" fill="#FFF4F8" stroke="${LINE}" stroke-width="2.8"/>
        <rect x="497" y="344" width="28" height="58" rx="14" fill="url(#hpCupGrad)" stroke="${LINE}" stroke-width="3"/>
        <ellipse cx="511" cy="373" rx="8.5" ry="18.5" fill="#FFF8FC" stroke="#C598DE" stroke-width="1.6"/>
        ${vectorEighthNote(511, 373, 0.54, '#FF7DA6', LINE, 4)}
        <ellipse cx="516" cy="355" rx="3" ry="7" fill="#FFFFFF" opacity="0.72"/>
      </g>
    </g>
  `,

  // 9-15. Mutually-exclusive Eye Layers
  'eyes_neutral.png': buildEyesNeutral,
  'eyes_blink.png': buildEyesBlink,
  'eyes_happy.png': buildEyesHappy,
  'eyes_sad.png': buildEyesSad,
  'eyes_doze.png': buildEyesDoze,
  'eyes_think.png': buildEyesThink,
  'eyes_shock.png': buildEyesShock,

  // 16-21. Mutually-exclusive Mouth Layers
  'mouth_neutral.png': () => `
    <g>
      <path d="M 392 415 Q 400 421 408 415" fill="none" stroke="${LINE}" stroke-width="2.4" stroke-linecap="round"/>
      <ellipse cx="400" cy="422" rx="3.2" ry="1.2" fill="#F2ACA6" opacity="0.55"/>
    </g>
  `,
  'mouth_happy.png': () => `
    <g>
      <path d="M 388 411 Q 400 414 412 411 C 412 427, 405 431, 400 431 C 395 431, 388 427, 388 411 Z" fill="#D55E78" stroke="${LINE}" stroke-width="2.3" stroke-linejoin="round"/>
      <path d="M 392 423 C 396 419, 404 419, 408 423 C 406 428, 403 430, 400 430 C 397 430, 394 428, 392 423 Z" fill="#FF9CB2"/>
      <path d="M 392 412.5 Q 400 414.5 408 412.5" fill="none" stroke="#FFFFFF" stroke-width="2" stroke-linecap="round"/>
    </g>
  `,
  'mouth_sad.png': () => `
    <g>
      <path d="M 391 420 C 395 414, 405 414, 409 420" fill="none" stroke="${LINE}" stroke-width="2.4" stroke-linecap="round"/>
      <ellipse cx="400" cy="424" rx="3" ry="1.1" fill="#F2ACA6" opacity="0.5"/>
    </g>
  `,
  'mouth_doze.png': () => `
    <g>
      <ellipse cx="400" cy="418" rx="5.2" ry="6.2" fill="#E37B92" stroke="${LINE}" stroke-width="2.2"/>
      <ellipse cx="400" cy="420.5" rx="3.2" ry="2.4" fill="#FFA4B7"/>
    </g>
  `,
  'mouth_think.png': () => `
    <g>
      <path d="M 393 417 C 396 414, 400 419, 404 416 C 406 415, 408 415, 410 417" fill="none" stroke="${LINE}" stroke-width="2.4" stroke-linecap="round"/>
    </g>
  `,
  'mouth_shock.png': () => `
    <g>
      <ellipse cx="400" cy="419" rx="8.2" ry="10.2" fill="#D55E78" stroke="${LINE}" stroke-width="2.5"/>
      <ellipse cx="400" cy="423" rx="5.2" ry="4.2" fill="#FF9EB3"/>
      <ellipse cx="400" cy="411.5" rx="4.2" ry="1.5" fill="#FFFFFF" opacity="0.9"/>
    </g>
  `,

  // 22-25. Random Event Head FX Layers
  'fx_love_hearts.png': () => `
    <g>
      ${vectorHeart(268, 224, 1.35, '#FF76A0', LINE, -16)}
      ${vectorHeart(534, 212, 1.45, '#FF6B98', LINE, 15)}
      ${vectorHeart(318, 170, 1.05, '#FFA8C4', LINE, -8)}
      ${vectorHeart(490, 162, 0.95, '#FF8CB2', LINE, 10)}
      ${vectorHeart(402, 146, 0.85, '#FFC2D6', LINE, 0)}
      ${vectorSparkle(242, 190, 8, '#FFE382')}
      ${vectorSparkle(560, 178, 8, '#FFE382')}
      ${vectorSparkle(360, 142, 6, '#FFD2E4')}
    </g>
  `,
  'fx_doze_zzz.png': () => `
    <g>
      <circle cx="428" cy="419" r="13.5" fill="#E8F6FF" fill-opacity="0.72" stroke="#89B2D9" stroke-width="2"/>
      <ellipse cx="423" cy="413" rx="4.2" ry="2.1" transform="rotate(-30 423 413)" fill="#FFFFFF" opacity="0.9"/>

      <!-- Vector Z z z floating comfortably inside window frame -->
      <path d="M 484 252 L 502 252 L 485 270 L 503 270" fill="none" stroke="#8568B2" stroke-width="4.2" stroke-linecap="round" stroke-linejoin="round"/>
      <path d="M 510 212 L 533 212 L 511 235 L 534 235" fill="none" stroke="#9B7FD0" stroke-width="5" stroke-linecap="round" stroke-linejoin="round"/>
      <path d="M 538 166 L 567 166 L 539 195 L 568 195" fill="none" stroke="#B79CE8" stroke-width="5.6" stroke-linecap="round" stroke-linejoin="round"/>
      ${vectorSparkle(496, 195, 6, '#D8C8FF')}
    </g>
  `,
  'fx_think_bubble.png': () => `
    <g>
      <circle cx="496" cy="256" r="6" fill="#FFFFFF" stroke="${LINE}" stroke-width="2.2"/>
      <circle cx="514" cy="236" r="9.5" fill="#FFFFFF" stroke="${LINE}" stroke-width="2.4"/>
      <path d="M 538 214
        C 522 214, 514 198, 524 186
        C 520 170, 536 156, 552 162
        C 562 146, 588 148, 594 164
        C 612 162, 622 180, 612 194
        C 618 208, 604 220, 588 216
        C 576 226, 550 224, 538 214 Z"
        fill="#FFFDF9" stroke="${LINE}" stroke-width="2.8" stroke-linejoin="round"/>
      <path d="M 558 179 C 558 170, 576 170, 576 180 C 576 186, 567 188, 567 195" fill="none" stroke="#EB6A94" stroke-width="4.2" stroke-linecap="round"/>
      <circle cx="567" cy="204" r="2.6" fill="#EB6A94"/>
      ${vectorSparkle(614, 152, 7, '#FFE27D')}
    </g>
  `,
  'fx_shock_marks.png': () => `
    <g>
      <g transform="translate(262 214) rotate(-18)">
        <path d="M -5 -26 L 5 -26 L 3 2 L -3 2 Z" fill="#FFE066" stroke="${LINE}" stroke-width="2.4" stroke-linejoin="round"/>
        <circle cx="0" cy="12" r="4.5" fill="#FFE066" stroke="${LINE}" stroke-width="2.4"/>
      </g>
      <g transform="translate(538 214) rotate(18)">
        <path d="M -5 -26 L 5 -26 L 3 2 L -3 2 Z" fill="#FF86AC" stroke="${LINE}" stroke-width="2.4" stroke-linejoin="round"/>
        <circle cx="0" cy="12" r="4.5" fill="#FF86AC" stroke="${LINE}" stroke-width="2.4"/>
      </g>
      <g stroke="#FF759F" stroke-width="3.2" stroke-linecap="round">
        <line x1="226" y1="222" x2="242" y2="232"/>
        <line x1="218" y1="248" x2="236" y2="249"/>
        <line x1="574" y1="222" x2="558" y2="232"/>
        <line x1="582" y1="248" x2="564" y2="249"/>
      </g>
      ${vectorSparkle(286, 178, 8, '#FFE37F')}
      ${vectorSparkle(514, 178, 8, '#FFE37F')}
    </g>
  `,

  // 26. desk_surface.png
  'desk_surface.png': () => `
    <g>
      <path d="M 48 666 L 752 666 L 744 714 C 742 724, 734 730, 722 730 L 78 730 C 66 730, 58 724, 56 714 Z" fill="url(#deskFrontGrad)" stroke="${LINE}" stroke-width="3.2" stroke-linejoin="round"/>
      <rect x="66" y="684" width="668" height="10" rx="5" fill="#C99282" opacity="0.45"/>
      <path d="M 78 556 L 722 556 C 736 556, 748 566, 752 580 L 760 650 C 762 662, 752 670, 738 670 L 62 670 C 48 670, 38 662, 40 650 L 48 580 C 52 566, 64 556, 78 556 Z" fill="url(#deskTopGrad)" stroke="${LINE}" stroke-width="3.4" stroke-linejoin="round"/>
      <path d="M 60 662 L 740 662" fill="none" stroke="#FFFFFF" stroke-width="3" stroke-linecap="round" opacity="0.65"/>
      <g stroke="#E9BEA8" stroke-width="1.6" stroke-linecap="round" opacity="0.5">
        <path d="M 90 595 Q 160 592 225 596"/>
        <path d="M 565 636 Q 640 634 710 638"/>
        <path d="M 110 644 Q 180 642 250 646"/>
      </g>
      <rect x="276" y="572" width="248" height="80" rx="14" fill="url(#deskMatGrad)" stroke="#E3ACC2" stroke-width="2.2"/>
      <rect x="283" y="578" width="234" height="68" rx="10" fill="none" stroke="#F6C8D9" stroke-width="1.4" stroke-dasharray="5 4"/>
    </g>
  `,

  // 27. prop_computer_off.png: Crisp sleek 3/4 closed standby laptop (base + upper lid bevel + sticker + standby LED)
  'prop_computer_off.png': () => `
    <g>
      <ellipse cx="177" cy="624" rx="88" ry="12" fill="#D5A69D" opacity="0.4"/>
      <!-- Lower chassis edge -->
      <path d="M 90 612 L 104 624 L 260 618 L 268 606 L 260 598 L 96 604 Z" fill="#B9A9D9" stroke="${LINE}" stroke-width="2.6" stroke-linejoin="round"/>
      <!-- Closed lid top surface -->
      <path d="M 106 566 L 248 562 C 254 562, 259 565, 261 571 L 268 604 C 269 610, 265 615, 258 615 L 106 620 C 98 620, 92 616, 91 610 L 97 574 C 98 569, 101 566, 106 566 Z" fill="url(#laptopBodyGrad)" stroke="${LINE}" stroke-width="2.8" stroke-linejoin="round"/>
      <!-- Lid highlight rim -->
      <path d="M 104 571 L 246 567" fill="none" stroke="#FFFFFF" stroke-width="2.5" stroke-linecap="round" opacity="0.72"/>
      <!-- Cute crescent / sakura laptop emblem -->
      <circle cx="179" cy="591" r="11" fill="#FFF7FB" stroke="#C5B0E0" stroke-width="1.8"/>
      <circle cx="179" cy="591" r="4.5" fill="#FF9BB8"/>
      <!-- Subtle dim standby dot on front lip -->
      <circle cx="242" cy="619" r="2.5" fill="#9D8EC4"/>
    </g>
  `,

  // 28. prop_computer_on.png
  'prop_computer_on.png': () => `
    <g>
      <ellipse cx="178" cy="586" rx="112" ry="62" fill="url(#screenAura)"/>
      <path d="M 96 566 L 250 566 L 268 614 C 270 621, 265 626, 256 626 L 94 626 C 85 626, 80 621, 82 614 Z" fill="url(#laptopBodyGrad)" stroke="${LINE}" stroke-width="2.8" stroke-linejoin="round"/>
      <path d="M 106 575 L 240 575 L 249 601 L 99 601 Z" fill="#CEC2E8" stroke="#A494C7" stroke-width="1.4"/>
      <path d="M 110 581 H 236 M 107 588 H 240 M 104 595 H 243" stroke="#F8F4FF" stroke-width="3.2" stroke-dasharray="8 3" stroke-linecap="round"/>
      <rect x="152" y="606" width="44" height="14" rx="4" fill="#DDD2F3" stroke="#B2A2D4" stroke-width="1.3"/>

      <path d="M 88 456 C 88 449, 94 445, 102 446 L 244 456 C 251 457, 256 462, 256 469 L 250 567 L 96 567 Z" fill="#F2EAFF" stroke="${LINE}" stroke-width="3" stroke-linejoin="round"/>
      <path d="M 96 462 L 246 471 L 242 558 L 103 558 Z" fill="#7C8BFF" opacity="0.5" filter="url(#tinyGlow)"/>
      <path d="M 97 460 L 246 469 L 242 558 L 103 558 Z" fill="url(#laptopScreenGlow)" stroke="#5D599A" stroke-width="2" stroke-linejoin="round"/>
      <rect x="112" y="476" width="62" height="44" rx="5" fill="#FFFFFF" opacity="0.34"/>
      <rect x="181" y="480" width="50" height="22" rx="4" fill="#FFFFFF" opacity="0.28"/>
      <rect x="181" y="507" width="50" height="15" rx="4" fill="#FFFFFF" opacity="0.24"/>
      <line x1="119" y1="488" x2="165" y2="488" stroke="#FFFFFF" stroke-width="3" stroke-linecap="round" opacity="0.85"/>
      <line x1="119" y1="498" x2="154" y2="498" stroke="#E8ECFF" stroke-width="2.5" stroke-linecap="round" opacity="0.75"/>
      <line x1="119" y1="508" x2="160" y2="508" stroke="#E8ECFF" stroke-width="2.5" stroke-linecap="round" opacity="0.75"/>
      <rect x="112" y="528" width="118" height="18" rx="5" fill="#FFFFFF" opacity="0.28"/>
      <path d="M 124 537 L 128 532 L 132 541 L 136 533 L 140 540 L 144 535 L 148 538" fill="none" stroke="#FFFFFF" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/>
      <circle cx="248" cy="619" r="2.8" fill="#7BF2D0"/>
    </g>
  `,

  // 29. prop_books.png
  'prop_books.png': () => `
    <g>
      <ellipse cx="646" cy="618" rx="74" ry="12" fill="#D5A69D" opacity="0.38"/>
      <g>
        <rect x="662" y="458" width="9" height="48" rx="3" transform="rotate(-14 666 482)" fill="#FFE07D" stroke="${LINE}" stroke-width="2.1"/>
        <rect x="676" y="450" width="8" height="54" rx="3" transform="rotate(-3 680 477)" fill="#FF92B3" stroke="${LINE}" stroke-width="2.1"/>
        <rect x="691" y="454" width="10" height="52" rx="2" transform="rotate(10 696 480)" fill="#9BE2D6" stroke="${LINE}" stroke-width="2.1"/>
        <rect x="703" y="464" width="7" height="44" rx="2" transform="rotate(20 706 486)" fill="#C8A8FF" stroke="${LINE}" stroke-width="2.1"/>
        <path d="M 656 496 L 714 496 L 708 584 C 707 590, 702 594, 694 594 L 676 594 C 668 594, 663 590, 662 584 Z" fill="#EDE2FF" stroke="${LINE}" stroke-width="2.8" stroke-linejoin="round"/>
        <ellipse cx="685" cy="496" rx="29" ry="7" fill="#D7C4F8" stroke="${LINE}" stroke-width="2.5"/>
        ${vectorStar(685, 540, 9, 4, '#FFE285', LINE, 0)}
      </g>

      <g>
        <path d="M 578 590 L 678 584 L 686 610 L 584 618 Z" fill="#FFFBF2" stroke="${LINE}" stroke-width="2.4" stroke-linejoin="round"/>
        <path d="M 574 587 L 678 580 L 682 590 L 578 597 Z" fill="#9ED6FF" stroke="${LINE}" stroke-width="2.4" stroke-linejoin="round"/>
        <path d="M 578 597 L 584 619 L 686 612" fill="none" stroke="#75B5EB" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"/>

        <path d="M 584 564 L 676 558 L 683 583 L 588 590 Z" fill="#FFFDF6" stroke="${LINE}" stroke-width="2.4" stroke-linejoin="round"/>
        <path d="M 580 560 L 676 554 L 680 564 L 584 570 Z" fill="#A6E6CB" stroke="${LINE}" stroke-width="2.4" stroke-linejoin="round"/>
        <path d="M 584 570 L 588 590 L 683 583" fill="none" stroke="#73C5A3" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"/>

        <path d="M 588 538 L 674 532 L 680 556 L 592 563 Z" fill="#FFFDF8" stroke="${LINE}" stroke-width="2.4" stroke-linejoin="round"/>
        <path d="M 584 534 L 674 528 L 678 538 L 588 544 Z" fill="#FFB2CA" stroke="${LINE}" stroke-width="2.4" stroke-linejoin="round"/>
        <path d="M 588 544 L 592 563 L 680 556" fill="none" stroke="#EB83A5" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"/>
        <path d="M 622 558 L 620 578 L 626 573 L 631 579 L 632 557 Z" fill="#FF6E99" stroke="${LINE}" stroke-width="1.8" stroke-linejoin="round"/>
      </g>
    </g>
  `,

  // 30. prop_timer_clock.png
  'prop_timer_clock.png': () => `
    <g>
      <circle cx="540" cy="582" r="44" fill="#FFD3A8" opacity="0.28" filter="url(#tinyGlow)"/>
      <ellipse cx="540" cy="618" rx="34" ry="8" fill="#D5A69D" opacity="0.4"/>
      <path d="M 512 552 C 506 544, 516 536, 524 544 Z" fill="#FFE082" stroke="${LINE}" stroke-width="2.4" stroke-linejoin="round"/>
      <path d="M 568 552 C 574 544, 564 536, 556 544 Z" fill="#FFE082" stroke="${LINE}" stroke-width="2.4" stroke-linejoin="round"/>
      <rect x="536" y="537" width="8" height="8" rx="3" fill="#FF92B3" stroke="${LINE}" stroke-width="2.2"/>
      <line x1="520" y1="607" x2="515" y2="616" stroke="${LINE}" stroke-width="4" stroke-linecap="round"/>
      <line x1="560" y1="607" x2="565" y2="616" stroke="${LINE}" stroke-width="4" stroke-linecap="round"/>

      <circle cx="540" cy="579" r="33" fill="#FF9DB9" stroke="${LINE}" stroke-width="2.8"/>
      <circle cx="540" cy="579" r="25.5" fill="#FFFDF8" stroke="#E589A6" stroke-width="1.8"/>
      <path d="M 540 579 L 540 556 A 23 23 0 0 1 560 590 Z" fill="#FFD1DF" opacity="0.82"/>
      <g stroke="#B8839B" stroke-width="1.8" stroke-linecap="round">
        <line x1="540" y1="556" x2="540" y2="560"/>
        <line x1="540" y1="598" x2="540" y2="602"/>
        <line x1="517" y1="579" x2="521" y2="579"/>
        <line x1="559" y1="579" x2="563" y2="579"/>
      </g>
      <line x1="540" y1="579" x2="540" y2="562" stroke="${LINE}" stroke-width="2.6" stroke-linecap="round"/>
      <line x1="540" y1="579" x2="554" y2="587" stroke="#EB5E85" stroke-width="2.4" stroke-linecap="round"/>
      <circle cx="540" cy="579" r="3.2" fill="#FFE07D" stroke="${LINE}" stroke-width="1.6"/>
      <path d="M 522 566 C 527 560, 534 557, 542 557" fill="none" stroke="#FFFFFF" stroke-width="2.5" stroke-linecap="round" opacity="0.85"/>
    </g>
  `,

  // 31. arms_chill.png: Cozy knit cuff bands + relaxed hands on desk mat
  'arms_chill.png': () => `
    <g>
      <ellipse cx="400" cy="616" rx="95" ry="15" fill="#DFABB8" opacity="0.34"/>
      <!-- Left Sleeve -->
      <path d="M 304 516 C 294 548, 294 584, 312 606 C 325 620, 355 624, 378 618 L 386 594 C 362 590, 344 582, 336 558 C 330 540, 326 524, 322 514" fill="url(#sleeveGrad)" stroke="${LINE}" stroke-width="3.2" stroke-linejoin="round"/>
      <!-- Ribbed knit cuff -->
      <path d="M 366 591 L 378 594 C 383 602, 381 615, 374 620 L 362 618 C 369 613, 371 600, 366 591 Z" fill="#FFF0F5" stroke="${LINE}" stroke-width="2.1" stroke-linejoin="round"/>
      <!-- Left hand -->
      <path d="M 378 596 C 392 594, 405 600, 406 610 C 406 618, 392 622, 376 618 Z" fill="url(#skinGrad)" stroke="${LINE}" stroke-width="2.6" stroke-linejoin="round"/>
      <path d="M 390 604 Q 398 605 402 608" fill="none" stroke="#E0A59D" stroke-width="1.4" stroke-linecap="round"/>

      <!-- Right Sleeve -->
      <path d="M 496 516 C 506 548, 506 584, 488 606 C 475 620, 445 624, 422 618 L 414 594 C 438 590, 456 582, 464 558 C 470 540, 474 524, 478 514" fill="url(#sleeveGrad)" stroke="${LINE}" stroke-width="3.2" stroke-linejoin="round"/>
      <path d="M 434 591 L 422 594 C 417 602, 419 615, 426 620 L 438 618 C 431 613, 429 600, 434 591 Z" fill="#FFF0F5" stroke="${LINE}" stroke-width="2.1" stroke-linejoin="round"/>
      <!-- Right hand -->
      <path d="M 422 597 C 408 596, 396 604, 397 613 C 398 621, 412 623, 425 618 Z" fill="url(#skinGrad)" stroke="${LINE}" stroke-width="2.6" stroke-linejoin="round"/>
      <path d="M 412 606 Q 405 607 401 610" fill="none" stroke="#E0A59D" stroke-width="1.4" stroke-linecap="round"/>
    </g>
  `,

  // 32. arms_study.png: Study notebook + ribbed knit cuffs + left hand steadying page + right hand holding sakura pen
  'arms_study.png': () => `
    <g>
      <g>
        <ellipse cx="400" cy="642" rx="70" ry="9" fill="#DFABB8" opacity="0.34"/>
        <path d="M 332 584 L 468 584 L 474 644 L 326 644 Z" fill="#FFB2C9" stroke="${LINE}" stroke-width="2.6" stroke-linejoin="round"/>
        <path d="M 337 587 L 400 589 L 400 639 L 332 639 Z" fill="#FFFDF9" stroke="${LINE_SOFT}" stroke-width="1.8" stroke-linejoin="round"/>
        <path d="M 463 587 L 400 589 L 400 639 L 468 639 Z" fill="#FFFDF9" stroke="${LINE_SOFT}" stroke-width="1.8" stroke-linejoin="round"/>
        <g stroke="#C8B6CE" stroke-width="1.5" stroke-linecap="round">
          <line x1="346" y1="598" x2="390" y2="598"/>
          <line x1="345" y1="607" x2="388" y2="607"/>
          <line x1="344" y1="616" x2="384" y2="616"/>
          <line x1="343" y1="625" x2="376" y2="625"/>
          <line x1="410" y1="598" x2="454" y2="598"/>
          <line x1="410" y1="607" x2="452" y2="607"/>
          <line x1="410" y1="616" x2="444" y2="616"/>
        </g>
        <rect x="454" y="592" width="12" height="7" rx="1.5" fill="#FFE282" stroke="${LINE_SOFT}" stroke-width="1"/>
        <rect x="455" y="603" width="12" height="7" rx="1.5" fill="#FF92B3" stroke="${LINE_SOFT}" stroke-width="1"/>
      </g>

      <!-- Left Sleeve & Cuff -->
      <path d="M 304 516 C 294 548, 294 584, 310 604 C 322 618, 344 619, 360 614 L 366 592 C 350 588, 340 578, 334 556 C 329 539, 325 524, 322 514" fill="url(#sleeveGrad)" stroke="${LINE}" stroke-width="3.2" stroke-linejoin="round"/>
      <path d="M 353 589 L 364 592 C 367 599, 366 610, 360 616 L 349 614 C 355 609, 356 598, 353 589 Z" fill="#FFF0F5" stroke="${LINE}" stroke-width="2" stroke-linejoin="round"/>
      <path d="M 362 594 C 374 593, 384 600, 383 608 C 382 616, 369 618, 359 614 Z" fill="url(#skinGrad)" stroke="${LINE}" stroke-width="2.5" stroke-linejoin="round"/>

      <!-- Right Sleeve & Cuff -->
      <path d="M 496 516 C 506 548, 506 584, 490 604 C 478 618, 456 620, 440 614 L 434 592 C 450 588, 460 578, 466 556 C 471 539, 475 524, 478 514" fill="url(#sleeveGrad)" stroke="${LINE}" stroke-width="3.2" stroke-linejoin="round"/>
      <path d="M 447 589 L 436 592 C 433 599, 434 610, 440 616 L 451 614 C 445 609, 444 598, 447 589 Z" fill="#FFF0F5" stroke="${LINE}" stroke-width="2" stroke-linejoin="round"/>
      <g>
        <path d="M 416 622 L 456 578 L 463 584 L 422 627 L 413 628 Z" fill="#FF89AC" stroke="${LINE}" stroke-width="2.1" stroke-linejoin="round"/>
        <circle cx="461" cy="579" r="4.2" fill="#FFE07D" stroke="${LINE}" stroke-width="1.7"/>
      </g>
      <path d="M 440 594 C 427 593, 417 602, 420 611 C 423 619, 435 619, 443 614 Z" fill="url(#skinGrad)" stroke="${LINE}" stroke-width="2.5" stroke-linejoin="round"/>
      <path d="M 426 604 Q 431 607 434 611" fill="none" stroke="#E0A59D" stroke-width="1.4" stroke-linecap="round"/>
    </g>
  `,
};

async function main() {
  await fs.mkdir(OUT_DIR, { recursive: true });

  const entries = Object.entries(LAYER_BUILDERS);
  console.log(`Generating ${entries.length} anime character & scene PNG assets (800x800 RGBA)...`);

  for (const [filename, buildBody] of entries) {
    const svg = wrapSvg(buildBody());
    const resvg = new Resvg(svg, {
      fitTo: { mode: 'width', value: 800 },
      background: 'rgba(0, 0, 0, 0)',
    });
    const rendered = resvg.render();
    if (rendered.width !== 800 || rendered.height !== 800) {
      throw new Error(`Expected 800x800 for ${filename}, got ${rendered.width}x${rendered.height}`);
    }
    const pngBuffer = rendered.asPng();
    await fs.writeFile(path.join(OUT_DIR, filename), pngBuffer);
  }

  console.log(`Successfully generated all ${entries.length} PNGs into ${OUT_DIR}`);
}

main().catch((err) => {
  console.error('Failed to generate anime assets:', err);
  process.exit(1);
});

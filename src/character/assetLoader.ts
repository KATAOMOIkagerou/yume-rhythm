import { useEffect, useRef, useState } from 'react';
import type { CharacterManifest, DeskCharacterSceneProps, ExpressionDefinition } from '../types/contracts';
import { legacyExpressions, parseExpressions } from './expressions';

export interface AssetBaseInfo {
  baseUrl: string;
  assetDirHint: string;
}

export interface LoadedCharacterAssets {
  expressions: ExpressionDefinition[];
  manifest: CharacterManifest;
  urls: Record<string, string>;
  assetDirHint: string;
  baseUrl: string;
}

export interface UseCharacterAssetsResult {
  assets: LoadedCharacterAssets | null;
  loading: boolean;
  error: string | null;
}

export function resolveAssetBase(): AssetBaseInfo {
  const bridge = typeof window !== 'undefined' ? window.desktopPetBridge : undefined;
  if (bridge?.assetProtocolBase) {
    return {
      baseUrl: bridge.assetProtocolBase.replace(/\/+$/, ''),
      assetDirHint: bridge.externalAssetDir || 'character_assets',
    };
  }
  return {
    baseUrl: './character_assets',
    assetDirHint: 'public/character_assets',
  };
}

function buildVersionedUrl(baseUrl: string, relativePath: string, reloadToken: number): string {
  const cleanSegments = relativePath
    .replace(/^\/+/, '')
    .split('/')
    .filter(Boolean)
    .map((segment) => encodeURIComponent(segment));
  return `${baseUrl}/${cleanSegments.join('/')}?v=${encodeURIComponent(String(reloadToken))}`;
}

export function collectManifestFiles(manifest: CharacterManifest): string[] {
  const files = new Set<string>();
  const add = (value?: string | null) => {
    if (typeof value === 'string' && value.trim().length > 0) {
      files.add(value.trim());
    }
  };

  Object.entries(manifest.layers).forEach(([key, file]) => { if (key !== 'sceneBg') add(file); });
  Object.values(manifest.expressions).forEach((expr) => {
    add(expr.eyes);
    add(expr.mouth);
  });
  add(manifest.blinkEyes);
  Object.values(manifest.randomEvents).forEach((ev) => {
    add(ev.eyes);
    add(ev.mouth);
    add(ev.fx);
  });

  return Array.from(files);
}

function validateManifest(raw: unknown): CharacterManifest {
  if (!raw || typeof raw !== 'object') {
    throw new Error('角色清单格式无效：根节点不是对象');
  }
  const m = raw as Partial<CharacterManifest>;
  if (
    !m.layers ||
    !m.expressions ||
    !m.blinkEyes ||
    !m.randomEvents ||
    !m.headPivot ||
    typeof m.canvasWidth !== 'number' ||
    typeof m.canvasHeight !== 'number'
  ) {
    throw new Error('角色清单缺少必需的图层、表情、眨眼、随机事件或支点配置');
  }
  return m as CharacterManifest;
}

function preloadImage(url: string, filename: string, signal?: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    if (signal?.aborted) {
      reject(new DOMException('Aborted', 'AbortError'));
      return;
    }

    const img = new Image();
    let settled = false;

    const cleanup = () => {
      img.onload = null;
      img.onerror = null;
      signal?.removeEventListener('abort', onAbort);
    };

    const onAbort = () => {
      if (settled) return;
      settled = true;
      cleanup();
      reject(new DOMException('Aborted', 'AbortError'));
    };

    signal?.addEventListener('abort', onAbort, { once: true });

    img.onload = () => {
      if (settled) return;
      settled = true;
      cleanup();
      if (typeof img.decode === 'function') {
        img.decode().then(
          () => resolve(),
          () => resolve()
        );
      } else {
        resolve();
      }
    };

    img.onerror = () => {
      if (settled) return;
      settled = true;
      cleanup();
      reject(new Error(`加载人物图层失败: ${filename}`));
    };

    img.src = url;
  });
}

export async function loadCharacterAssets(
  reloadToken: number,
  signal?: AbortSignal
): Promise<LoadedCharacterAssets> {
  const { baseUrl, assetDirHint } = resolveAssetBase();
  const manifestUrl = buildVersionedUrl(baseUrl, 'manifest.json', reloadToken);

  const response = await fetch(manifestUrl, {
    cache: 'no-store',
    signal,
  });
  if (!response.ok) {
    throw new Error(`无法读取角色清单，响应状态码：${response.status}`);
  }

  const rawManifest: unknown = await response.json();
  const manifest = validateManifest(rawManifest);
  const expressionResponse = await fetch(buildVersionedUrl(baseUrl, 'expressions.json', reloadToken), { cache: 'no-store', signal });
  const expressions = expressionResponse.status === 404 ? legacyExpressions(manifest)
    : expressionResponse.ok ? parseExpressions(await expressionResponse.json())
    : (() => { throw new Error('无法读取表情配置'); })();
  const files = [...new Set([...collectManifestFiles(manifest),
    ...expressions.flatMap((entry) => Object.values(entry.resources))])];

  const urls: Record<string, string> = {};
  for (const file of files) {
    urls[file] = buildVersionedUrl(baseUrl, file, reloadToken);
  }

  await Promise.all(files.map((file) => preloadImage(urls[file], file, signal)));

  return {
    expressions,
    manifest,
    urls,
    assetDirHint,
    baseUrl,
  };
}

export function useCharacterAssets(
  reloadToken: number,
  onAssetStatus?: DeskCharacterSceneProps['onAssetStatus']
): UseCharacterAssetsResult {
  const [state, setState] = useState<UseCharacterAssetsResult>({
    assets: null,
    loading: true,
    error: null,
  });

  const onAssetStatusRef = useRef(onAssetStatus);
  onAssetStatusRef.current = onAssetStatus;

  useEffect(() => {
    let cancelled = false;
    const controller = new AbortController();
    const { assetDirHint } = resolveAssetBase();

    setState((prev) => ({
      assets: prev.assets,
      loading: true,
      error: null,
    }));

    loadCharacterAssets(reloadToken, controller.signal)
      .then((loaded) => {
        if (cancelled) return;
        setState({
          assets: loaded,
          loading: false,
          error: null,
        });
        onAssetStatusRef.current?.({
          loaded: true,
          packName: loaded.manifest.packName,
          assetDirHint: loaded.assetDirHint,
        });
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        if (err instanceof DOMException && err.name === 'AbortError') return;
        const rawMessage = err instanceof Error ? err.message : '';
        const message = /[\u4e00-\u9fff]/.test(rawMessage) ? rawMessage : '素材加载失败，请确认素材文件完整后重新加载。';
        setState((prev) => ({
          assets: prev.assets,
          loading: false,
          error: message,
        }));
        onAssetStatusRef.current?.({
          loaded: false,
          assetDirHint,
          error: message,
        });
      });

    return () => {
      cancelled = true;
      controller.abort();
    };
  }, [reloadToken]);

  return state;
}

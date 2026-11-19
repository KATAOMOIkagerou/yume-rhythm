import type { CharacterManifest, ExpressionDefinition, RandomEventType } from '../types/contracts';

export function legacyExpressions(manifest: CharacterManifest): ExpressionDefinition[] {
  return [
    ...(['neutral', 'happy', 'sad'] as const).map((id) => ({
      id, name: { neutral: '自然', happy: '开心', sad: '难过' }[id], type: 'base' as const,
      resources: manifest.expressions[id], durationMs: 3200, random: id !== 'neutral',
    })),
    ...(['doze', 'think', 'shock', 'love'] as const).map((id) => ({
      id, name: { doze: '打瞌睡', think: '思考', shock: '震惊', love: '喜欢' }[id],
      type: 'accessory' as const, resources: {
        eyes: manifest.randomEvents[id].eyes, mouth: manifest.randomEvents[id].mouth,
        overlay: manifest.randomEvents[id].fx,
      }, durationMs: 3200, random: true, event: id,
    })),
  ];
}

export function parseExpressions(raw: unknown): ExpressionDefinition[] {
  if (!raw || typeof raw !== 'object' || !('expressions' in raw) || !Array.isArray(raw.expressions)) {
    throw new Error('表情配置必须包含表情列表');
  }
  const ids = new Set<string>();
  const events: RandomEventType[] = ['doze', 'think', 'shock', 'love'];
  const list = raw.expressions.map((item: unknown): ExpressionDefinition => {
    if (!item || typeof item !== 'object') throw new Error('表情配置项无效');
    const entry = item as ExpressionDefinition;
    if (typeof entry.id !== 'string' || !/^[a-z][a-z0-9_-]*$/.test(entry.id) || ids.has(entry.id)
      || typeof entry.name !== 'string' || !entry.name.trim()
      || !['base', 'accessory'].includes(entry.type) || !entry.resources || typeof entry.resources !== 'object'
      || !Number.isFinite(entry.durationMs) || entry.durationMs < 100 || entry.durationMs > 600000
      || typeof entry.random !== 'boolean' || (entry.event !== undefined && !events.includes(entry.event))) {
      throw new Error('表情配置的标识、名称、类型、时长或随机开关无效');
    }
    const resources: ExpressionDefinition['resources'] = {};
    for (const key of ['animation', 'eyes', 'mouth', 'overlay'] as const) {
      const value = entry.resources[key];
      if (value === undefined) continue;
      if (typeof value !== 'string' || !value.trim() || /(^[\\/]|[:?#]|(^|[\\/])\.\.([\\/]|$))/.test(value)) {
        throw new Error(`表情「${entry.name}」的资源必须使用素材目录内的相对路径`);
      }
      resources[key] = value;
    }
    if (entry.type === 'base' ? !(resources.animation || (resources.eyes && resources.mouth)) : !Object.keys(resources).length) {
      throw new Error(`表情「${entry.name}」缺少可播放资源`);
    }
    ids.add(entry.id);
    return { ...entry, resources };
  });
  if (!list.some((item) => item.id === 'neutral' && item.type === 'base')) {
    throw new Error('表情列表必须包含自然表情');
  }
  return list;
}

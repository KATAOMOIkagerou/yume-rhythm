import React from 'react';
import type { EmotionType } from '../types/contracts';
import { CheckIcon, HeartIcon } from './Icons';

export interface EmotionTabProps {
  emotion: EmotionType;
  onSelectEmotion: (emotion: EmotionType) => void;
}

const EMOTION_LABEL: Record<EmotionType, string> = {
  neutral: '自然 (Neutral)',
  happy: '开心 (Happy)',
  sad: '难过 (Sad)',
};

export function EmotionTab({ emotion, onSelectEmotion }: EmotionTabProps) {
  const toggleEmotion = (target: 'happy' | 'sad') => {
    onSelectEmotion(emotion === target ? 'neutral' : target);
  };

  return (
    <div className="space-y-3">
      <div className="anime-card-sakura rounded-2xl p-3.5">
        <div className="mb-3 flex items-center justify-between">
          <div className="flex items-center gap-1.5 text-[13px] font-bold text-[#4A3246]">
            <HeartIcon size={16} className="text-[#EC4899]" />
            <span>情绪表情切换</span>
          </div>
          <span
            data-testid="emotion-current"
            className="rounded-full border border-[#E5B8D0] bg-white px-2.5 py-0.5 text-[11px] font-bold text-[#7C3AED]"
          >
            当前：{EMOTION_LABEL[emotion]}
          </span>
        </div>

        <div className="space-y-2.5">
          <button
            type="button"
            data-testid="emotion-happy"
            onClick={() => toggleEmotion('happy')}
            className={`flex w-full cursor-pointer items-center gap-3 rounded-2xl border-2 px-3.5 py-2.5 text-left transition ${
              emotion === 'happy'
                ? 'border-[#6E4F75] bg-[#FFF5D6] shadow-[0_2px_0_#E6CE80]'
                : 'border-[#E6C7DA] bg-white/90 hover:border-[#6E4F75] hover:bg-[#FFF9FC]'
            }`}
          >
            <span
              className={`flex h-5 w-5 shrink-0 items-center justify-center rounded-md border-2 transition ${
                emotion === 'happy'
                  ? 'border-[#6E4F75] bg-[#F472B6] text-white'
                  : 'border-[#8D6E92] bg-white text-transparent'
              }`}
            >
              <CheckIcon size={13} />
            </span>
            <div className="flex-1">
              <div className="text-[13.5px] font-bold text-[#4A3246]">Happy (开心)</div>
              <div className="text-[11px] text-[#755873]">弯弯笑眼与甜美微笑表情</div>
            </div>
            <span className="text-lg" aria-hidden="true">
              ✨
            </span>
          </button>

          <button
            type="button"
            data-testid="emotion-sad"
            onClick={() => toggleEmotion('sad')}
            className={`flex w-full cursor-pointer items-center gap-3 rounded-2xl border-2 px-3.5 py-2.5 text-left transition ${
              emotion === 'sad'
                ? 'border-[#6E4F75] bg-[#EDE9FE] shadow-[0_2px_0_#C7B8F5]'
                : 'border-[#E6C7DA] bg-white/90 hover:border-[#6E4F75] hover:bg-[#F9F6FF]'
            }`}
          >
            <span
              className={`flex h-5 w-5 shrink-0 items-center justify-center rounded-md border-2 transition ${
                emotion === 'sad'
                  ? 'border-[#6E4F75] bg-[#8B5CF6] text-white'
                  : 'border-[#8D6E92] bg-white text-transparent'
              }`}
            >
              <CheckIcon size={13} />
            </span>
            <div className="flex-1">
              <div className="text-[13.5px] font-bold text-[#4A3246]">Sad (难过)</div>
              <div className="text-[11px] text-[#755873]">垂眉微润泪眼与小委屈嘴巴</div>
            </div>
            <span className="text-lg" aria-hidden="true">
              💧
            </span>
          </button>
        </div>
      </div>

      <p className="rounded-xl border border-dashed border-[#DFC7DA] bg-white/75 px-3 py-2 text-[11.5px] leading-relaxed text-[#755873]">
        💡 小提示：再次点击当前已选中的情绪选项，即可恢复默认自然表情（Neutral）。
      </p>
    </div>
  );
}

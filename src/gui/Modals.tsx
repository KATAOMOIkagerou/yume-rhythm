import React from 'react';
import { ClockIcon, HeartIcon, CheckIcon } from './Icons';

export type AppModalType =
  | 'none'
  | 'confirm-pause'
  | 'confirm-leave-study'
  | 'go-back-study'
  | 'confirm-save-todo';

export interface ModalsProps {
  modal: AppModalType;
  onConfirmPause: () => void;
  onRejectPause: () => void;
  onDismissGoBack: () => void;
  onConfirmSaveTodo: () => void;
  onCancelSaveTodo: () => void;
}

export function Modals({
  modal,
  onConfirmPause,
  onRejectPause,
  onDismissGoBack,
  onConfirmSaveTodo,
  onCancelSaveTodo,
}: ModalsProps) {
  if (modal === 'none') return null;

  return (
    <div
      data-testid="modal-overlay"
      className="fixed inset-0 z-50 flex items-center justify-center bg-[#4A3246]/25 px-4 backdrop-blur-[3px]"
      role="dialog"
      aria-modal="true"
    >
      <div
        data-testid="modal-card"
        className="modal-pop anime-window w-full max-w-[340px] rounded-[24px] p-5 text-center"
      >
        {(modal === 'confirm-pause' || modal === 'confirm-leave-study') && (
          <>
            <div className="mx-auto mb-2.5 flex h-11 w-11 items-center justify-center rounded-2xl border-2 border-[#6E4F75] bg-[#FFF5D6] text-[#583B5E]">
              <ClockIcon size={22} />
            </div>
            <p className="text-[15px] font-bold leading-snug text-[#4A3246]">
              确定暂停本次自习吗？
            </p>
            <p className="mt-1.5 text-[12.5px] font-medium text-[#755873]">
              {modal === 'confirm-leave-study'
                ? '暂停并离开自习后，计时进度仍会保留。'
                : '暂停后保留当前进度，随时可以继续。'}
            </p>
            <div className="mt-4 flex items-center justify-center gap-3">
              <button
                type="button"
                data-testid="modal-btn-yes"
                onClick={onConfirmPause}
                className="min-w-[96px] cursor-pointer rounded-2xl border-2 border-[#6E4F75] bg-[#FFE4EF] px-4 py-2 text-[13px] font-bold text-[#4A3246] shadow-[0_2px_0_#E6B9CE] transition hover:bg-[#FFD4E6] active:translate-y-0.5"
              >
                确定
              </button>
              <button
                type="button"
                data-testid="modal-btn-no"
                onClick={onRejectPause}
                className="min-w-[96px] cursor-pointer rounded-2xl border-2 border-[#6E4F75] bg-[#EFE8FF] px-4 py-2 text-[13px] font-bold text-[#4A3246] shadow-[0_2px_0_#CBB9F2] transition hover:bg-[#E4D7FF] active:translate-y-0.5"
              >
                取消
              </button>
            </div>
          </>
        )}

        {modal === 'go-back-study' && (
          <>
            <div className="mx-auto mb-2.5 flex h-11 w-11 items-center justify-center rounded-2xl border-2 border-[#6E4F75] bg-[#FFE4EF] text-[#D9468F]">
              <HeartIcon size={22} />
            </div>
            <p className="text-[15px] font-bold leading-snug text-[#4A3246]">
              继续专注，加油！
            </p>
            <p className="mt-1.5 text-[12.5px] font-medium text-[#755873]">
              继续完成本次自习吧！
            </p>
            <div className="mt-4 flex justify-center">
              <button
                type="button"
                data-testid="modal-btn-okay"
                onClick={onDismissGoBack}
                className="min-w-[120px] cursor-pointer rounded-2xl border-2 border-[#6E4F75] bg-[#FFF2B8] px-5 py-2 text-[13px] font-bold text-[#4A3246] shadow-[0_2px_0_#E6CE80] transition hover:bg-[#FFEA96] active:translate-y-0.5"
              >
                继续学习
              </button>
            </div>
          </>
        )}

        {modal === 'confirm-save-todo' && (
          <>
            <div className="mx-auto mb-2.5 flex h-11 w-11 items-center justify-center rounded-2xl border-2 border-[#6E4F75] bg-[#E8FFF2] text-[#15803D]">
              <CheckIcon size={20} />
            </div>
            <p className="text-[15px] font-bold leading-snug text-[#4A3246]">
              保存清单？
            </p>
            <p className="mt-1.5 text-[12.5px] font-medium text-[#755873]">
              保存清单？确认后将保存在本地以便下次继续查看。
            </p>
            <div className="mt-4 flex items-center justify-center gap-3">
              <button
                type="button"
                data-testid="modal-save-yes"
                onClick={onConfirmSaveTodo}
                className="min-w-[96px] cursor-pointer rounded-2xl border-2 border-[#6E4F75] bg-[#DCFCE7] px-4 py-2 text-[13px] font-bold text-[#14532D] shadow-[0_2px_0_#A7DFBA] transition hover:bg-[#C6F6D5] active:translate-y-0.5"
              >
                确定
              </button>
              <button
                type="button"
                data-testid="modal-save-no"
                onClick={onCancelSaveTodo}
                className="min-w-[96px] cursor-pointer rounded-2xl border-2 border-[#6E4F75] bg-[#FFF5FA] px-4 py-2 text-[13px] font-bold text-[#4A3246] shadow-[0_2px_0_#E3C4D7] transition hover:bg-[#FFEBF4] active:translate-y-0.5"
              >
                取消
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

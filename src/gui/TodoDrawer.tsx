import React from 'react';
import { CheckIcon } from './Icons';

export interface TodoItem {
  id: string;
  text: string;
  done: boolean;
}

export interface TodoDrawerProps {
  open: boolean;
  listName: string;
  onListNameChange: (name: string) => void;
  items: TodoItem[];
  onToggleItem: (id: string) => void;
  onUpdateItemText: (id: string, text: string) => void;
  onAddItem: () => void;
  onRemoveItem: (id: string) => void;
  onRequestSave: () => void;
  savedNotice: boolean;
}

export function TodoDrawer({
  open,
  listName,
  onListNameChange,
  items,
  onToggleItem,
  onUpdateItemText,
  onAddItem,
  onRemoveItem,
  onRequestSave,
  savedNotice,
}: TodoDrawerProps) {
  if (!open) return null;

  return (
    <div
      data-testid="todo-drawer"
      className="drawer-drop anime-drawer mx-3 rounded-b-[22px] px-4 pb-3.5 pt-3"
    >
      <div className="mb-2 flex items-center gap-2">
        <span className="shrink-0 text-[12px] font-bold text-[#6E4F75]">清单名称</span>
        <input
          type="text"
          data-testid="todo-list-name"
          value={listName}
          onChange={(e) => onListNameChange(e.target.value)}
          placeholder="今日学习计划..."
          maxLength={40}
          className="min-w-0 flex-1 rounded-xl border-2 border-[#D7C1D9] bg-white px-2.5 py-1 text-[12.5px] font-semibold text-[#4A3246] outline-none transition focus:border-[#6E4F75]"
        />
      </div>

      <div className="anime-scroll max-h-[146px] space-y-1.5 overflow-y-auto pr-0.5">
        {items.map((item, idx) => (
          <div key={item.id} className="flex items-center gap-2">
            <span className="w-4 shrink-0 text-right font-mono text-[12px] font-bold text-[#8A6B89]">
              {idx + 1}.
            </span>
            <button
              type="button"
              data-testid={`todo-check-${idx}`}
              onClick={() => onToggleItem(item.id)}
              aria-label={`完成第 ${idx + 1} 项`}
              className={`flex h-5 w-5 shrink-0 cursor-pointer items-center justify-center rounded-md border-2 transition ${
                item.done
                  ? 'border-[#6E4F75] bg-[#F9A8D4] text-[#4A3246]'
                  : 'border-[#9B7E9E] bg-white text-transparent hover:border-[#6E4F75]'
              }`}
            >
              <CheckIcon size={12} />
            </button>
            <input
              type="text"
              data-testid={`todo-input-${idx}`}
              value={item.text}
              onChange={(e) => onUpdateItemText(item.id, e.target.value)}
              placeholder={`待办事项 ${idx + 1}`}
              maxLength={60}
              className={`min-w-0 flex-1 rounded-lg border border-[#E7D5E4] bg-white/90 px-2 py-1 text-[12px] outline-none transition focus:border-[#6E4F75] ${
                item.done ? 'text-[#9A849A] line-through' : 'text-[#4A3246]'
              }`}
            />
            {items.length > 1 && (
              <button
                type="button"
                onClick={() => onRemoveItem(item.id)}
                title="删除该行"
                className="flex h-5 w-5 shrink-0 cursor-pointer items-center justify-center rounded-md text-[12px] font-bold text-[#A589A5] transition hover:bg-[#FFE4EF] hover:text-[#BE185D]"
              >
                ×
              </button>
            )}
          </div>
        ))}
      </div>

      <div className="mt-2.5 flex items-center justify-between gap-2 border-t border-dashed border-[#DFC9D9] pt-2.5">
        <button
          type="button"
          data-testid="todo-add-row"
          onClick={onAddItem}
          className="cursor-pointer rounded-xl border border-[#CFAFC9] bg-white px-2.5 py-1 text-[11.5px] font-semibold text-[#6E4F75] transition hover:bg-[#FFF0F6]"
        >
          + 添加一行
        </button>

        <div className="flex items-center gap-2">
          {savedNotice && (
            <span
              data-testid="todo-saved-badge"
              className="rounded-full border border-[#86EFAC] bg-[#DCFCE7] px-2 py-0.5 text-[11px] font-bold text-[#166534]"
            >
              ✓ 已保存
            </span>
          )}
          <button
            type="button"
            data-testid="btn-save-list"
            onClick={onRequestSave}
            className="cursor-pointer rounded-xl border-2 border-[#6E4F75] bg-[#FFF2B8] px-3 py-1 text-[12px] font-bold text-[#4A3246] shadow-[0_2px_0_#E6CE80] transition hover:bg-[#FFE991] active:translate-y-0.5"
          >
            保存清单
          </button>
        </div>
      </div>
    </div>
  );
}

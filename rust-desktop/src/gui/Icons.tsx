import React from 'react';

export interface IconProps {
  className?: string;
  size?: number;
}

export function LockIcon({ className = '', size = 16 }: IconProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      className={className}
      aria-hidden="true"
    >
      <path
        d="M7.5 10.5V7.8C7.5 5.2 9.5 3.5 12 3.5C14.5 3.5 16.5 5.2 16.5 7.8V10.5"
        stroke="currentColor"
        strokeWidth="2.1"
        strokeLinecap="round"
      />
      <rect
        x="5"
        y="10.2"
        width="14"
        height="10.3"
        rx="3.4"
        fill="#FDE68A"
        stroke="currentColor"
        strokeWidth="2"
      />
      <circle cx="12" cy="14.6" r="1.4" fill="currentColor" />
      <path d="M12 15.8V17.7" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" />
    </svg>
  );
}

export function MetronomeIcon({ className = '', size = 16 }: IconProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      className={className}
      aria-hidden="true"
    >
      <path
        d="M7.2 20L10.3 4.8C10.6 3.7 11.2 3.3 12 3.3C12.8 3.3 13.4 3.7 13.7 4.8L16.8 20H7.2Z"
        fill="#EDE9FE"
        stroke="currentColor"
        strokeWidth="1.9"
        strokeLinejoin="round"
      />
      <path d="M5.5 20H18.5" stroke="currentColor" strokeWidth="2.1" strokeLinecap="round" />
      <path d="M12 16.8L17.3 7.2" stroke="#EC4899" strokeWidth="2.1" strokeLinecap="round" />
      <circle cx="16" cy="9.6" r="1.7" fill="#F9A8D4" stroke="currentColor" strokeWidth="1.5" />
    </svg>
  );
}

export function ClockIcon({ className = '', size = 16 }: IconProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      className={className}
      aria-hidden="true"
    >
      <circle cx="12" cy="12.5" r="8.3" fill="#FFFDF0" stroke="currentColor" strokeWidth="2" />
      <path d="M12 8V12.6L15.1 14.3" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
      <path d="M8.5 3.3L6.2 5.2M15.5 3.3L17.8 5.2" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" />
    </svg>
  );
}

export function HeartIcon({ className = '', size = 16 }: IconProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      className={className}
      aria-hidden="true"
    >
      <path
        d="M12 20.2C12 20.2 4.3 14.9 4.3 9.2C4.3 6.6 6.3 4.7 8.7 4.7C10.3 4.7 11.4 5.5 12 6.7C12.6 5.5 13.7 4.7 15.3 4.7C17.7 4.7 19.7 6.6 19.7 9.2C19.7 14.9 12 20.2 12 20.2Z"
        fill="#FBCFE8"
        stroke="currentColor"
        strokeWidth="1.9"
        strokeLinejoin="round"
      />
      <path d="M7.6 8.3C7.9 7.5 8.5 7 9.3 6.8" stroke="#FFFFFF" strokeWidth="1.6" strokeLinecap="round" />
    </svg>
  );
}

export function CheckIcon({ className = '', size = 14 }: IconProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 20 20"
      fill="none"
      className={className}
      aria-hidden="true"
    >
      <path
        d="M4.2 10.4L8.1 14.3L15.8 5.7"
        stroke="currentColor"
        strokeWidth="2.7"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function PlayIcon({ className = '', size = 15 }: IconProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 20 20"
      fill="none"
      className={className}
      aria-hidden="true"
    >
      <path
        d="M6 4.4C6 3.7 6.8 3.3 7.4 3.7L15.8 9.3C16.3 9.6 16.3 10.4 15.8 10.7L7.4 16.3C6.8 16.7 6 16.3 6 15.6V4.4Z"
        fill="currentColor"
      />
    </svg>
  );
}

export function PauseIcon({ className = '', size = 15 }: IconProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 20 20"
      fill="none"
      className={className}
      aria-hidden="true"
    >
      <rect x="5.2" y="4" width="3.5" height="12" rx="1.3" fill="currentColor" />
      <rect x="11.3" y="4" width="3.5" height="12" rx="1.3" fill="currentColor" />
    </svg>
  );
}

export interface StripedProgressBarProps {
  progress: number;
  className?: string;
  animated?: boolean;
}

export function StripedProgressBar({
  progress,
  className = '',
  animated = true,
}: StripedProgressBarProps) {
  const pct = Math.max(0, Math.min(100, progress));
  return (
    <div
      className={`relative h-4 w-full overflow-hidden rounded-full border-2 border-[#6E4F75] bg-[#FFF7FB] shadow-inner ${className}`}
      role="progressbar"
      aria-valuenow={Math.round(pct)}
      aria-valuemin={0}
      aria-valuemax={100}
    >
      <div
        className={`striped-progress-fill h-full rounded-full transition-[width] duration-200 ${animated ? 'animated' : ''}`}
        style={{ width: `${pct}%` }}
      />
      <div className="pointer-events-none absolute inset-x-1 top-0.5 h-1 rounded-full bg-white/55" />
    </div>
  );
}

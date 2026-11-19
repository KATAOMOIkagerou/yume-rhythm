import { useEffect, useMemo, useRef, useState } from 'react';
import type {
  ActivityMode,
  EmotionType,
  EventSwitchMatrix,
  RandomEventType,
} from '../types/contracts';

export const RANDOM_EVENT_ORDER: readonly RandomEventType[] = [
  'doze',
  'think',
  'shock',
  'love',
];

const BLINK_MIN_DELAY_MS = 2400;
const BLINK_MAX_DELAY_MS = 5200;
const BLINK_DURATION_MS = 150;


export interface UseCharacterEventsOptions {
  musicRunning: boolean;
  activityMode: ActivityMode;
  emotion: EmotionType;
  eventSwitches: EventSwitchMatrix;
  forcedEventPreview?: RandomEventType | null;
}

export interface CharacterEventsState {
  isBlinking: boolean;
  activeEvent: RandomEventType | null;
  allowedEvents: RandomEventType[];
}

export function isRandomEventAllowed(
  event: RandomEventType,
  activityMode: ActivityMode,
  musicRunning: boolean,
  eventSwitches: EventSwitchMatrix
): boolean {
  if (musicRunning && event === 'doze') {
    return false;
  }
  const modeAllows = eventSwitches[activityMode]?.[event] === true;
  if (!modeAllows) {
    return false;
  }
  if (musicRunning) {
    return eventSwitches.music?.[event] === true;
  }
  return true;
}

export function getAllowedRandomEvents(
  activityMode: ActivityMode,
  musicRunning: boolean,
  eventSwitches: EventSwitchMatrix
): RandomEventType[] {
  return RANDOM_EVENT_ORDER.filter((ev) =>
    isRandomEventAllowed(ev, activityMode, musicRunning, eventSwitches)
  );
}

function randomBetween(minMs: number, maxMs: number): number {
  return minMs + Math.random() * (maxMs - minMs);
}

export function useCharacterEvents({
  musicRunning,
  activityMode,
  emotion,
  eventSwitches,
  forcedEventPreview = null,
}: UseCharacterEventsOptions): CharacterEventsState {
  const [blinkActive, setBlinkActive] = useState(false);


  const allowedEvents = useMemo(
    () => getAllowedRandomEvents(activityMode, musicRunning, eventSwitches),
    [activityMode, musicRunning, eventSwitches]
  );

  // Compute effective active event synchronously so state prohibitions (especially musicRunning -> no doze) apply immediately on render
  const activeEvent: RandomEventType | null = useMemo(() => {
    if (forcedEventPreview) {
      if (musicRunning && forcedEventPreview === 'doze') {
        return null;
      }
      return forcedEventPreview;
    }
    return null;
  }, [forcedEventPreview, musicRunning]);

  const suppressBlink =
    emotion === 'happy' || activeEvent === 'love' || activeEvent === 'doze';

  const suppressBlinkRef = useRef(suppressBlink);
  suppressBlinkRef.current = suppressBlink;

  // Clear blink immediately when entering a suppressed expression/event
  useEffect(() => {
    if (suppressBlink) {
      setBlinkActive(false);
    }
  }, [suppressBlink]);

  // Natural blink timer (2400ms ~ 5200ms interval, ~150ms blink)
  useEffect(() => {
    let disposed = false;
    let nextBlinkTimer: number | null = null;
    let endBlinkTimer: number | null = null;

    const scheduleBlink = () => {
      if (disposed) return;
      nextBlinkTimer = window.setTimeout(() => {
        nextBlinkTimer = null;
        if (disposed) return;

        if (suppressBlinkRef.current) {
          scheduleBlink();
          return;
        }

        setBlinkActive(true);
        endBlinkTimer = window.setTimeout(() => {
          endBlinkTimer = null;
          if (disposed) return;
          setBlinkActive(false);
          scheduleBlink();
        }, BLINK_DURATION_MS);
      }, randomBetween(BLINK_MIN_DELAY_MS, BLINK_MAX_DELAY_MS));
    };

    scheduleBlink();

    return () => {
      disposed = true;
      if (nextBlinkTimer !== null) window.clearTimeout(nextBlinkTimer);
      if (endBlinkTimer !== null) window.clearTimeout(endBlinkTimer);
    };
  }, []);

  return {
    isBlinking: suppressBlink ? false : blinkActive,
    activeEvent,
    allowedEvents,
  };
}

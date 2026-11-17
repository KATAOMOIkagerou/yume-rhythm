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

const EVENT_MIN_INTERVAL_MS = 8500;
const EVENT_MAX_INTERVAL_MS = 14500;
const EVENT_DURATION_MS = 3200;

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
  const [randomEvent, setRandomEvent] = useState<RandomEventType | null>(null);

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
    if (musicRunning && randomEvent === 'doze') {
      return null;
    }
    return randomEvent;
  }, [forcedEventPreview, musicRunning, randomEvent]);

  const suppressBlink =
    emotion === 'happy' || activeEvent === 'love' || activeEvent === 'doze';

  const allowedEventsRef = useRef<RandomEventType[]>(allowedEvents);
  allowedEventsRef.current = allowedEvents;

  const forcedPreviewRef = useRef<RandomEventType | null | undefined>(forcedEventPreview);
  forcedPreviewRef.current = forcedEventPreview;

  const suppressBlinkRef = useRef<boolean>(suppressBlink);
  suppressBlinkRef.current = suppressBlink;

  const nextEventTimerRef = useRef<number | null>(null);
  const endEventTimerRef = useRef<number | null>(null);
  const scheduleNextEventRef = useRef<() => void>(() => {});

  // Random event state machine (8.5s ~ 14.5s interval, 3200ms duration)
  useEffect(() => {
    let disposed = false;

    const clearEventTimers = () => {
      if (nextEventTimerRef.current !== null) {
        window.clearTimeout(nextEventTimerRef.current);
        nextEventTimerRef.current = null;
      }
      if (endEventTimerRef.current !== null) {
        window.clearTimeout(endEventTimerRef.current);
        endEventTimerRef.current = null;
      }
    };

    const scheduleNext = () => {
      if (disposed) return;
      if (nextEventTimerRef.current !== null) {
        window.clearTimeout(nextEventTimerRef.current);
      }
      nextEventTimerRef.current = window.setTimeout(() => {
        nextEventTimerRef.current = null;
        if (disposed) return;

        const pool = allowedEventsRef.current;
        if (forcedPreviewRef.current || pool.length === 0) {
          scheduleNext();
          return;
        }

        const chosen = pool[Math.floor(Math.random() * pool.length)];
        setRandomEvent(chosen);

        endEventTimerRef.current = window.setTimeout(() => {
          endEventTimerRef.current = null;
          if (disposed) return;
          setRandomEvent(null);
          scheduleNext();
        }, EVENT_DURATION_MS);
      }, randomBetween(EVENT_MIN_INTERVAL_MS, EVENT_MAX_INTERVAL_MS));
    };

    scheduleNextEventRef.current = scheduleNext;
    scheduleNext();

    return () => {
      disposed = true;
      clearEventTimers();
    };
  }, []);

  // Immediately clear any active random event that is now prohibited (e.g. musicRunning starts while doze is active)
  useEffect(() => {
    if (
      randomEvent !== null &&
      !isRandomEventAllowed(randomEvent, activityMode, musicRunning, eventSwitches)
    ) {
      if (endEventTimerRef.current !== null) {
        window.clearTimeout(endEventTimerRef.current);
        endEventTimerRef.current = null;
      }
      setRandomEvent(null);
      if (nextEventTimerRef.current === null) {
        scheduleNextEventRef.current();
      }
    }
  }, [randomEvent, activityMode, musicRunning, eventSwitches]);

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

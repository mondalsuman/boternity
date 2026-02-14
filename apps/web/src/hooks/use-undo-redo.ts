/**
 * Generic undo/redo hook for workflow canvas state.
 *
 * Stores snapshots in past/future ref arrays using structuredClone
 * for deep copies. Max 50 history entries to cap memory usage.
 *
 * Usage:
 *   const { takeSnapshot, undo, redo, canUndo, canRedo } = useUndoRedo();
 *   // Call takeSnapshot(state) before each mutation
 *   // Call undo()/redo() to navigate history
 */

import { useCallback, useRef, useState } from "react";

const MAX_HISTORY = 50;

export interface UndoRedoState<T> {
  /** Take a snapshot of current state before a mutation. */
  takeSnapshot: (state: T) => void;
  /** Undo: restore the most recent past snapshot. Returns it, or undefined. */
  undo: (currentState: T) => T | undefined;
  /** Redo: restore the most recent future snapshot. Returns it, or undefined. */
  redo: (currentState: T) => T | undefined;
  /** Whether undo is available. */
  canUndo: boolean;
  /** Whether redo is available. */
  canRedo: boolean;
  /** Reset all history. */
  reset: () => void;
}

/**
 * useUndoRedo - maintains past/future stacks for arbitrary state T.
 *
 * The caller is responsible for applying the returned state from undo/redo.
 * This hook only manages the snapshot stacks.
 */
export function useUndoRedo<T>(): UndoRedoState<T> {
  const pastRef = useRef<T[]>([]);
  const futureRef = useRef<T[]>([]);

  // Version counter to trigger re-renders when stacks change
  const [, setVersion] = useState(0);
  const bump = useCallback(() => setVersion((v) => v + 1), []);

  const takeSnapshot = useCallback(
    (state: T) => {
      const clone = structuredClone(state);
      pastRef.current.push(clone);
      if (pastRef.current.length > MAX_HISTORY) {
        pastRef.current.shift();
      }
      // Taking a new snapshot clears the future (new branch)
      futureRef.current = [];
      bump();
    },
    [bump],
  );

  const undo = useCallback(
    (currentState: T): T | undefined => {
      const past = pastRef.current;
      if (past.length === 0) return undefined;

      const previous = past.pop()!;
      futureRef.current.push(structuredClone(currentState));
      bump();
      return previous;
    },
    [bump],
  );

  const redo = useCallback(
    (currentState: T): T | undefined => {
      const future = futureRef.current;
      if (future.length === 0) return undefined;

      const next = future.pop()!;
      pastRef.current.push(structuredClone(currentState));
      bump();
      return next;
    },
    [bump],
  );

  const reset = useCallback(() => {
    pastRef.current = [];
    futureRef.current = [];
    bump();
  }, [bump]);

  return {
    takeSnapshot,
    undo,
    redo,
    canUndo: pastRef.current.length > 0,
    canRedo: futureRef.current.length > 0,
    reset,
  };
}

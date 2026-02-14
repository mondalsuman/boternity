import { useCallback, useEffect, useRef, useState } from "react";

/**
 * Returns a debounced version of the given callback.
 * The callback is invoked after `delay` ms of inactivity.
 *
 * The debounced function is stable (same reference across renders)
 * and will cancel pending calls on unmount.
 *
 * @param callback - The function to debounce.
 * @param delay - Debounce delay in milliseconds.
 * @returns A stable debounced callback and a cancel function.
 */
export function useDebouncedCallback<T extends (...args: never[]) => void>(
  callback: T,
  delay: number,
): [(...args: Parameters<T>) => void, () => void] {
  const callbackRef = useRef(callback);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Always keep the latest callback in the ref.
  useEffect(() => {
    callbackRef.current = callback;
  }, [callback]);

  const cancel = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const debouncedFn = useCallback(
    (...args: Parameters<T>) => {
      cancel();
      timerRef.current = setTimeout(() => {
        callbackRef.current(...args);
      }, delay);
    },
    [delay, cancel],
  );

  // Cancel on unmount.
  useEffect(() => cancel, [cancel]);

  return [debouncedFn, cancel];
}

/**
 * Returns a debounced version of the given value.
 * The returned value updates only after `delay` ms of the input value
 * remaining stable.
 *
 * @param value - The value to debounce.
 * @param delay - Debounce delay in milliseconds.
 * @returns The debounced value.
 */
export function useDebounce<T>(value: T, delay: number): T {
  const [debouncedValue, setDebouncedValue] = useState(value);

  useEffect(() => {
    const timer = setTimeout(() => setDebouncedValue(value), delay);
    return () => clearTimeout(timer);
  }, [value, delay]);

  return debouncedValue;
}

/**
 * Shared utilities for workflow custom nodes.
 */

/**
 * Returns Tailwind classes for node border based on step execution status.
 *
 * - idle/pending: default muted border
 * - running: blue border with pulse animation
 * - completed: green border
 * - failed: red border
 */
export function nodeStatusClass(status?: string): string {
  switch (status) {
    case "running":
      return "border-blue-500 animate-pulse";
    case "completed":
      return "border-green-500";
    case "failed":
      return "border-red-500";
    case "skipped":
      return "border-muted-foreground/30 opacity-60";
    case "waiting_approval":
      return "border-yellow-500";
    default:
      return "border-border";
  }
}

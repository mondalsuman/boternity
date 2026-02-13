/**
 * Budget usage indicator for agent hierarchy requests.
 *
 * Shows a progress bar with token usage and estimated cost.
 * Color-coded: green < 50%, yellow 50-80%, red > 80%.
 * When budget warning fires: shows Continue/Stop buttons.
 * When budget exhausted: shows red alert with final count.
 *
 * Only visible when there are active agents (budget is being tracked).
 */

import { memo } from "react";
import { AlertTriangle, DollarSign } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface BudgetIndicatorProps {
  tokensUsed: number;
  budgetTotal: number;
  percentage: number;
  warning: boolean;
  exhausted: boolean;
  onContinue: () => void;
  onStop: () => void;
}

/** Rough cost estimate based on tokens.
 * Uses a blended rate (~$3/1M input + $15/1M output) averaged to ~$9/1M.
 * This is just a UI hint, not exact billing.
 */
function estimateCost(tokens: number): string {
  const costPerMillion = 9;
  const cost = (tokens / 1_000_000) * costPerMillion;
  if (cost < 0.01) return "<$0.01";
  return `~$${cost.toFixed(2)}`;
}

/** Format token count with thousands separator */
function formatTokens(tokens: number): string {
  return tokens.toLocaleString();
}

/** Progress bar color based on percentage */
function barColor(percentage: number): string {
  if (percentage > 80) return "bg-red-500";
  if (percentage > 50) return "bg-yellow-500";
  return "bg-green-500";
}

export const BudgetIndicator = memo(function BudgetIndicator({
  tokensUsed,
  budgetTotal,
  percentage,
  warning,
  exhausted,
  onContinue,
  onStop,
}: BudgetIndicatorProps) {
  // Only show when budget tracking is active
  if (budgetTotal === 0) return null;

  return (
    <div className="space-y-2">
      {/* Main budget bar */}
      <div className="flex items-center gap-3 px-3 py-2 bg-muted/30 rounded-lg">
        <DollarSign className="size-3.5 text-muted-foreground shrink-0" />

        {/* Progress bar */}
        <div className="flex-1 min-w-0">
          <div className="h-1.5 bg-muted rounded-full overflow-hidden">
            <div
              className={cn(
                "h-full rounded-full transition-all duration-300",
                barColor(percentage),
              )}
              style={{ width: `${Math.min(percentage, 100)}%` }}
            />
          </div>
        </div>

        {/* Token count + cost */}
        <span className="text-[10px] text-muted-foreground whitespace-nowrap shrink-0">
          {formatTokens(tokensUsed)} / {formatTokens(budgetTotal)} (
          {estimateCost(tokensUsed)})
        </span>
      </div>

      {/* Budget warning prompt */}
      {warning && !exhausted && (
        <div className="flex items-center gap-2 px-3 py-2 bg-yellow-500/10 border border-yellow-500/30 rounded-lg">
          <AlertTriangle className="size-4 text-yellow-500 shrink-0" />
          <span className="text-xs text-yellow-200 flex-1">
            Budget threshold reached ({Math.round(percentage)}% used). Continue?
          </span>
          <div className="flex items-center gap-1.5 shrink-0">
            <Button
              variant="outline"
              size="sm"
              className="h-6 text-xs px-2"
              onClick={onStop}
            >
              Stop
            </Button>
            <Button
              size="sm"
              className="h-6 text-xs px-2"
              onClick={onContinue}
            >
              Continue
            </Button>
          </div>
        </div>
      )}

      {/* Budget exhausted alert */}
      {exhausted && (
        <div className="flex items-center gap-2 px-3 py-2 bg-red-500/10 border border-red-500/30 rounded-lg">
          <AlertTriangle className="size-4 text-red-500 shrink-0" />
          <span className="text-xs text-red-200">
            Budget exhausted: {formatTokens(tokensUsed)} tokens used
          </span>
        </div>
      )}
    </div>
  );
});

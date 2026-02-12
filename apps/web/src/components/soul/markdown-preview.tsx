import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { cn } from "@/lib/utils";

interface MarkdownPreviewProps {
  content: string;
  className?: string;
}

/**
 * Rendered markdown preview panel.
 * Uses react-markdown with GFM support for tables, strikethrough, etc.
 */
export function MarkdownPreview({ content, className }: MarkdownPreviewProps) {
  if (!content.trim()) {
    return (
      <div
        className={cn(
          "flex items-center justify-center text-muted-foreground text-sm h-full min-h-[200px]",
          className,
        )}
      >
        No content to preview
      </div>
    );
  }

  return (
    <div
      className={cn(
        "prose prose-sm dark:prose-invert max-w-none overflow-auto p-4",
        className,
      )}
    >
      <ReactMarkdown remarkPlugins={[remarkGfm]}>{content}</ReactMarkdown>
    </div>
  );
}

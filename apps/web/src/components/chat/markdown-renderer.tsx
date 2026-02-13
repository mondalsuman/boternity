/**
 * Markdown renderer for chat messages.
 *
 * Uses react-markdown with GFM support and syntax highlighting via
 * rehype-highlight. Custom component overrides style markdown elements
 * with Tailwind classes matching the app's dark-first theme.
 *
 * Code blocks include a copy-to-clipboard button with toast feedback.
 * Memoized to prevent re-renders when parent updates but content is unchanged.
 */

import {
  memo,
  useCallback,
  useRef,
  type ComponentPropsWithoutRef,
  type ReactNode,
} from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import { Check, Copy } from "lucide-react";
import { toast } from "sonner";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";

import "highlight.js/styles/github-dark.css";

interface MarkdownRendererProps {
  content: string;
}

/**
 * Copy button for code blocks.
 * Extracts text from the code element and copies to clipboard.
 * Shows a check icon briefly after copying.
 */
function CopyCodeButton({ code }: { code: string }) {
  const copiedRef = useRef(false);
  const buttonRef = useRef<HTMLButtonElement>(null);

  const handleCopy = useCallback(async () => {
    if (copiedRef.current) return;
    try {
      await navigator.clipboard.writeText(code);
      copiedRef.current = true;
      toast.success("Copied to clipboard");

      // Swap icon briefly
      const btn = buttonRef.current;
      if (btn) {
        btn.dataset.copied = "true";
        setTimeout(() => {
          btn.dataset.copied = "false";
          copiedRef.current = false;
        }, 2000);
      }
    } catch {
      toast.error("Failed to copy");
    }
  }, [code]);

  return (
    <Button
      ref={buttonRef}
      variant="ghost"
      size="icon-xs"
      className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity text-muted-foreground hover:text-foreground data-[copied=true]:text-green-400"
      onClick={handleCopy}
      aria-label="Copy code"
      data-copied="false"
    >
      <Copy className="size-3.5 data-[copied=true]:hidden" />
      <Check className="size-3.5 hidden data-[copied=true]:block" />
    </Button>
  );
}

/**
 * Extracts plain text content from a React node tree.
 * Used to get the text content of a code block for copying.
 */
function extractTextContent(node: ReactNode): string {
  if (typeof node === "string") return node;
  if (typeof node === "number") return String(node);
  if (node == null || typeof node === "boolean") return "";
  if (Array.isArray(node)) return node.map(extractTextContent).join("");
  if (typeof node === "object" && "props" in node) {
    return extractTextContent(node.props.children);
  }
  return "";
}

/**
 * Custom component overrides for react-markdown.
 * Each element is styled with Tailwind to match the dark-first design.
 */
const markdownComponents = {
  // --- Code blocks ---
  pre({ children, ...props }: ComponentPropsWithoutRef<"pre">) {
    // Extract text for copy button from the nested <code> element
    const codeText = extractTextContent(children);

    return (
      <div className="relative group my-3">
        <pre
          className="overflow-x-auto rounded-lg bg-[#0d1117] p-4 text-sm leading-relaxed"
          {...props}
        >
          {children}
        </pre>
        <CopyCodeButton code={codeText} />
      </div>
    );
  },

  // Inline code vs block code (block code is inside <pre>)
  code({
    className,
    children,
    ...props
  }: ComponentPropsWithoutRef<"code">) {
    // If className contains "hljs" or a language class, it's a highlighted block
    const isBlock =
      className?.includes("hljs") || className?.startsWith("language-");

    if (isBlock) {
      return (
        <code className={cn("font-mono text-sm", className)} {...props}>
          {children}
        </code>
      );
    }

    // Inline code
    return (
      <code
        className="bg-muted px-1.5 py-0.5 rounded text-sm font-mono text-foreground"
        {...props}
      >
        {children}
      </code>
    );
  },

  // --- Tables ---
  table({ children, ...props }: ComponentPropsWithoutRef<"table">) {
    return (
      <div className="my-3 overflow-x-auto rounded-lg border">
        <table className="w-full border-collapse text-sm" {...props}>
          {children}
        </table>
      </div>
    );
  },
  thead({ children, ...props }: ComponentPropsWithoutRef<"thead">) {
    return (
      <thead className="bg-muted/50" {...props}>
        {children}
      </thead>
    );
  },
  th({ children, ...props }: ComponentPropsWithoutRef<"th">) {
    return (
      <th
        className="border-b px-3 py-2 text-left font-semibold text-foreground"
        {...props}
      >
        {children}
      </th>
    );
  },
  td({ children, ...props }: ComponentPropsWithoutRef<"td">) {
    return (
      <td className="border-b px-3 py-2 text-foreground" {...props}>
        {children}
      </td>
    );
  },

  // --- Links ---
  a({
    href,
    children,
    ...props
  }: ComponentPropsWithoutRef<"a">) {
    const isExternal = href?.startsWith("http");
    return (
      <a
        href={href}
        className="text-primary underline underline-offset-2 hover:text-primary/80 transition-colors"
        {...(isExternal ? { target: "_blank", rel: "noopener noreferrer" } : {})}
        {...props}
      >
        {children}
      </a>
    );
  },

  // --- Lists ---
  ul({ children, ...props }: ComponentPropsWithoutRef<"ul">) {
    return (
      <ul className="my-2 ml-6 list-disc space-y-1 text-foreground" {...props}>
        {children}
      </ul>
    );
  },
  ol({ children, ...props }: ComponentPropsWithoutRef<"ol">) {
    return (
      <ol
        className="my-2 ml-6 list-decimal space-y-1 text-foreground"
        {...props}
      >
        {children}
      </ol>
    );
  },
  li({ children, ...props }: ComponentPropsWithoutRef<"li">) {
    return (
      <li className="leading-relaxed" {...props}>
        {children}
      </li>
    );
  },

  // --- Block elements ---
  blockquote({ children, ...props }: ComponentPropsWithoutRef<"blockquote">) {
    return (
      <blockquote
        className="my-2 border-l-4 border-primary/30 pl-4 italic text-muted-foreground"
        {...props}
      >
        {children}
      </blockquote>
    );
  },

  // --- Headings ---
  h1({ children, ...props }: ComponentPropsWithoutRef<"h1">) {
    return (
      <h1
        className="mt-4 mb-2 text-xl font-bold text-foreground"
        {...props}
      >
        {children}
      </h1>
    );
  },
  h2({ children, ...props }: ComponentPropsWithoutRef<"h2">) {
    return (
      <h2
        className="mt-3 mb-2 text-lg font-semibold text-foreground"
        {...props}
      >
        {children}
      </h2>
    );
  },
  h3({ children, ...props }: ComponentPropsWithoutRef<"h3">) {
    return (
      <h3
        className="mt-3 mb-1.5 text-base font-semibold text-foreground"
        {...props}
      >
        {children}
      </h3>
    );
  },
  h4({ children, ...props }: ComponentPropsWithoutRef<"h4">) {
    return (
      <h4
        className="mt-2 mb-1 text-sm font-semibold text-foreground"
        {...props}
      >
        {children}
      </h4>
    );
  },

  // --- Paragraph ---
  p({ children, ...props }: ComponentPropsWithoutRef<"p">) {
    return (
      <p className="my-1.5 leading-relaxed" {...props}>
        {children}
      </p>
    );
  },

  // --- Horizontal rule ---
  hr(props: ComponentPropsWithoutRef<"hr">) {
    return <hr className="my-4 border-border" {...props} />;
  },
};

export const MarkdownRenderer = memo(function MarkdownRenderer({
  content,
}: MarkdownRendererProps) {
  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      rehypePlugins={[rehypeHighlight]}
      components={markdownComponents}
    >
      {content}
    </ReactMarkdown>
  );
});

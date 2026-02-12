/**
 * Shared chat layout wrapper used by both /chat/ and /chat/$sessionId routes.
 *
 * Two-panel layout: session sidebar (left, 280px) + main content area (right).
 * Since TanStack Router treats /chat/ and /chat/$sessionId as sibling routes
 * (not nested), this layout component is shared between both.
 */

import { SessionSidebar } from "@/components/chat/session-sidebar";

interface ChatLayoutProps {
  activeSessionId?: string;
  children: React.ReactNode;
}

export function ChatLayout({ activeSessionId, children }: ChatLayoutProps) {
  return (
    <div className="flex h-[calc(100vh-3rem)]">
      {/* Session sidebar */}
      <div className="w-72 border-r shrink-0 hidden md:block">
        <SessionSidebar activeSessionId={activeSessionId} />
      </div>

      {/* Main chat area */}
      <div className="flex-1 min-w-0">{children}</div>
    </div>
  );
}

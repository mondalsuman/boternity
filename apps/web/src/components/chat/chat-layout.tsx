/**
 * Shared chat layout wrapper used by both /chat/ and /chat/$sessionId routes.
 *
 * Two-panel layout: session sidebar (left, 280px) + main content area (right).
 * On mobile, session sidebar is accessible via a sheet/drawer trigger.
 * Since TanStack Router treats /chat/ and /chat/$sessionId as sibling routes
 * (not nested), this layout component is shared between both.
 */

import { useState } from "react";
import { MessageCircle } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetDescription,
} from "@/components/ui/sheet";
import { SessionSidebar } from "@/components/chat/session-sidebar";

interface ChatLayoutProps {
  activeSessionId?: string;
  children: React.ReactNode;
}

export function ChatLayout({ activeSessionId, children }: ChatLayoutProps) {
  const [mobileSessionsOpen, setMobileSessionsOpen] = useState(false);

  return (
    <div className="flex h-[calc(100vh-3rem)]">
      {/* Desktop: session sidebar */}
      <div className="w-72 border-r shrink-0 hidden md:block">
        <SessionSidebar activeSessionId={activeSessionId} />
      </div>

      {/* Mobile: session sidebar as sheet */}
      <Sheet open={mobileSessionsOpen} onOpenChange={setMobileSessionsOpen}>
        <SheetContent side="left" className="w-72 p-0">
          <SheetHeader className="sr-only">
            <SheetTitle>Sessions</SheetTitle>
            <SheetDescription>Chat session list</SheetDescription>
          </SheetHeader>
          <SessionSidebar activeSessionId={activeSessionId} />
        </SheetContent>
      </Sheet>

      {/* Main chat area */}
      <div className="flex-1 min-w-0 relative">
        {/* Mobile session list trigger */}
        <Button
          variant="ghost"
          size="icon"
          className="absolute top-3 left-2 z-10 md:hidden"
          onClick={() => setMobileSessionsOpen(true)}
          aria-label="Open sessions"
        >
          <MessageCircle className="size-4" />
        </Button>
        {children}
      </div>
    </div>
  );
}

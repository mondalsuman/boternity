import { Link, useMatches } from "@tanstack/react-router";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";

/**
 * Route-aware breadcrumbs.
 * Derives crumbs from the current route matches.
 * Each segment is a link except the last (current page).
 */

interface BreadcrumbEntry {
  label: string;
  path: string;
}

function pathToLabel(segment: string): string {
  if (segment === "") return "Dashboard";
  // Capitalize and replace hyphens with spaces
  return segment
    .replace(/-/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

export function Breadcrumbs() {
  const matches = useMatches();

  // Build breadcrumb entries from route matches
  const crumbs: BreadcrumbEntry[] = [];

  for (const match of matches) {
    const path = match.pathname;
    // Skip the root route
    if (path === "/" && matches.length > 1) {
      crumbs.push({ label: "Dashboard", path: "/" });
      continue;
    }
    if (path === "/") {
      crumbs.push({ label: "Dashboard", path: "/" });
      continue;
    }
    // Extract the last segment for the label
    const segments = path.split("/").filter(Boolean);
    const lastSegment = segments[segments.length - 1] || "";

    // Skip if we'd duplicate the previous crumb
    if (crumbs.length > 0 && crumbs[crumbs.length - 1].path === path) {
      continue;
    }

    // Check if it looks like a UUID (skip labeling UUIDs, they'll be replaced by bot names etc.)
    const isUuid =
      /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
        lastSegment,
      );

    crumbs.push({
      label: isUuid ? lastSegment.slice(0, 8) + "..." : pathToLabel(lastSegment),
      path,
    });
  }

  // Deduplicate
  const uniqueCrumbs = crumbs.filter(
    (crumb, index, arr) =>
      index === 0 || crumb.path !== arr[index - 1].path,
  );

  if (uniqueCrumbs.length <= 1) return null;

  // On mobile, show only first + last crumb to save space
  const displayCrumbs =
    uniqueCrumbs.length > 2
      ? [uniqueCrumbs[0], uniqueCrumbs[uniqueCrumbs.length - 1]]
      : uniqueCrumbs;

  return (
    <Breadcrumb>
      <BreadcrumbList className="flex-nowrap overflow-hidden">
        {/* Full breadcrumbs on desktop */}
        {uniqueCrumbs.map((crumb, index) => {
          const isLast = index === uniqueCrumbs.length - 1;
          return (
            <span
              key={crumb.path}
              className={`items-center gap-1.5 ${
                uniqueCrumbs.length > 2 && index > 0 && !isLast
                  ? "hidden md:flex"
                  : "flex"
              }`}
            >
              {index > 0 && <BreadcrumbSeparator />}
              <BreadcrumbItem>
                {isLast ? (
                  <BreadcrumbPage className="truncate max-w-[150px] md:max-w-none">
                    {crumb.label}
                  </BreadcrumbPage>
                ) : (
                  <BreadcrumbLink asChild>
                    <Link to={crumb.path} className="truncate">
                      {crumb.label}
                    </Link>
                  </BreadcrumbLink>
                )}
              </BreadcrumbItem>
            </span>
          );
        })}
      </BreadcrumbList>
    </Breadcrumb>
  );
}

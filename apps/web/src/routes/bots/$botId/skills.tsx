import { useState } from "react";
import { createFileRoute } from "@tanstack/react-router";
import {
  MessageSquare,
  Wrench,
  Plus,
  X,
  Search,
  Download,
  ChevronDown,
  ChevronRight,
  Loader2,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import {
  useAllSkills,
  useBotSkills,
  useAttachSkill,
  useDetachSkill,
  useToggleSkill,
  useRegistrySearch,
  useInstallSkill,
} from "@/hooks/use-skill-queries";
import type {
  InstalledSkill,
  BotSkillConfig,
  DiscoveredSkill,
  TrustTier,
} from "@/types/skill";
import { useDebounce } from "@/hooks/use-debounce";

export const Route = createFileRoute("/bots/$botId/skills")({
  component: BotSkillsPage,
});

// ---------------------------------------------------------------------------
// Trust tier badge colors
// ---------------------------------------------------------------------------

const TIER_COLORS: Record<TrustTier, string> = {
  local: "border-green-500 text-green-400",
  verified: "border-yellow-500 text-yellow-400",
  untrusted: "border-red-500 text-red-400",
};

const TIER_LABELS: Record<TrustTier, string> = {
  local: "Local",
  verified: "Verified",
  untrusted: "Untrusted",
};

function TrustBadge({ tier }: { tier: TrustTier | null }) {
  if (!tier) return null;
  return (
    <Badge variant="outline" className={TIER_COLORS[tier]}>
      {TIER_LABELS[tier]}
    </Badge>
  );
}

function SkillTypeBadge({ type }: { type: string | null }) {
  if (!type) return null;
  const Icon = type === "tool" ? Wrench : MessageSquare;
  return (
    <Badge variant="secondary" className="gap-1">
      <Icon className="size-3" />
      {type}
    </Badge>
  );
}

// ---------------------------------------------------------------------------
// Main page component
// ---------------------------------------------------------------------------

function BotSkillsPage() {
  const { botId } = Route.useParams();
  const { data: botSkills, isLoading: botSkillsLoading } =
    useBotSkills(botId);
  const { data: allSkills, isLoading: allSkillsLoading } = useAllSkills();
  const [discoverOpen, setDiscoverOpen] = useState(false);

  const isLoading = botSkillsLoading || allSkillsLoading;

  // Compute available skills (installed globally but not attached to this bot)
  const attachedNames = new Set(botSkills?.map((s) => s.name) ?? []);
  const availableSkills =
    allSkills?.filter((s) => !attachedNames.has(s.name)) ?? [];

  if (isLoading) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-8 w-48" />
        <Skeleton className="h-32" />
        <Skeleton className="h-32" />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Section 1: Attached Skills */}
      <section>
        <div className="flex items-center justify-between mb-3">
          <div>
            <h2 className="text-lg font-semibold tracking-tight">
              Attached Skills
            </h2>
            <p className="text-sm text-muted-foreground">
              Skills active on this bot.
            </p>
          </div>
        </div>

        {botSkills && botSkills.length > 0 ? (
          <div className="grid gap-3 md:grid-cols-2">
            {botSkills.map((skill) => (
              <AttachedSkillCard
                key={skill.name}
                skill={skill}
                botId={botId}
              />
            ))}
          </div>
        ) : (
          <Card>
            <CardContent className="py-8 text-center text-muted-foreground">
              No skills attached. Add skills from the Available section below.
            </CardContent>
          </Card>
        )}
      </section>

      {/* Section 2: Available Skills */}
      <section>
        <div className="mb-3">
          <h2 className="text-lg font-semibold tracking-tight">
            Available Skills
          </h2>
          <p className="text-sm text-muted-foreground">
            Installed skills not yet attached to this bot.
          </p>
        </div>

        {availableSkills.length > 0 ? (
          <div className="grid gap-3 md:grid-cols-2">
            {availableSkills.map((skill) => (
              <AvailableSkillCard
                key={skill.name}
                skill={skill}
                botId={botId}
              />
            ))}
          </div>
        ) : (
          <Card>
            <CardContent className="py-8 text-center text-muted-foreground">
              {allSkills && allSkills.length > 0
                ? "All installed skills are attached to this bot."
                : "No skills installed. Discover skills from registries below."}
            </CardContent>
          </Card>
        )}
      </section>

      {/* Section 3: Discover Skills */}
      <section>
        <button
          type="button"
          onClick={() => setDiscoverOpen(!discoverOpen)}
          className="flex items-center gap-2 mb-3 text-lg font-semibold tracking-tight hover:text-foreground/80 transition-colors"
        >
          {discoverOpen ? (
            <ChevronDown className="size-5" />
          ) : (
            <ChevronRight className="size-5" />
          )}
          Discover Skills
        </button>

        {discoverOpen && <DiscoverSection />}
      </section>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Attached skill card
// ---------------------------------------------------------------------------

function AttachedSkillCard({
  skill,
  botId,
}: {
  skill: BotSkillConfig;
  botId: string;
}) {
  const toggleMutation = useToggleSkill();
  const detachMutation = useDetachSkill();

  return (
    <Card
      className={
        skill.enabled ? "" : "opacity-60"
      }
    >
      <CardHeader className="pb-2">
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0 flex-1">
            <CardTitle className="text-base truncate">{skill.name}</CardTitle>
            <CardDescription className="line-clamp-2">
              {skill.description || "No description"}
            </CardDescription>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            <Switch
              checked={skill.enabled}
              onCheckedChange={(checked) =>
                toggleMutation.mutate({
                  botId,
                  skillName: skill.name,
                  enabled: checked,
                })
              }
              disabled={toggleMutation.isPending}
            />
          </div>
        </div>
      </CardHeader>
      <CardContent>
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <SkillTypeBadge type={skill.skill_type} />
            <TrustBadge tier={skill.trust_tier} />
          </div>
          <AlertDialog>
            <AlertDialogTrigger asChild>
              <Button
                variant="ghost"
                size="sm"
                className="text-destructive hover:text-destructive"
              >
                <X className="size-4" />
                Detach
              </Button>
            </AlertDialogTrigger>
            <AlertDialogContent>
              <AlertDialogHeader>
                <AlertDialogTitle>
                  Detach {skill.name}?
                </AlertDialogTitle>
                <AlertDialogDescription>
                  This will remove the skill from this bot. The skill will
                  remain installed globally and can be re-attached later.
                </AlertDialogDescription>
              </AlertDialogHeader>
              <AlertDialogFooter>
                <AlertDialogCancel>Cancel</AlertDialogCancel>
                <AlertDialogAction
                  onClick={() =>
                    detachMutation.mutate({
                      botId,
                      skillName: skill.name,
                    })
                  }
                  className="bg-destructive text-white hover:bg-destructive/90"
                >
                  {detachMutation.isPending ? "Detaching..." : "Detach"}
                </AlertDialogAction>
              </AlertDialogFooter>
            </AlertDialogContent>
          </AlertDialog>
        </div>
      </CardContent>
    </Card>
  );
}

// ---------------------------------------------------------------------------
// Available skill card (attach action)
// ---------------------------------------------------------------------------

function AvailableSkillCard({
  skill,
  botId,
}: {
  skill: InstalledSkill;
  botId: string;
}) {
  const attachMutation = useAttachSkill();

  return (
    <Card>
      <CardHeader className="pb-2">
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0 flex-1">
            <CardTitle className="text-base truncate">{skill.name}</CardTitle>
            <CardDescription className="line-clamp-2">
              {skill.description || "No description"}
            </CardDescription>
          </div>
          <Button
            size="sm"
            variant="outline"
            className="shrink-0 gap-1"
            onClick={() =>
              attachMutation.mutate({ botId, skillName: skill.name })
            }
            disabled={attachMutation.isPending}
          >
            {attachMutation.isPending ? (
              <Loader2 className="size-3 animate-spin" />
            ) : (
              <Plus className="size-3" />
            )}
            Attach
          </Button>
        </div>
      </CardHeader>
      <CardContent>
        <div className="flex items-center gap-2">
          <SkillTypeBadge type={skill.skill_type} />
          <TrustBadge tier={skill.trust_tier} />
          {skill.version && (
            <Badge variant="secondary" className="text-xs">
              v{skill.version}
            </Badge>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

// ---------------------------------------------------------------------------
// Discover section (registry search)
// ---------------------------------------------------------------------------

function DiscoverSection() {
  const [searchInput, setSearchInput] = useState("");
  const debouncedQuery = useDebounce(searchInput, 300);
  const { data: results, isLoading } = useRegistrySearch(debouncedQuery);
  const installMutation = useInstallSkill();

  return (
    <div className="space-y-3">
      <div className="relative">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-muted-foreground" />
        <Input
          placeholder="Search skill registries..."
          value={searchInput}
          onChange={(e) => setSearchInput(e.target.value)}
          className="pl-9"
        />
      </div>

      {isLoading && (
        <div className="space-y-2">
          <Skeleton className="h-16" />
          <Skeleton className="h-16" />
        </div>
      )}

      {results && results.length > 0 && (
        <div className="space-y-2">
          {results.map((skill) => (
            <DiscoveredSkillRow
              key={`${skill.source}-${skill.name}`}
              skill={skill}
              onInstall={() =>
                installMutation.mutate({
                  source: skill.source,
                  skillName: skill.name,
                })
              }
              installing={installMutation.isPending}
            />
          ))}
        </div>
      )}

      {results && results.length === 0 && debouncedQuery.length >= 2 && (
        <p className="text-sm text-muted-foreground text-center py-4">
          No skills found for "{debouncedQuery}".
        </p>
      )}

      {!debouncedQuery && (
        <p className="text-sm text-muted-foreground text-center py-4">
          Type at least 2 characters to search skill registries.
        </p>
      )}
    </div>
  );
}

function DiscoveredSkillRow({
  skill,
  onInstall,
  installing,
}: {
  skill: DiscoveredSkill;
  onInstall: () => void;
  installing: boolean;
}) {
  return (
    <div className="flex items-center justify-between rounded-md border px-4 py-3">
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="font-medium text-sm truncate">{skill.name}</span>
          <Badge variant="secondary" className="text-xs shrink-0">
            {skill.source}
          </Badge>
          <TrustBadge tier={skill.trust_tier} />
        </div>
        <p className="text-xs text-muted-foreground line-clamp-1">
          {skill.description}
        </p>
        {skill.categories.length > 0 && (
          <div className="flex items-center gap-1 mt-1">
            {skill.categories.slice(0, 3).map((cat) => (
              <Badge key={cat} variant="outline" className="text-xs">
                {cat}
              </Badge>
            ))}
          </div>
        )}
      </div>
      <div className="flex items-center gap-2 ml-3 shrink-0">
        {skill.install_count != null && (
          <span className="text-xs text-muted-foreground">
            {skill.install_count.toLocaleString()} installs
          </span>
        )}
        <Button
          size="sm"
          variant="outline"
          className="gap-1"
          onClick={onInstall}
          disabled={installing}
        >
          {installing ? (
            <Loader2 className="size-3 animate-spin" />
          ) : (
            <Download className="size-3" />
          )}
          Install
        </Button>
      </div>
    </div>
  );
}

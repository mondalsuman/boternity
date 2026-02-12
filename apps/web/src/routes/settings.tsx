import { createFileRoute } from "@tanstack/react-router";
import { Moon, Sun, Monitor } from "lucide-react";
import { type Theme, useThemeStore } from "@/stores/theme-store";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";

export const Route = createFileRoute("/settings")({
  component: SettingsPage,
});

function SettingsPage() {
  const { theme, setTheme } = useThemeStore();

  const themeOptions: { value: Theme; label: string; icon: React.ReactNode }[] = [
    { value: "dark", label: "Dark", icon: <Moon className="h-4 w-4" /> },
    { value: "light", label: "Light", icon: <Sun className="h-4 w-4" /> },
    { value: "system", label: "System", icon: <Monitor className="h-4 w-4" /> },
  ];

  return (
    <div className="p-6 space-y-6 max-w-2xl">
      <div>
        <h1 className="text-2xl font-bold tracking-tight">Settings</h1>
        <p className="text-muted-foreground">
          Configure your Boternity dashboard preferences.
        </p>
      </div>

      <Separator />

      {/* Theme */}
      <Card>
        <CardHeader>
          <CardTitle>Appearance</CardTitle>
          <CardDescription>
            Choose your preferred color theme for the dashboard.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="flex gap-2">
            {themeOptions.map((opt) => (
              <Button
                key={opt.value}
                variant={theme === opt.value ? "default" : "outline"}
                size="sm"
                onClick={() => setTheme(opt.value)}
                className="gap-2"
              >
                {opt.icon}
                {opt.label}
              </Button>
            ))}
          </div>
        </CardContent>
      </Card>

      {/* Backend API URL */}
      <Card>
        <CardHeader>
          <CardTitle>Backend Connection</CardTitle>
          <CardDescription>
            The API URL for the Boternity backend. Defaults to the local server.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <Input
            placeholder="http://localhost:3000"
            defaultValue="http://localhost:3000"
            disabled
          />
          <p className="text-xs text-muted-foreground mt-2">
            Currently using the Vite proxy. Direct API URL configuration coming
            in a future update.
          </p>
        </CardContent>
      </Card>
    </div>
  );
}

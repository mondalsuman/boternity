import { create } from "zustand";
import { persist } from "zustand/middleware";

export type Theme = "dark" | "light" | "system";

interface ThemeStore {
  theme: Theme;
  setTheme: (theme: Theme) => void;
}

export const useThemeStore = create<ThemeStore>()(
  persist(
    (set) => ({
      theme: "dark" as Theme, // Dark by default per user decision
      setTheme: (theme: Theme) => set({ theme }),
    }),
    { name: "boternity-theme" },
  ),
);

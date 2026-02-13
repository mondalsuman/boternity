import { create } from "zustand";
import { persist } from "zustand/middleware";

interface ApiKeyStore {
  apiKey: string | null;
  setApiKey: (key: string | null) => void;
}

export const useApiKeyStore = create<ApiKeyStore>()(
  persist(
    (set) => ({
      apiKey: null,
      setApiKey: (key: string | null) => set({ apiKey: key }),
    }),
    { name: "boternity-api-key" },
  ),
);

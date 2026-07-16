import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import { companionCommands, errorMessage } from "../lib/companion";
import { initialSnapshot, type CompanionSnapshot } from "../lib/types";

const SNAPSHOT_EVENT = "companion://snapshot";

export function useCompanion() {
  const [snapshot, setSnapshot] = useState<CompanionSnapshot>(initialSnapshot);
  const [loading, setLoading] = useState(true);
  const [pending, setPending] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    let stopListening: (() => void) | undefined;

    async function initialize() {
      const unlisten = await listen<CompanionSnapshot>(SNAPSHOT_EVENT, (event) => {
        if (active) setSnapshot(event.payload);
      });
      if (!active) {
        unlisten();
        return;
      }
      stopListening = unlisten;
      const current = await companionCommands.snapshot();
      if (active) setSnapshot(current);
    }

    initialize()
      .catch((error: unknown) => {
        if (active) setActionError(errorMessage(error));
      })
      .finally(() => {
        if (active) setLoading(false);
      });

    return () => {
      active = false;
      stopListening?.();
    };
  }, []);

  const run = useCallback(async (name: string, action: () => Promise<void>) => {
    setPending(name);
    setActionError(null);
    try {
      await action();
    } catch (error: unknown) {
      setActionError(errorMessage(error));
    } finally {
      setPending(null);
    }
  }, []);

  return { snapshot, loading, pending, actionError, clearError: () => setActionError(null), run };
}

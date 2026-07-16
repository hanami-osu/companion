import { useCallback, useEffect, useReducer, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import { companionCommands, errorMessage } from "../lib/companion";
import { initialSnapshot, type CompanionSnapshot } from "../lib/types";

const SNAPSHOT_EVENT = "companion://snapshot";

export type ActionName =
  | "tracking"
  | "tosu-auto-start"
  | "launch-tosu"
  | "stop-tosu"
  | "select-tosu"
  | "reset-tosu"
  | "grant-memory-access"
  | "connect-hanami"
  | "disconnect-hanami"
  | "open-tosu"
  | "open-hanami"
  | "open-repository";

interface ActionState {
  pending: Set<ActionName>;
  errors: Partial<Record<ActionName, string>>;
  latestError: { action: ActionName; message: string } | null;
}

type ActionEvent =
  | { type: "started"; action: ActionName }
  | { type: "succeeded"; action: ActionName }
  | { type: "failed"; action: ActionName; message: string }
  | { type: "clear-error"; action?: ActionName };

export const initialActionState: ActionState = {
  pending: new Set(),
  errors: {},
  latestError: null,
};

export function actionReducer(state: ActionState, event: ActionEvent): ActionState {
  const pending = new Set(state.pending);
  const errors = { ...state.errors };

  switch (event.type) {
    case "started":
      pending.add(event.action);
      delete errors[event.action];
      return {
        pending,
        errors,
        latestError: state.latestError?.action === event.action ? null : state.latestError,
      };
    case "succeeded":
      pending.delete(event.action);
      delete errors[event.action];
      return {
        pending,
        errors,
        latestError:
          state.latestError?.action === event.action ? null : state.latestError,
      };
    case "failed":
      pending.delete(event.action);
      errors[event.action] = event.message;
      return {
        pending,
        errors,
        latestError: { action: event.action, message: event.message },
      };
    case "clear-error":
      if (event.action) delete errors[event.action];
      else for (const action of Object.keys(errors) as ActionName[]) delete errors[action];
      return { pending, errors, latestError: null };
  }
}

export function useCompanion() {
  const [snapshot, setSnapshot] = useState<CompanionSnapshot>(initialSnapshot);
  const [loading, setLoading] = useState(true);
  const [actions, dispatch] = useReducer(actionReducer, initialActionState);

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
        if (active) {
          dispatch({
            type: "failed",
            action: "tracking",
            message: errorMessage(error),
          });
        }
      })
      .finally(() => {
        if (active) setLoading(false);
      });

    return () => {
      active = false;
      stopListening?.();
    };
  }, []);

  const run = useCallback(async (name: ActionName, action: () => Promise<void>) => {
    dispatch({ type: "started", action: name });
    try {
      await action();
      dispatch({ type: "succeeded", action: name });
    } catch (error: unknown) {
      dispatch({ type: "failed", action: name, message: errorMessage(error) });
    }
  }, []);

  return {
    snapshot,
    loading,
    pending: actions.pending,
    actionErrors: actions.errors,
    actionError: actions.latestError?.message ?? null,
    latestActionError: actions.latestError,
    clearError: () => dispatch({ type: "clear-error" }),
    run,
  };
}

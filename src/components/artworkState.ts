export interface ArtworkState {
    url: string | null;
    retryKey: string;
    failed: boolean;
    attempts: number;
    generation: number;
}

export type ArtworkEvent =
    { type: "source"; url: string | null; retryKey: string } | { type: "failed" } | { type: "retry" };

export function artworkReducer(state: ArtworkState, event: ArtworkEvent): ArtworkState {
    switch (event.type) {
        case "source":
            if (state.url === event.url && state.retryKey === event.retryKey) return state;
            return {
                url: event.url,
                retryKey: event.retryKey,
                failed: false,
                attempts: 0,
                generation: state.generation + 1,
            };
        case "failed":
            return { ...state, failed: true, attempts: state.attempts + 1 };
        case "retry":
            return { ...state, failed: false, generation: state.generation + 1 };
    }
}

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

const CONNECTION_EVENT = "tosu-connection-status";

function App() {
    const [tosuEnabled, setTosuEnabled] = useState(false);
    const [tosuConnected, setTosuConnected] = useState(false);
    const [tosuPending, setTosuPending] = useState(false);

    useEffect(() => {
        let mounted = true;
        let stopListening: (() => void) | undefined;

        const initialize = async () => {
            const unlisten = await listen<boolean>(CONNECTION_EVENT, (event) => {
                if (mounted) {
                    setTosuConnected(event.payload);
                }
            });

            if (!mounted) {
                unlisten();
                return;
            }

            stopListening = unlisten;

            const [enabled, connected] = await Promise.all([invoke<boolean>("is_tosu_running"), invoke<boolean>("is_tosu_connected")]);

            if (mounted) {
                setTosuEnabled(enabled);
                setTosuConnected(connected);
            }
        };

        initialize().catch(console.error);

        return () => {
            mounted = false;
            stopListening?.();
        };
    }, []);

    const toggleTosu = async () => {
        const enable = !tosuEnabled;

        setTosuPending(true);
        try {
            await invoke("toggle_tosu", { enable });
            setTosuEnabled(enable);
        } catch (error) {
            console.error("Failed to toggle Tosu:", error);
        } finally {
            setTosuPending(false);
        }
    };

    return (
        <div className="app-shell">
            <header className="app-header">
                <span>Hanami</span>
                <h1>Companion</h1>
            </header>

            <main className="app-main">
                <div className={`connection-status ${tosuConnected ? "is-connected" : ""}`} role="status" aria-live="polite">
                    <span className="connection-dot" aria-hidden="true" />
                    {tosuConnected ? "Connected" : "Not connected"}
                </div>

                <div className="tosu-copy">
                    <p>Local tracking</p>
                    <h2>Tosu</h2>
                    <span>Capture plays directly from osu!.</span>
                </div>

                <button type="button" className="tosu-control" onClick={toggleTosu} disabled={tosuPending} aria-pressed={tosuEnabled}>
                    <span>{tosuEnabled ? "Stop Tosu" : "Start Tosu"}</span>
                    <span className={`switch ${tosuEnabled ? "is-on" : ""}`} aria-hidden="true">
                        <span />
                    </span>
                </button>
            </main>
        </div>
    );
}

export default App;

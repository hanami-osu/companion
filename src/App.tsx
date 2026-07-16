import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

function App() {
    const [tosuRunning, setTosuRunning] = useState(false);
    const [hanamiConnected, setHanamiConnected] = useState(false);

    useEffect(() => {
        invoke<boolean>("is_tosu_running").then(setTosuRunning);
    }, []);

    const toggleTosu = async () => {
        const newState = !tosuRunning;
        try {
            await invoke("toggle_tosu", { enable: newState });
            setTosuRunning(newState);
        } catch (e) {
            console.error("Failed to toggle tosu:", e);
        }
    };

    const connectHanami = () => {
        // Mock connecting to Hanami Web
        console.log("Connecting to Hanami Web...");
        setTimeout(() => {
            setHanamiConnected(true);
        }, 1000);
    };

    return (
        <div className="min-h-screen bg-zinc-900 text-zinc-100 flex flex-col p-6 selection:bg-pink-500/30">
            <header className="mb-8 text-center">
                <h1 className="text-3xl font-bold bg-gradient-to-r from-pink-400 to-rose-400 bg-clip-text text-transparent">Hanami Companion</h1>
                <p className="text-zinc-400 text-sm mt-1">Local osu! Tracking</p>
            </header>

            <div className="space-y-6 flex-grow">
                <section className="bg-zinc-800/50 rounded-xl p-5 border border-zinc-700/50 shadow-lg backdrop-blur-sm">
                    <div className="flex items-center justify-between mb-4">
                        <div>
                            <h2 className="text-lg font-semibold flex items-center gap-2">
                                Tosu Tracking
                                <span className="relative flex h-3 w-3">
                                    {tosuRunning && <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"></span>}
                                    <span className={`relative inline-flex rounded-full h-3 w-3 ${tosuRunning ? "bg-emerald-500" : "bg-zinc-600"}`}></span>
                                </span>
                            </h2>
                            <p className="text-sm text-zinc-400">Reads memory to capture local plays.</p>
                        </div>

                        <button
                            onClick={toggleTosu}
                            className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-pink-500 focus:ring-offset-2 focus:ring-offset-zinc-900 ${
                                tosuRunning ? "bg-emerald-500" : "bg-zinc-600"
                            }`}
                        >
                            <span className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${tosuRunning ? "translate-x-6" : "translate-x-1"}`} />
                        </button>
                    </div>
                </section>

                <section className="bg-zinc-800/50 rounded-xl p-5 border border-zinc-700/50 shadow-lg backdrop-blur-sm">
                    <h2 className="text-lg font-semibold mb-1">Hanami Web</h2>
                    <p className="text-sm text-zinc-400 mb-4">Link your companion to send data to the bot.</p>

                    <div className="flex items-center justify-between">
                        <span className={`text-sm font-medium ${hanamiConnected ? "text-emerald-400" : "text-amber-400"}`}>{hanamiConnected ? "Linked to Hanami" : "Not Linked"}</span>
                        <button onClick={connectHanami} className="px-4 py-2 bg-pink-600 hover:bg-pink-500 text-white text-sm font-semibold rounded-lg transition-colors shadow-md shadow-pink-500/20">
                            {hanamiConnected ? "Reconnect" : "Connect"}
                        </button>
                    </div>
                </section>

                {/* Activity Log (Mock) */}
                <section className="mt-8">
                    <h3 className="text-sm font-semibold text-zinc-400 uppercase tracking-wider mb-3 px-1">Recent Activity</h3>
                    <div className="bg-zinc-950/50 rounded-xl p-4 h-32 overflow-y-auto border border-zinc-800/80 font-mono text-xs text-zinc-300">
                        <div className="text-emerald-400/80">System initialized. Waiting for osu!...</div>
                        {tosuRunning && <div className="text-emerald-400/80 mt-1">Tosu started. Monitoring memory.</div>}
                    </div>
                </section>
            </div>
        </div>
    );
}

export default App;

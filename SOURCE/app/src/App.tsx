import React, { createContext, useContext, useEffect, useState } from "react";
import { call } from "./api";
import Dashboard from "./pages/Dashboard";
import Market from "./pages/Market";
import Explorer from "./pages/Explorer";
import Generator from "./pages/Generator";
import Engine from "./pages/Engine";
import Simulator from "./pages/Simulator";
import PromptPage from "./pages/Prompt";
import Documents from "./pages/Documents";
import Library from "./pages/Library";
import Settings from "./pages/Settings";

export type PageKey = "dashboard" | "market" | "explorer" | "generator" | "engine" | "simulator" | "prompt" | "documents" | "library" | "settings";

const PAGES: [PageKey, string][] = [
  ["dashboard", "Dashboard"],
  ["market", "Market Intelligence"],
  ["explorer", "Slot Explorer"],
  ["generator", "Math Generator"],
  ["engine", "Math Engine"],
  ["simulator", "Spin Simulator"],
  ["prompt", "Prompt Generator"],
  ["documents", "Document Library"],
  ["library", "Math Library"],
  ["settings", "Settings"],
];

type Ctx = {
  go: (p: PageKey, params?: any) => void;
  params: any;
  model: { id: string; version?: number } | null;
  setModel: (m: { id: string; version?: number } | null) => void;
  info: any;
};

const AppCtx = createContext<Ctx>(null as any);
export const useApp = () => useContext(AppCtx);

export default function App() {
  const [page, setPage] = useState<PageKey>("dashboard");
  const [params, setParams] = useState<any>(null);
  const [model, setModel] = useState<{ id: string; version?: number } | null>(null);
  const [info, setInfo] = useState<any>(null);
  useEffect(() => {
    call("app_info").then(setInfo).catch(() => {});
  }, []);
  const go = (p: PageKey, prm?: any) => {
    setParams(prm ?? null);
    if (prm?.model) setModel(prm.model);
    setPage(p);
  };
  const Comp = { dashboard: Dashboard, market: Market, explorer: Explorer, generator: Generator, engine: Engine, simulator: Simulator, prompt: PromptPage, documents: Documents, library: Library, settings: Settings }[page];
  return (
    <AppCtx.Provider value={{ go, params, model, setModel, info }}>
      <div className="app">
        <aside className="side">
          <div className="brand">
            <b>Slot Math Intelligence</b>
            <span>laboratoire mathématique · v{info?.version ?? "…"}</span>
          </div>
          <nav className="nav">
            {PAGES.map(([k, l], i) => (
              <button key={k} className={page === k ? "on" : ""} onClick={() => go(k)}>
                <span className="k">{String(i + 1).padStart(2, "0")}</span>
                {l}
              </button>
            ))}
          </nav>
          <div className="side-foot">
            {info ? (
              <>
                Moteur Rust smi-core {info.engine}
                <br />
                {info.threads} thread(s) sur {info.cores} cœurs
                <br />
                Base v{info.schema_version}
              </>
            ) : (
              "Chargement…"
            )}
          </div>
        </aside>
        <main className="main">
          <Comp key={page + JSON.stringify(params ?? {})} />
        </main>
      </div>
    </AppCtx.Provider>
  );
}

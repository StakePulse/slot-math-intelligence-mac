import React, { useEffect, useState } from "react";
import { call } from "../api";
import { useApp } from "../App";
import { Card, ErrorBox, Field, Note, NumInput, Page, Status, Tabs } from "../components/ui";

export default function Settings() {
  const { info } = useApp();
  const [s, setS] = useState<any>(null);
  const [threads, setThreads] = useState<number>(0);
  const [cores, setCores] = useState<number>(0);
  const [tab, setTab] = useState("perf");
  const [error, setError] = useState<any>(null);
  const [saved, setSaved] = useState("");
  const [json, setJson] = useState("");
  const [structure, setStructure] = useState<any>(null);
  const load = () => call("settings_get").then((r) => { setS(r.settings); setThreads(r.threads); setCores(r.cores); setJson(JSON.stringify({ volatility: r.settings.volatility, tolerances: r.settings.tolerances, stats: r.settings.stats, design: r.settings.design }, null, 2)); });
  useEffect(() => {
    load();
  }, []);
  if (!s) return <Page title="Settings"><div className="muted">Chargement…</div></Page>;
  const up = (path: string[], v: any) => {
    const n = structuredClone(s);
    let o = n;
    for (const k of path.slice(0, -1)) o = o[k];
    o[path[path.length - 1]] = v;
    setS(n);
  };
  async function save(next = s) {
    try {
      const r = await call("settings_save", { settings: next });
      setS(r.settings);
      setThreads(r.threads);
      setSaved(new Date().toLocaleTimeString("fr-FR"));
    } catch (e) {
      setError(e);
    }
  }
  return (
    <Page title="Settings" intro="Tous les seuils utilisés par un calcul sont ici, modifiables ; les valeurs par défaut sont justifiées dans DECISIONS.md (dossier PROJECT de l'édition complète)." actions={<><span className="faint">{saved && `enregistré à ${saved}`}</span><button className="btn primary" onClick={() => save()}>Enregistrer</button></>}>
      <ErrorBox error={error} />
      <Tabs tabs={[["perf", "Performance"], ["collect", "Collecte"], ["market", "Marché"], ["thresholds", "Seuils et tolérances"], ["project", "Projet"]]} on={tab} set={setTab} />
      {tab === "perf" && (
        <Card title="Modes de performance" sub={`${cores} cœurs logiques`}>
          <div className="grid g3">
            {[["eco", "Économique", "1/4 des cœurs : le PC reste fluide."], ["balanced", "Équilibré", "La moitié des cœurs."], ["performance", "Performance", "Tous les cœurs sauf un."]].map(([k, l, d]) => (
              <button key={k} className={`card ${s.performance_mode === k ? "sel" : ""}`} style={{ textAlign: "left", cursor: "pointer", borderColor: s.performance_mode === k ? "var(--accent)" : undefined }} onClick={() => up(["performance_mode"], k)}>
                <b>{l}</b><div className="muted" style={{ marginTop: 4 }}>{d}</div>
              </button>
            ))}
          </div>
          <div className="grid g3" style={{ marginTop: 12 }}>
            <Field label="Plafond de threads (0 = selon le mode)"><NumInput value={s.max_threads} onChange={(v) => up(["max_threads"], Math.max(0, Math.round(v ?? 0)))} /></Field>
            <Field label="Graine par défaut"><NumInput value={s.default_seed} onChange={(v) => up(["default_seed"], Math.round(v ?? 0))} /></Field>
            <Field label="Limite d'énumération des rouleaux (combinaisons)"><NumInput value={s.enumeration_limit} onChange={(v) => up(["enumeration_limit"], v ?? 0)} /></Field>
          </div>
          <div className="faint" style={{ marginTop: 8 }}>Threads utilisés actuellement : {threads}. Jamais tous les cœurs : un cœur reste libre.</div>
        </Card>
      )}
      {tab === "collect" && (
        <Card title="Collecte StakeCruncher">
          <Note kind="warn">
            <b>À lire avant d'activer.</b> {info?.robots_notice}
          </Note>
          <div className="row" style={{ marginTop: 10 }}>
            <Status s={s.collector.enabled ? "OK" : "NOT_RUN"} />
            <span>{s.collector.enabled ? `Activée le ${s.collector.accepted_at}` : "Désactivée"}</span>
            <span className="spacer" />
            {s.collector.enabled ? (
              <button className="btn danger" onClick={() => call("collector_enable", { enabled: false }).then(load)}>Désactiver</button>
            ) : (
              <button className="btn" onClick={() => { if (confirm("Activer la collecte automatique malgré l'interdiction du robots.txt ? Cette décision est enregistrée avec sa date.")) call("collector_enable", { enabled: true }).then(load); }}>J'ai lu l'avertissement : activer</button>
            )}
          </div>
          <div className="grid g4" style={{ marginTop: 12 }}>
            <Field label="Pause entre requêtes (ms, ≥ 1 000)"><NumInput value={s.collector.delay_ms} onChange={(v) => up(["collector", "delay_ms"], Math.max(1000, Math.round(v ?? 1500)))} /></Field>
            <Field label="Profondeur du classement lu"><NumInput value={s.collector.ranking_depth} onChange={(v) => up(["collector", "ranking_depth"], Math.round(v ?? 400))} /></Field>
            <Field label="Slots dont on lit les tables (top bets 30 j)"><NumInput value={s.collector.top_n} onChange={(v) => up(["collector", "top_n"], Math.round(v ?? 50))} /></Field>
            <Field label="Actualisation automatique (heures, vide = manuelle)"><NumInput value={s.collector.auto_refresh_hours} onChange={(v) => up(["collector", "auto_refresh_hours"], v === null ? null : Math.max(1, Math.round(v)))} /></Field>
          </div>
          <label className="chk" style={{ marginTop: 10 }}><input type="checkbox" checked={s.collector.fetch_luts} onChange={(e) => up(["collector", "fetch_luts"], e.target.checked)} /> télécharger et analyser les lookup tables</label>
          <Field label="User-Agent" style={{ marginTop: 10 }}><input type="text" value={s.collector.user_agent} onChange={(e) => up(["collector", "user_agent"], e.target.value)} /></Field>
        </Card>
      )}
      {tab === "market" && (
        <Card title="Référence du marché">
          <div className="grid g3">
            <Field label="Nombre de slots de référence (30 à 50)"><NumInput value={s.market.reference_size} onChange={(v) => up(["market", "reference_size"], Math.round(v ?? 50))} /></Field>
            <Field label="Échantillon minimal (n)"><NumInput value={s.market.min_sample} onChange={(v) => up(["market", "min_sample"], Math.round(v ?? 10))} /></Field>
            <Field label="Plafonds comparables : rapport maximal"><NumInput value={s.market.peer_factor} onChange={(v) => up(["market", "peer_factor"], v ?? 2)} /></Field>
          </div>
        </Card>
      )}
      {tab === "thresholds" && (
        <Card title="Volatilité, tolérances de validation, catégories, barèmes Stake, contrôles de conception" sub="JSON édité tel quel">
          <Note>
            <b>Volatilité</b> : indicateurs (clé, poids, ancres lo → hi, échelle log), coupures du score entre les 6 classes, plages σ/prix visées par le générateur. <b>Tolérances</b> : écarts admis par la validation. <b>Catégories</b> : seuils petits / moyens / gros / très gros gains (multiples du prix). <b>Barèmes</b> : limites 2★ / 3★ (null = non contrôlé).
          </Note>
          <textarea rows={34} value={json} onChange={(e) => setJson(e.target.value)} style={{ marginTop: 8 }} />
          <button className="btn primary" style={{ marginTop: 8 }} onClick={() => { try { const v = JSON.parse(json); const n = { ...s, ...v }; setS(n); save(n); } catch (e) { setError(`JSON invalide : ${e}`); } }}>Appliquer et enregistrer</button>
        </Card>
      )}
      {tab === "project" && (
        <Card title="Projet et intégrité">
          <table>
            <tbody>
              {Object.entries(info?.paths ?? {}).map(([k, v]: any) => (
                <tr key={k}><td>{k}</td><td className="mono">{v}</td><td><button className="btn small" onClick={() => call("open_path", { path: v })}>Ouvrir</button></td></tr>
              ))}
            </tbody>
          </table>
          <button className="btn" style={{ marginTop: 10 }} onClick={() => call("structure_check").then(setStructure)}>Vérifier la structure</button>
          {structure && (
            <table style={{ marginTop: 10 }}>
              <tbody>
                {structure.files.map((f: any) => <tr key={f.path}><td className="mono">{f.path}</td><td><Status s={f.exists ? "PASS" : "FAIL"} /></td></tr>)}
                {structure.dirs.map((d: any) => <tr key={d[0]}><td className="mono">{d[0]}</td><td>{d[1] ? <Status s="PASS" /> : <span className="badge b-warn">CRÉÉ</span>}</td></tr>)}
                <tr><td>Version du schéma SQLite</td><td>{structure.schema_version}</td></tr>
              </tbody>
            </table>
          )}
        </Card>
      )}
    </Page>
  );
}

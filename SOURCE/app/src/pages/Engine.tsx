import React, { useEffect, useState } from "react";
import { call } from "../api";
import { useApp } from "../App";
import { FullReport, KeyStats, BandsCard } from "../components/statsview";
import { Card, Empty, ErrorBox, Field, JobBar, Nature, Note, NumInput, Page, Status, Tabs, useJob } from "../components/ui";
import { int, ms, num, oneIn, pct, x } from "../fmt";

export function ModelPicker({ value, onChange }: { value: { id: string; version?: number } | null; onChange: (v: { id: string; version?: number } | null) => void }) {
  const [models, setModels] = useState<any[]>([]);
  const [versions, setVersions] = useState<any[]>([]);
  useEffect(() => {
    call("models_list").then(setModels);
  }, []);
  useEffect(() => {
    if (value?.id) call("model_get", { id: value.id, version: null }).then((m) => setVersions(m.versions));
  }, [value?.id]);
  return (
    <div className="row">
      <select value={value?.id ?? ""} onChange={(e) => onChange(e.target.value ? { id: e.target.value } : null)} style={{ minWidth: 260 }}>
        <option value="">— choisir un modèle —</option>
        {models.map((m) => <option key={m.id} value={m.id}>{m.name} (v{m.current_version}, {m.status})</option>)}
      </select>
      {value?.id && (
        <select value={value.version ?? ""} onChange={(e) => onChange({ id: value.id, version: e.target.value ? Number(e.target.value) : undefined })}>
          <option value="">version courante</option>
          {versions.map((v) => <option key={v.version} value={v.version}>v{v.version} — {v.change_note}</option>)}
        </select>
      )}
    </div>
  );
}

export function ValidationView({ r }: { r: any }) {
  if (!r) return null;
  const steps = ["Probabilités", "RTP", "Max win", "Bonus", "Distributions", "Fréquences", "Simulation", "Théorie / simulation", "Rapport final"];
  return (
    <Card title={<>Rapport de validation <Status s={r.status} /></>} sub={`${r.passed} réussis · ${r.warnings} avertissements · ${r.failed} échecs · ${r.not_run} non exécutés · ${r.generated_at}`}>
      {r.status === "NEEDS_REVIEW" && <Note kind="warn">Des contrôles obligatoires n'ont pas été exécutés : le modèle ne peut pas être VALIDATED.</Note>}
      {steps.map((st, i) => (
        <div key={st} style={{ marginTop: 10 }}>
          <div className="muted" style={{ fontSize: 11, textTransform: "uppercase", letterSpacing: "0.05em" }}>{i + 1}. {st}</div>
          <table>
            <tbody>
              {r.checks.filter((c: any) => c.step === i + 1).map((c: any) => (
                <tr key={c.id}>
                  <td style={{ width: 120 }}><Status s={c.status} /></td>
                  <td style={{ width: "34%" }}>{c.id} {c.label}{!c.mandatory && <span className="faint"> (indicatif)</span>}</td>
                  <td className="faint" style={{ width: "18%" }}>attendu {c.expected}</td>
                  <td style={{ width: "22%" }}>{c.observed}</td>
                  <td className="faint"><Nature n={c.nature} /> {c.detail}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ))}
    </Card>
  );
}

export default function Engine() {
  const { model, setModel, go, info } = useApp();
  const [tab, setTab] = useState("exact");
  const [data, setData] = useState<any>(null);
  const [error, setError] = useState<any>(null);
  const [spins, setSpins] = useState<number>(10_000_000);
  const [seed, setSeed] = useState<number>(20261106);
  const job = useJob();
  const load = () => {
    if (!model?.id) return;
    setError(null);
    call("model_get", { id: model.id, version: model.version ?? null }).then(setData).catch(setError);
  };
  useEffect(load, [model?.id, model?.version]);

  async function runValidation() {
    const end = await job.start("validate_start", { id: model!.id, version: model!.version ?? null, spins, seed });
    if (end?.status === "done") load();
  }
  async function exportWhat(what: string) {
    try {
      const r = await call("export_files", { id: model!.id, version: model!.version ?? null, what });
      alert(`Fichiers écrits :\n${r.files.join("\n")}`);
    } catch (e) {
      setError(e);
    }
  }
  const m = data?.model;
  return (
    <Page
      title="Math Engine"
      intro="Analyse exacte d'un modèle (sommes pondérées finies sur sa loi), puis contrôle qualité indépendant en 9 étapes. Aucun statut VALIDATED sans simulation, reproductibilité et comparaison théorie / simulation."
      actions={<ModelPicker value={model} onChange={setModel} />}
    >
      <ErrorBox error={error} />
      <Tabs tabs={[["exact", "Analyse exacte"], ["validation", "Validation"], ["rules", "Moteur de règles (rouleaux)"]]} on={tab} set={setTab} />
      {tab !== "rules" && !model?.id && <Empty>Choisissez un modèle (ou créez-en un dans le Math Generator).</Empty>}
      {tab !== "rules" && m && (
        <Card title={`${m.name} — v${m.version}`} sub={`créé le ${m.created_at} · empreinte ${m.fingerprint}`} right={<Status s={m.status} />}>
          <div className="row">
            <span className="muted">{m.spec.game_type} · {m.spec.mechanics.join(", ")} · max {x(m.spec.max_win)} · RTP cible {pct(m.spec.rtp, 2)} · {m.spec.volatility}</span>
            <span className="spacer" />
            <button className="btn small" onClick={() => go("generator", { spec: m.spec, modelId: m.model_id })}>Modifier (nouvelle version)</button>
            <button className="btn small" onClick={() => go("simulator", { target: { kind: "model", id: m.model_id, version: m.version }, label: `${m.name} v${m.version}` })}>Simuler</button>
            <button className="btn small" onClick={() => exportWhat("model_json")}>Exporter loi (JSON + lookup table)</button>
            <button className="btn small" onClick={() => go("prompt", { model: { id: m.model_id, version: m.version } })}>Prompt</button>
          </div>
          {m.notes?.length > 0 && <div className="note warn" style={{ marginTop: 8 }}>{m.notes.join(" · ")}</div>}
        </Card>
      )}
      {tab === "exact" && data && <FullReport s={data.stats} vol={data.vol} base={data.base} bonus={data.bonus} />}
      {tab === "validation" && data && (
        <>
          <Card title="Lancer la validation complète">
            <div className="row">
              <Field label="Spins (≥ minimum des réglages)"><NumInput value={spins} onChange={(v) => v && setSpins(v)} /></Field>
              <Field label="Graine"><NumInput value={seed} onChange={(v) => v !== null && setSeed(v)} /></Field>
              <button className="btn primary" style={{ alignSelf: "flex-end" }} disabled={job.job?.status === "running"} onClick={runValidation}>Valider</button>
              {data.validation && <button className="btn" style={{ alignSelf: "flex-end" }} onClick={() => exportWhat("validation")}>Exporter le rapport (MD + JSON)</button>}
              {data.validation?.status === "VALIDATED" && <button className="btn" style={{ alignSelf: "flex-end" }} onClick={() => call("model_set_status", { id: model!.id, version: m.version, status: "READY_FOR_EXPORT" }).then(load).catch(setError)}>Marquer READY FOR EXPORT</button>}
            </div>
            <div className="faint" style={{ marginTop: 8 }}>Étapes : probabilités · RTP · max win · bonus · distributions · fréquences · simulation ({info?.threads} threads) · théorie / simulation · rapport. Une simulation existante de la même empreinte et d'au moins autant de spins est réutilisée.</div>
          </Card>
          <JobBar j={job} />
          {data.validation ? <ValidationView r={data.validation} /> : <Empty>Aucune validation pour cette version : statut DRAFT.</Empty>}
          {data.sim && (
            <Card title="Simulation de référence" right={<Nature n="MONTE_CARLO" />}>
              <div className="faint">{int(data.sim.spins_done)} spins · graine {data.sim.config.seed} · {data.sim.config.threads} threads · {ms(data.sim.elapsed_ms)} · empreinte des compteurs {data.sim.counts_hash}</div>
              <table style={{ marginTop: 8 }}>
                <tbody>
                  <tr><td>RTP simulé</td><td className="n">{pct(data.sim.comparison.rtp_sim, 4)}</td><td className="faint">exact {pct(data.sim.comparison.rtp_theory, 4)} · z = {num(data.sim.comparison.rtp_z, 2)}</td></tr>
                  <tr><td>Hit rate simulé</td><td className="n">{pct(data.sim.comparison.hit_sim, 3)}</td><td className="faint">exact {pct(data.sim.comparison.hit_theory, 3)} · z = {num(data.sim.comparison.hit_z, 2)}</td></tr>
                  <tr><td>Max win</td><td className="n">{data.sim.comparison.max_win.observed} vu(s)</td><td className="faint">{num(data.sim.comparison.max_win.expected, 2)} attendu(s) · {data.sim.comparison.max_win.note}</td></tr>
                </tbody>
              </table>
            </Card>
          )}
        </>
      )}
      {tab === "rules" && <RulesEngine />}
    </Page>
  );
}

function RulesEngine() {
  const { go } = useApp();
  const [txt, setTxt] = useState("");
  const [exact, setExact] = useState<any>(null);
  const [error, setError] = useState<any>(null);
  useEffect(() => {
    call("reels_demo").then((g) => setTxt(JSON.stringify(g, null, 1)));
  }, []);
  function parse() {
    try {
      return JSON.parse(txt);
    } catch (e) {
      setError(`JSON invalide : ${e}`);
      return null;
    }
  }
  async function computeExact() {
    setError(null);
    const g = parse();
    if (!g) return;
    try {
      setExact(await call("reels_exact", { game: g }));
    } catch (e) {
      setError(e);
    }
  }
  return (
    <div className="grid g2">
      <Card title="Règles du jeu (JSON)" right={<Nature n="CONFIGURATION" />}>
        <Note>Rouleaux à bandes, ways ou lignes, wild, scatter, tours gratuits (relances, multiplicateur), plafond. Exemple : jeu de démonstration 243 ways, non réglé. Gains en multiples de la mise totale.</Note>
        <textarea rows={26} value={txt} onChange={(e) => setTxt(e.target.value)} style={{ marginTop: 8 }} />
        <div className="row" style={{ marginTop: 8 }}>
          <button className="btn primary" onClick={computeExact}>Calcul exact du jeu de base</button>
          <button className="btn" onClick={() => { const g = parse(); if (g) go("simulator", { target: { kind: "reels", game: g, buy: null }, label: g.name }); }}>Simuler des rounds complets</button>
        </div>
        <ErrorBox error={error} />
      </Card>
      <div className="stack">
        {exact && (
          <Card title="Jeu de base" right={<Nature n="EXACT" />}>
            <table>
              <tbody>
                <tr><td>RTP des lignes / ways (formule fermée)</td><td className="n">{pct(exact.line_or_ways_rtp, 5)}</td></tr>
                <tr><td>RTP des scatters</td><td className="n">{pct(exact.scatter_rtp, 5)}</td></tr>
                <tr><td>Probabilité de déclencher les tours gratuits</td><td className="n">{pct(exact.trigger_prob, 5)} · {oneIn(1 / exact.trigger_prob)}</td></tr>
                <tr><td>Combinaisons d'arrêts</td><td className="n">{int(exact.combinations)}</td></tr>
              </tbody>
            </table>
            <h2 style={{ marginTop: 12 }}>Loi du nombre de scatters visibles</h2>
            <table>
              <thead><tr>{exact.scatter_dist.map((_: any, k: number) => <th key={k} className="n">{k}</th>)}</tr></thead>
              <tbody><tr>{exact.scatter_dist.map((p: number, k: number) => <td key={k} className="n">{p > 0 ? oneIn(1 / p) : "—"}</td>)}</tr></tbody>
            </table>
            {exact.enumerated ? (
              <Note kind="ok">Énumération exhaustive faite : RTP {pct(exact.enumerated.rtp, 6)} · hit {pct(exact.enumerated.hit_rate, 4)} · déclenchement {pct(exact.enumerated.trigger_prob, 5)} — loi complète du spin de base, {int(exact.enumerated.law.length)} gains distincts.</Note>
            ) : (
              <Note>Énumération non faite : {int(exact.combinations)} combinaisons dépassent la limite des réglages. Les espérances ci-dessus restent exactes (formules fermées) ; la loi complète passe par la simulation.</Note>
            )}
          </Card>
        )}
        <Card title="Méthode">
          <div className="muted" style={{ lineHeight: 1.6 }}>
            Ways : E = Σ<sub>s</sub> Σ<sub>L</sub> pay<sub>s</sub>(L) · (Π<sub>r&lt;L</sub> E[a<sub>r</sub>] − Π<sub>r&lt;L</sub> E[w<sub>r</sub>]) · P(a<sub>L</sub> = 0), a<sub>r</sub> = cases du rouleau r portant s ou wild, w<sub>r</sub> = wilds (une way faite que de wilds ne paie pas).<br />
            Lignes : même formule avec les probabilités par case de chaque ligne. Scatters : convolution exacte des rouleaux.<br />
            Les tests du moteur vérifient l'égalité formule fermée = énumération exhaustive au 1e-12.
          </div>
        </Card>
      </div>
    </div>
  );
}

export { KeyStats, BandsCard };

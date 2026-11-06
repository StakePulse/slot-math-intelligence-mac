import React, { useEffect, useState } from "react";
import { call, SPIN_PRESETS } from "../api";
import { useApp } from "../App";
import { CompareBars, ConvergenceChart, SurvivalChart } from "../components/charts";
import { FullReport } from "../components/statsview";
import { Card, Empty, ErrorBox, Field, JobBar, Nature, Note, NumInput, Page, Status, Tabs, useJob } from "../components/ui";
import { int, ms, num, oneIn, oneInP, pct, sci, x } from "../fmt";
import { ModelPicker } from "./Engine";

function Target({ target, setTarget, label }: { target: any; setTarget: (t: any, l: string) => void; label: string }) {
  const [modes, setModes] = useState<any[]>([]);
  useEffect(() => {
    call("market_query", { filter: { providers: [], kinds: [], vol_classes: [], include_ours: true } }).then((d) => setModes(d.rows.filter((r: any) => r.has_lut)));
  }, []);
  return (
    <div className="stack">
      <Field label="Modèle de la bibliothèque">
        <ModelPicker value={target?.kind === "model" ? { id: target.id, version: target.version } : null} onChange={(v) => v && setTarget({ kind: "model", id: v.id, version: v.version ?? null }, "modèle")} />
      </Field>
      <Field label="… ou mode du marché avec lookup table">
        <select value={target?.kind === "mode" ? target.mode_id : ""} onChange={(e) => { const m = modes.find((r) => r.mode_id === Number(e.target.value)); if (m) setTarget({ kind: "mode", mode_id: m.mode_id }, `${m.name} — ${m.mode}`); }}>
          <option value="">—</option>
          {modes.map((m) => <option key={m.mode_id} value={m.mode_id}>{m.name} — {m.mode} (×{num(m.cost, 0)})</option>)}
        </select>
      </Field>
      {target?.kind === "reels" && <Note>Cible : règles « {target.game.name} » (moteur de rouleaux) — rounds complets avec tours gratuits.</Note>}
      <div className="faint">Cible actuelle : <b>{label || "aucune"}</b></div>
    </div>
  );
}

export function SimResultView({ data }: { data: any }) {
  if (!data) return null;
  if (data.kind === "reels") {
    const r = data.result;
    const e = r.exact_base;
    const n = r.reel.rounds;
    const z = e.trigger_prob > 0 && !r.reel.buy ? (r.reel.trigger_rate - e.trigger_prob) / Math.sqrt((e.trigger_prob * (1 - e.trigger_prob)) / n) : null;
    return (
      <div className="stack">
        <Card title="Rounds complets simulés" right={<Nature n="MONTE_CARLO" />}>
          <table>
            <tbody>
              <tr><td>Rounds</td><td className="n">{int(n)}</td><td className="faint">graine {r.reel.seed} · {ms(r.reel.elapsed_ms)}</td></tr>
              <tr><td>Déclenchement des tours gratuits</td><td className="n">{pct(r.reel.trigger_rate, 4)}</td><td className="faint">exact {pct(e.trigger_prob, 4)}{z !== null && ` · z = ${num(z, 2)}`}</td></tr>
              <tr><td>Tours gratuits joués</td><td className="n">{int(r.reel.free_spins_played)}</td><td className="faint">{num(r.reel.free_spins_played / Math.max(1, r.reel.bonus_rounds), 2)} par bonus</td></tr>
              <tr><td>RTP du jeu de base (exact, hors tours gratuits)</td><td className="n">{pct(e.line_or_ways_rtp + e.scatter_rtp, 4)}</td><td className="faint">formule fermée</td></tr>
            </tbody>
          </table>
        </Card>
        <FullReport s={r.stats} nature="MONTE_CARLO" />
      </div>
    );
  }
  const r = data.result;
  const c = r.comparison;
  const th = data.theory;
  const N = r.spins_done;
  const cats = r.empirical.categories;
  return (
    <div className="stack">
      {!r.completed && <Note kind="warn">Simulation interrompue à {int(N)} spins : résultats partiels, non reproductibles à l'identique.</Note>}
      {r.anomalies.length > 0 && <Note kind="bad"><b>Anomalies :</b> {r.anomalies.join(" · ")}</Note>}
      <Card title="Théorie et simulation" sub={`${int(N)} spins · graine ${r.config.seed} · ${r.config.threads} threads · ${ms(r.elapsed_ms)} · ${int(r.spins_per_sec)} spins/s`} right={<><Nature n="EXACT" /> <Nature n="MONTE_CARLO" /></>}>
        <table>
          <thead><tr><th>Mesure</th><th className="n">Exact</th><th className="n">Simulé</th><th className="n">Écart</th><th>Lecture</th></tr></thead>
          <tbody>
            <tr><td>RTP</td><td className="n">{pct(c.rtp_theory, 4)}</td><td className="n">{pct(c.rtp_sim, 4)}</td><td className="n">z = {num(c.rtp_z, 2)}</td><td className="faint">IC 95 % {pct(c.rtp_ci95[0], 3)} – {pct(c.rtp_ci95[1], 3)} (σ exact) · p = {num(c.rtp_p_value, 3)}</td></tr>
            <tr><td>Hit rate</td><td className="n">{pct(c.hit_theory, 3)}</td><td className="n">{pct(c.hit_sim, 3)}</td><td className="n">z = {num(c.hit_z, 2)}</td><td className="faint">Wilson {pct(c.hit_ci95[0], 3)} – {pct(c.hit_ci95[1], 3)}</td></tr>
            <tr><td>Spins perdants</td><td className="n">{pct(th.zero_rate, 3)}</td><td className="n">{int(N * r.empirical.zero_rate)}</td><td /><td /></tr>
            {c.bonus_freq && <tr><td>Fréquence des bonus</td><td className="n">{oneInP(c.bonus_freq[0])}</td><td className="n">{oneInP(c.bonus_freq[1])}</td><td className="n">z = {num(c.bonus_freq[2], 2)}</td><td /></tr>}
            {c.bonus_mean && <tr><td>Gain moyen des bonus</td><td className="n">{x(c.bonus_mean[0])}</td><td className="n">{x(c.bonus_mean[1])}</td><td className="n">z = {num(c.bonus_mean[2], 2)}</td><td /></tr>}
            {c.bonus_median && <tr><td>Médiane des bonus</td><td className="n">{x(c.bonus_median[0])}</td><td className="n">{x(c.bonus_median[1])}</td><td /><td /></tr>}
            <tr><td>Plus gros gain</td><td className="n">{x(th.max_win.value)} (max)</td><td className="n">{x(r.biggest_win)}</td><td /><td /></tr>
            {c.chi2 && <tr><td>Bandes de gain</td><td /><td className="n">χ² = {num(c.chi2.stat, 1)}</td><td className="n">ddl {c.chi2.df}</td><td className="faint">p = {num(c.chi2.p_value, 4)} (bandes fusionnées jusqu'à 5 attendus)</td></tr>}
          </tbody>
        </table>
      </Card>
      <div className="grid g2">
        <Card title="Nombre de spins par catégorie" right={<Nature n="MONTE_CARLO" />}>
          <table>
            <thead><tr><th>Catégorie</th><th className="n">Observés</th><th className="n">Attendus</th></tr></thead>
            <tbody>
              {cats.map((k: any, i: number) => (
                <tr key={k.key}><td>{k.label}</td><td className="n">{int(k.prob * N)}</td><td className="n">{int(th.categories[i].prob * N)}</td></tr>
              ))}
            </tbody>
          </table>
        </Card>
        <Card title="Convergence du RTP" sub="RTP cumulé et IC 95 % autour du RTP exact" right={<Nature n="MONTE_CARLO" />}>
          <ConvergenceChart points={r.convergence} theory={c.rtp_theory} se1={th.sd_per_price} />
        </Card>
      </div>
      <Card title="Seuils de gain : observés et attendus" sub="les événements rares sont jugés par leur probabilité exacte">
        <div className="tablewrap">
          <table>
            <thead><tr><th>Gain ≥</th><th className="n">Probabilité exacte</th><th className="n">Attendus (N·p)</th><th className="n">Observés</th><th className="n">p-valeur</th><th className="n">P(aucun en N)</th><th>Note</th></tr></thead>
            <tbody>
              {[...c.thresholds, { ...c.max_win, mult_price: null }].map((t: any, i: number) => (
                <tr key={i}>
                  <td>{t.mult_price === null ? <b>Max win</b> : `${x(t.mult_price)} le prix`}</td><td className="n">{sci(t.prob)}</td><td className="n">{num(t.expected, 2)}</td><td className="n">{int(t.observed)}</td>
                  <td className="n">{t.p_value < 1e-5 ? <span className="badge b-bad">{sci(t.p_value)}</span> : num(t.p_value, 3)}</td><td className="n">{pct(t.p_none, 2)}</td><td className="faint">{t.note}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Card>
      <FullReport s={r.empirical} nature="MONTE_CARLO" compare={undefined} />
    </div>
  );
}

export default function Simulator() {
  const { params, info } = useApp();
  const [tab, setTab] = useState("run");
  const [target, setTargetS] = useState<any>(params?.target ?? null);
  const [label, setLabel] = useState<string>(params?.label ?? "");
  const [spins, setSpins] = useState(10_000_000);
  const [seed, setSeed] = useState(20261106);
  const [threads, setThreads] = useState<number | null>(null);
  const [result, setResult] = useState<any>(null);
  const job = useJob();
  const setTarget = (t: any, l: string) => { setTargetS(t); setLabel(l); };

  async function start() {
    setResult(null);
    const end = await job.start("sim_start", { target, spins, seed, threads });
    if (end?.result) setResult(end.result);
  }
  return (
    <Page title="Spin Simulator" intro={<>Tirages au hasard comme le RGS : chaque round tire une issue avec une probabilité proportionnelle à son poids. Graine fixe, lots de 1 048 576 spins à flux indépendants : <b>même résultat quel que soit le nombre de threads</b>. Les gains ne sont jamais « fabriqués » pour atteindre le RTP.</>}>
      <Tabs tabs={[["run", "Simulation"], ["sessions", "Sessions et bankroll"], ["compare", "Comparer des simulations"]]} on={tab} set={setTab} />
      {tab === "run" && (
        <>
          <div className="grid g2">
            <Card title="Cible"><Target target={target} setTarget={setTarget} label={label} /></Card>
            <Card title="Paramètres">
              <div className="stack">
                <Field label="Nombre de spins">
                  <div className="chips">
                    {SPIN_PRESETS.map((n) => <button key={n} className={`chip ${spins === n ? "on" : ""}`} onClick={() => setSpins(n)}>{n >= 1e6 ? `${n / 1e6} M` : `${n / 1e3} k`}</button>)}
                  </div>
                  <NumInput value={spins} onChange={(v) => v && setSpins(Math.min(4e9, Math.round(v)))} />
                </Field>
                <div className="grid g2">
                  <Field label="Graine"><NumInput value={seed} onChange={(v) => v !== null && setSeed(Math.round(v))} /></Field>
                  <Field label={`Threads (vide = mode de performance : ${info?.threads ?? "?"})`}><NumInput value={threads} onChange={setThreads} placeholder="auto" /></Field>
                </div>
                <button className="btn primary" disabled={!target || job.job?.status === "running" || job.job?.status === "paused"} onClick={start}>Lancer</button>
                <div className="faint">Mémoire : un compteur 32 bits par issue et par thread, rien par spin. Pause / reprise / annulation à tout moment.</div>
              </div>
            </Card>
          </div>
          <JobBar j={job} />
          {result ? <SimResultView data={result} /> : !job.job && <Empty>Choisissez une cible et lancez une simulation.</Empty>}
        </>
      )}
      {tab === "sessions" && <Sessions target={target} setTarget={setTarget} label={label} />}
      {tab === "compare" && <Compare />}
    </Page>
  );
}

function Sessions({ target, setTarget, label }: any) {
  const [rounds, setRounds] = useState(200);
  const [sessions, setSessions] = useState(100_000);
  const [bankroll, setBankroll] = useState<number | null>(100);
  const [big, setBig] = useState(100);
  const [seed, setSeed] = useState(20261106);
  const [r, setR] = useState<any>(null);
  const job = useJob();
  return (
    <>
      <div className="grid g2">
        <Card title="Cible"><Target target={target} setTarget={setTarget} label={label} /></Card>
        <Card title="Sessions" right={<Nature n="MONTE_CARLO" />}>
          <div className="grid g2">
            <Field label="Rounds par session"><NumInput value={rounds} onChange={(v) => v && setRounds(Math.round(v))} /></Field>
            <Field label="Nombre de sessions"><NumInput value={sessions} onChange={(v) => v && setSessions(Math.round(v))} /></Field>
            <Field label="Bankroll de départ (× prix, vide = sans)"><NumInput value={bankroll} onChange={setBankroll} /></Field>
            <Field label="« Gros gain » (× prix)"><NumInput value={big} onChange={(v) => v && setBig(v)} /></Field>
            <Field label="Graine"><NumInput value={seed} onChange={(v) => v !== null && setSeed(Math.round(v))} /></Field>
          </div>
          <button className="btn primary" style={{ marginTop: 10 }} disabled={!target || target.kind === "reels"} onClick={async () => { const e = await job.start("sessions_start", { target, rounds, sessions, seed, bankroll, bigWin: big }); if (e?.result) setR(e.result); }}>Lancer</button>
        </Card>
      </div>
      <JobBar j={job} />
      {r && (
        <div className="grid g2">
          <Card title="Retour d'une session" sub={`R = gains / mises · ${int(r.config.sessions)} sessions`}>
            <table>
              <tbody>
                <tr><td>Retour moyen</td><td className="n">{pct(r.mean_return, 2)}</td><td className="faint">IC 95 % {pct(r.mean_ci95[0], 2)} – {pct(r.mean_ci95[1], 2)}</td></tr>
                {r.percentiles.map((p: any) => <tr key={p[0]}><td>P{num(p[0] * 100, 0)}</td><td className="n">{pct(p[1], 1)}</td><td className="faint">IC {pct(p[2], 1)} – {pct(p[3], 1)}</td></tr>)}
                <tr><td>Sessions gagnantes (R ≥ 100 %)</td><td className="n">{pct(r.p_profit, 2)}</td><td className="faint">Wilson {pct(r.p_profit_ci95[0], 2)} – {pct(r.p_profit_ci95[1], 2)}</td></tr>
                <tr><td>Sessions sous 50 %</td><td className="n">{pct(r.p_below_half, 2)}</td><td /></tr>
                <tr><td>Au moins un gain ≥ {x(r.config.big_win)} le prix</td><td className="n">{pct(r.big_win_sim, 2)}</td><td className="faint">exact (N fixe) {pct(r.big_win_exact, 2)}{r.config.bankroll ? " — non comparable avec une bankroll" : ` · z = ${num(r.big_win_z, 2)}`}</td></tr>
              </tbody>
            </table>
          </Card>
          {r.survival.length > 0 && <Card title="Survie de la bankroll" sub={`départ ${x(r.config.bankroll)} le prix`}><SurvivalChart points={r.survival} /></Card>}
        </div>
      )}
    </>
  );
}

function Compare() {
  const [list, setList] = useState<any[]>([]);
  const [sel, setSel] = useState<number[]>([]);
  const [cmp, setCmp] = useState<any>(null);
  const [error, setError] = useState<any>(null);
  useEffect(() => {
    call("simulations_list", { modelId: null }).then(setList);
  }, []);
  return (
    <>
      <Card title="Historique des simulations" sub="cocher au moins deux simulations">
        <div className="tablewrap">
          <table>
            <thead><tr><th /><th>Date</th><th>Cible</th><th>Type</th><th className="n">Spins</th><th className="n">RTP simulé</th><th>État</th><th className="n">Durée</th></tr></thead>
            <tbody>
              {list.map((s) => (
                <tr key={s.id}>
                  <td><input type="checkbox" checked={sel.includes(s.id)} onChange={(e) => setSel(e.target.checked ? [...sel, s.id] : sel.filter((v) => v !== s.id))} /></td>
                  <td className="faint">{s.created_at}</td><td>{s.label}</td><td>{s.target_kind}</td><td className="n">{int(s.spins)}</td><td className="n">{pct(s.rtp, 4)}</td><td><Status s={s.completed ? "OK" : "PARTIEL"} /></td><td className="n">{ms(s.elapsed_ms)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <button className="btn primary" style={{ marginTop: 8 }} disabled={sel.length < 2} onClick={() => call("simulations_compare", { ids: sel }).then(setCmp).catch(setError)}>Comparer</button>
      </Card>
      <ErrorBox error={error} />
      {cmp && (
        <Card title="Comparaison" sub={cmp.method}>
          <div className="tablewrap">
            <table>
              <thead><tr><th>Simulation</th><th className="n">Spins</th><th className="n">RTP</th><th className="n">Hit</th><th className="n">σ/prix</th><th className="n">Bonus</th><th className="n">Moy. bonus</th><th className="n">Méd. bonus</th><th className="n">Plus gros gain</th><th>Objectifs</th><th className="n">Distance</th></tr></thead>
              <tbody>
                {cmp.rows.map((r: any) => (
                  <tr key={r.id} className={cmp.closest === r.id ? "sel" : ""}>
                    <td>{r.label} {cmp.closest === r.id && <span className="badge b-ok">LA PLUS PROCHE</span>}</td><td className="n">{int(r.spins)}</td><td className="n">{pct(r.rtp, 3)}</td><td className="n">{pct(r.hit_rate, 2)}</td><td className="n">{num(r.sd_per_price, 2)}</td>
                    <td className="n">{oneIn(r.bonus_one_in)}</td><td className="n">{x(r.bonus_mean)}</td><td className="n">{x(r.bonus_median)}</td><td className="n">{x(r.max_observed)}</td>
                    <td className="faint">{r.targets ? `RTP ${pct(r.targets.rtp, 2)} · hit ${pct(r.targets.hit_rate, 1)} · bonus 1/${int(r.targets.bonus_one_in)} · ${r.targets.volatility}` : "—"}</td><td className="n">{r.distance === null ? "—" : num(r.distance, 4)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <div style={{ marginTop: 12 }}>
            <CompareBars rows={cmp.rows.map((r: any) => ({ ...r, label: `#${r.id}` }))} metrics={[{ key: "rtp", label: "RTP simulé", fmt: (v) => pct(v, 2) }, { key: "hit_rate", label: "Hit rate", fmt: (v) => pct(v, 1) }, { key: "sd_per_price", label: "σ / prix", fmt: (v) => num(v, 1) }]} />
          </div>
        </Card>
      )}
    </>
  );
}

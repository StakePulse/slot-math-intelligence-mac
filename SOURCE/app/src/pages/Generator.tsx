import React, { useEffect, useRef, useState } from "react";
import { call, MAX_PRESETS, MECHANICS, VOL_CLASSES } from "../api";
import { useApp } from "../App";
import { FullReport } from "../components/statsview";
import { Card, Chips, ErrorBox, Field, Nature, Note, NumInput, Page, Tabs } from "../components/ui";
import { int, num, oneIn, oneInP, pct, x } from "../fmt";
import { AggregatesTable, fmtMetric } from "./Market";

const DEFAULT = {
  name: "Nouveau jeu",
  game_type: "Video slot 5×3",
  mechanics: ["Ways", "Free Spins", "Scatter Features"],
  price: 1,
  rtp: 0.965,
  max_win: 5000,
  max_win_one_in: null as number | null,
  max_rtp_share: 0.015,
  hit_rate: 0.25,
  bonus_one_in: 200,
  bonus_share: 0.45,
  volatility: "High",
  sd_target: null as number | null,
  base_max_win: null as number | null,
  bonus_floor: null as number | null,
  granularity: 0.1,
};

export default function Generator() {
  const { params, go, model } = useApp();
  const [spec, setSpec] = useState<any>(() => {
    if (params?.spec) return { ...DEFAULT, ...params.spec };
    if (params?.fromMode) return { ...DEFAULT, max_win: Math.round((params.fromMode.max_mult ?? 5000) * (params.fromMode.cost ?? 1)), name: `Inspiré de ${params.fromMode.name}` };
    return DEFAULT;
  });
  const [preview, setPreview] = useState<any>(null);
  const [peers, setPeers] = useState<any>(null);
  const [error, setError] = useState<any>(null);
  const [busy, setBusy] = useState(false);
  const [tab, setTab] = useState("result");
  const [note, setNote] = useState("");
  const [customMax, setCustomMax] = useState(!MAX_PRESETS.includes(spec.max_win));
  const timer = useRef<any>(null);
  const set = (k: string, v: any) => setSpec((s: any) => ({ ...s, [k]: v }));

  useEffect(() => {
    clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      setBusy(true);
      setError(null);
      call("generate_preview", { spec })
        .then(setPreview)
        .catch((e) => { setPreview(null); setError(e); })
        .finally(() => setBusy(false));
    }, 350);
  }, [JSON.stringify(spec)]);
  useEffect(() => {
    call("generator_peers", { maxWin: spec.max_win, price: spec.price }).then(setPeers).catch(() => setPeers(null));
  }, [spec.max_win, spec.price]);

  const med = (k: string) => peers?.aggregates?.find((a: any) => a.key === k)?.agg?.median ?? null;
  async function save(asVersion: boolean) {
    try {
      const r = await call("model_save", { spec, modelId: asVersion ? params?.modelId ?? model?.id ?? null : null, note });
      go("engine", { model: { id: r.id, version: r.version } });
    } catch (e) {
      setError(e);
    }
  }
  const s = preview?.stats;
  const p = preview?.params;

  return (
    <Page
      title="Math Generator"
      intro={<>Définissez les objectifs du jeu ; le moteur construit une loi complète qui les atteint EXACTEMENT (RTP, hit rate, fréquence et part des bonus, fréquence du max win) et cherche la forme qui vise la volatilité demandée. Les références du marché servent de <b>comparaison</b>, jamais de modèle à recopier.</>}
      actions={<>
        <input type="text" placeholder="note de version" value={note} onChange={(e) => setNote(e.target.value)} style={{ width: 200 }} />
        {(params?.modelId || model?.id) && <button className="btn" disabled={!preview} onClick={() => save(true)}>Nouvelle version du modèle ouvert</button>}
        <button className="btn primary" disabled={!preview} onClick={() => save(false)}>Enregistrer comme nouveau modèle</button>
      </>}
    >
      <div className="grid" style={{ gridTemplateColumns: "400px 1fr" }}>
        <div className="stack">
          <Card title="Objectifs" right={<Nature n="SPECIFICATION" />}>
            <div className="stack">
              <Field label="Nom du projet"><input type="text" value={spec.name} onChange={(e) => set("name", e.target.value)} /></Field>
              <Field label="Type de slot"><input type="text" value={spec.game_type} onChange={(e) => set("game_type", e.target.value)} /></Field>
              <Field label="Mécaniques (combinables)"><Chips options={MECHANICS} value={spec.mechanics} onChange={(v) => set("mechanics", v)} /></Field>
              <Field label="Multiplicateur maximum (× mise de base)">
                <div className="chips">
                  {MAX_PRESETS.map((m) => <button key={m} className={`chip ${!customMax && spec.max_win === m ? "on" : ""}`} onClick={() => { setCustomMax(false); set("max_win", m); }}>×{int(m)}</button>)}
                  <button className={`chip ${customMax ? "on" : ""}`} onClick={() => setCustomMax(true)}>Personnalisé</button>
                </div>
                {customMax && <NumInput value={spec.max_win} onChange={(v) => v && set("max_win", v)} />}
              </Field>
              <div className="grid g2">
                <Field label="RTP cible (%)"><NumInput value={Math.round(spec.rtp * 1e6) / 1e4} onChange={(v) => v && set("rtp", v / 100)} /></Field>
                <Field label="Prix du mode (× mise)"><NumInput value={spec.price} onChange={(v) => v && set("price", v)} /></Field>
                <Field label="Hit rate (%)"><NumInput value={Math.round(spec.hit_rate * 1e6) / 1e4} onChange={(v) => v && set("hit_rate", v / 100)} /></Field>
                <Field label="Bonus : 1 sur N"><NumInput value={spec.bonus_one_in} onChange={(v) => v && set("bonus_one_in", v)} /></Field>
                <Field label="Part du RTP des bonus (%)"><NumInput value={Math.round(spec.bonus_share * 1e4) / 100} onChange={(v) => v !== null && set("bonus_share", v / 100)} /></Field>
                <Field label="Max win : 1 sur N (vide = auto)"><NumInput value={spec.max_win_one_in} onChange={(v) => set("max_win_one_in", v)} placeholder={`auto (${num(spec.max_rtp_share * 100, 2)} % du RTP)`} /></Field>
              </div>
              <Field label="Volatilité souhaitée"><Chips multi={false} options={VOL_CLASSES} value={[spec.volatility]} onChange={(v) => v[0] && set("volatility", v[0])} /></Field>
              <details>
                <summary className="muted" style={{ cursor: "pointer" }}>Réglages avancés</summary>
                <div className="grid g2" style={{ marginTop: 8 }}>
                  <Field label="σ / prix visé (vide = classe)"><NumInput value={spec.sd_target} onChange={(v) => set("sd_target", v)} /></Field>
                  <Field label="Part du RTP au max (auto)"><NumInput value={Math.round(spec.max_rtp_share * 1e4) / 100} onChange={(v) => v !== null && set("max_rtp_share", v / 100)} /></Field>
                  <Field label="Gain max du jeu de base"><NumInput value={spec.base_max_win} onChange={(v) => set("base_max_win", v)} placeholder="min(max, 1 000× prix)" /></Field>
                  <Field label="Plancher du bonus (vide = cherché)"><NumInput value={spec.bonus_floor} onChange={(v) => set("bonus_floor", v)} /></Field>
                  <Field label="Pas des gains (×)"><NumInput value={spec.granularity} onChange={(v) => v && set("granularity", v)} /></Field>
                </div>
              </details>
            </div>
          </Card>

          <Card title="Références du marché" sub={peers ? `${peers.peers.length} jeux comparables` : "aucune"}>
            {peers && peers.peers.length > 0 ? (
              <div className="stack">
                <div className="faint">Modes de même nature, plafond entre ×{int(spec.max_win / spec.price / 2)} et ×{int((spec.max_win / spec.price) * 2)} ; classement par {peers.basis}.</div>
                <table>
                  <tbody>
                    {[["hit", "Hit rate médian", (v: number) => pct(v, 1), () => set("hit_rate", med("hit"))],
                      ["sd_per_price", "σ / prix médian", (v: number) => num(v, 1), () => set("sd_target", Math.round(med("sd_per_price") * 10) / 10)],
                      ["p_max", "Max win médian", (v: number) => oneInP(v), () => set("max_win_one_in", Math.round(1 / med("p_max")))],
                      ["share_100x", "Part du RTP ≥ 100×", (v: number) => pct(v, 0), null]].map(([k, l, f, apply]: any) => (
                      <tr key={k}>
                        <td>{l}</td><td className="n">{med(k) === null ? "n.d." : f(med(k))}</td>
                        <td>{apply && med(k) !== null && <button className="btn small" onClick={apply}>Appliquer</button>}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                <Note>Appliquer une médiane est un choix de conception : la source et la date restent visibles dans le prompt.</Note>
              </div>
            ) : (
              <div className="muted">Aucun mode comparable dans la base (importez des données de marché).</div>
            )}
          </Card>
        </div>

        <div className="stack">
          <ErrorBox error={error} />
          {busy && <div className="faint">Calcul…</div>}
          {preview && (
            <>
              {preview.notes.length > 0 && <Note kind="warn">{preview.notes.map((n: string, i: number) => <div key={i}>{n}</div>)}</Note>}
              <Card title="Construction" sub="paramètres résolus par le moteur" right={<Nature n="CONFIGURATION" />}>
                <div className="grid g4">
                  <div className="stat"><div className="l">P(pas de gain)</div><div className="v">{pct(p.p_zero, 2)}</div></div>
                  <div className="stat"><div className="l">P(gain de base)</div><div className="v">{pct(p.p_base, 3)}</div><div className="s">moyenne {x(p.mean_base)} · exposant {num(p.alpha_base, 3)}</div></div>
                  <div className="stat"><div className="l">P(bonus hors max)</div><div className="v">{pct(p.p_bonus_body, 4)}</div><div className="s">moyenne {x(p.mean_bonus_body)} · plancher {x(p.bonus_floor)}</div></div>
                  <div className="stat"><div className="l">P(max win)</div><div className="v">{oneInP(p.p_max)}</div><div className="s">{int(preview.outcomes)} issues · gain max de base {x(p.base_max)}</div></div>
                </div>
                <div className="faint" style={{ marginTop: 8 }}>
                  σ/prix visé {p.sd_target ? num(p.sd_target, 2) : "—"} · obtenu {num(p.sd_achieved, 2)} · atteignable avec ces objectifs : {num(p.sd_feasible[0], 2)} – {num(p.sd_feasible[1], 2)}
                </div>
              </Card>
              <Tabs tabs={[["result", "Loi générée (exacte)"], ["market", "Comparaison au marché"]]} on={tab} set={setTab} />
              {tab === "result" && <FullReport s={s} vol={preview.vol} base={preview.base} bonus={preview.bonus} />}
              {tab === "market" && (preview.market ? (
                <Card title="Notre loi face aux jeux comparables" sub={preview.market.selection_rule}>
                  <div className="faint" style={{ marginBottom: 8 }}>Source : {preview.market.source} · {preview.market.fetched}</div>
                  <div className="tablewrap">
                    <table>
                      <thead><tr><th>Indicateur</th><th className="n">n</th><th className="n">Médiane du marché</th><th className="n">P25 – P75</th><th className="n">Notre loi</th><th className="n">Rang centile</th></tr></thead>
                      <tbody>
                        {preview.market.metrics.map((m: any) => (
                          <tr key={m.key}>
                            <td>{m.label}</td><td className="n">{m.agg.n}</td><td className="n">{fmtMetric(m.fmt, m.agg.median)}</td><td className="n">{fmtMetric(m.fmt, m.agg.q1)} – {fmtMetric(m.fmt, m.agg.q3)}</td>
                            <td className="n"><b>{fmtMetric(m.fmt, m.ours)}</b></td><td className="n">{m.ours_rank_pct === null ? "—" : num(m.ours_rank_pct, 0)}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                  <h2 style={{ marginTop: 14 }}>Jeux comparables</h2>
                  <div className="tablewrap">
                    <table>
                      <thead><tr><th>Jeu</th><th>Studio</th><th className="n">Max</th><th className="n">Hit</th><th className="n">σ/prix</th><th className="n">Max win</th><th className="n">≥ 100×</th><th>Volatilité</th></tr></thead>
                      <tbody>
                        {preview.market.peers.map((r: any, i: number) => (
                          <tr key={i}><td>{r.name}</td><td>{r.studio}</td><td className="n">{x(r.max_mult)}</td><td className="n">{pct(r.hit, 1)}</td><td className="n">{num(r.sd_per_price, 1)}</td><td className="n">{oneIn(r.max_one_in)}</td><td className="n">{pct(r.share_100x, 0)}</td><td>{r.vol_class}</td></tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                  {peers && <><h2 style={{ marginTop: 14 }}>Statistiques complètes du groupe</h2><AggregatesTable aggs={peers.aggregates} /></>}
                  <Note>Non disponible dans la source : {preview.market.missing.join(" ; ")}.</Note>
                </Card>
              ) : <Note>Aucun jeu comparable dans la base pour ce plafond.</Note>)}
            </>
          )}
        </div>
      </div>
    </Page>
  );
}

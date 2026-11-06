import React, { useEffect, useMemo, useState } from "react";
import { call } from "../api";
import { useApp } from "../App";
import { FullReport } from "../components/statsview";
import { Card, Empty, ErrorBox, Nature, Note, Page } from "../components/ui";
import { int, num, oneInP, pct, x } from "../fmt";

export default function Explorer() {
  const { params, go } = useApp();
  const [rows, setRows] = useState<any[]>([]);
  const [q, setQ] = useState("");
  const [slotId, setSlotId] = useState<number | null>(params?.slotId ?? null);
  const [modeId, setModeId] = useState<number | null>(params?.modeId ?? null);
  const [full, setFull] = useState<any>(null);
  const [error, setError] = useState<any>(null);

  useEffect(() => {
    call("market_query", { filter: { providers: [], kinds: [], vol_classes: [], include_ours: true } }).then((d) => setRows(d.rows)).catch(setError);
  }, []);
  useEffect(() => {
    setFull(null);
    if (modeId) call("mode_full_stats", { modeId }).then(setFull).catch(setError);
  }, [modeId]);

  const slots = useMemo(() => {
    const m = new Map<number, any>();
    for (const r of rows) if (!m.has(r.slot_id)) m.set(r.slot_id, r);
    return Array.from(m.values()).filter((r) => !q || `${r.name} ${r.provider}`.toLowerCase().includes(q.toLowerCase()));
  }, [rows, q]);
  const modes = rows.filter((r) => r.slot_id === slotId);
  const mode = rows.find((r) => r.mode_id === modeId);
  const fig = mode?.figures ?? {};

  return (
    <Page title="Slot Explorer" intro="Une slot, tous ses modes. Avec une lookup table enregistrée, l'analyse complète est recalculée EXACTEMENT par notre moteur ; sinon seuls les chiffres importés (datés, sourcés) sont affichés.">
      <ErrorBox error={error} />
      <div className="grid" style={{ gridTemplateColumns: "320px 1fr" }}>
        <Card title="Slots" sub={`${slots.length}`}>
          <input type="text" placeholder="Rechercher…" value={q} onChange={(e) => setQ(e.target.value)} style={{ width: "100%", marginBottom: 8 }} />
          <div className="tablewrap tall">
            <table>
              <tbody>
                {slots.map((s) => (
                  <tr key={s.slot_id} className={`click ${slotId === s.slot_id ? "sel" : ""}`} onClick={() => { setSlotId(s.slot_id); setModeId(rows.find((r) => r.slot_id === s.slot_id && r.kind === "base")?.mode_id ?? rows.find((r) => r.slot_id === s.slot_id)?.mode_id); }}>
                    <td>{s.name}{s.ours && <span className="badge b-ok" style={{ marginLeft: 6 }}>NOTRE JEU</span>}<br /><span className="faint">{s.provider}</span></td>
                    <td className="n faint">{s.rank_bets ?? s.rank_turnover ?? ""}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </Card>
        <div className="stack">
          {!slotId && <Empty>Choisissez une slot.</Empty>}
          {slotId && (
            <Card title={modes[0]?.name} sub={modes[0]?.provider}>
              <table>
                <thead><tr><th>Mode</th><th>Nature</th><th className="n">Prix</th><th className="n">RTP</th><th className="n">Hit</th><th className="n">σ/prix</th><th className="n">Max</th><th className="n">Max win</th><th>Volatilité</th><th>Source</th></tr></thead>
                <tbody>
                  {modes.map((m) => (
                    <tr key={m.mode_id} className={`click ${m.mode_id === modeId ? "sel" : ""}`} onClick={() => setModeId(m.mode_id)}>
                      <td>{m.mode} <span className="faint">({m.kind})</span></td><td><Nature n={m.nature} /></td><td className="n">{num(m.cost, 0)}</td><td className="n">{pct(m.rtp, 3)}</td><td className="n">{pct(m.hit, 2)}</td>
                      <td className="n">{num(m.sd_per_price, 2)}</td><td className="n">{x(m.max_mult)}</td><td className="n">{oneInP(m.p_max)}</td><td>{m.vol_class ?? "—"}</td><td className="faint">{m.collected_at}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </Card>
          )}
          {mode && (
            <>
              <Note>
                Source : <b>{mode.source}</b> · données du {mode.collected_at}. {mode.has_lut ? "Lookup table enregistrée : analyse exacte ci-dessous." : "Pas de lookup table : chiffres importés uniquement — bonus, scatters et anticipation non disponibles dans cette source."}
              </Note>
              <div className="row">
                {mode.has_lut && <button className="btn primary" onClick={() => go("simulator", { target: { kind: "mode", mode_id: mode.mode_id }, label: `${mode.name} — ${mode.mode}` })}>Simuler ce mode</button>}
                <button className="btn" onClick={() => go("generator", { fromMode: mode })}>Nouveau modèle comparable</button>
              </div>
              {full?.stats ? (
                <FullReport s={full.stats} vol={full.vol} />
              ) : (
                <Card title="Chiffres disponibles" right={<Nature n={mode.nature} />}>
                  <div className="grid g4">
                    {[
                      ["RTP", pct(fig.rtp, 3)], ["Hit rate", pct(fig.hit, 2)], ["σ / prix", num(fig.vol, 2)], ["Max win (× prix)", x(fig.maxMult)], ["Fréquence du max", oneInP(fig.pmax)],
                      ["Médiane (× prix)", x(fig.median)], ["P25 / P75 / P90", `${x(fig.q25)} / ${x(fig.q75)} / ${x(fig.q90)}`], ["Part du RTP ≥ 10× / 100× / 1 000×", `${pct(fig.vs10, 0)} / ${pct(fig.vs100, 0)} / ${pct(fig.vs1000, 0)}`],
                      ["Top 1 % des rounds", pct(fig.top1, 1)], ["Gains < prix (parmi les gains)", pct(fig.winsUnderStake, 1)], ["Rounds < 0,1× prix", pct(fig.nearZero, 1)], ["Session 100 spins : médiane", fig.s100Median !== undefined ? `${num(fig.s100Median, 1)} mises` : "n.d."],
                    ].map(([l, v]) => (
                      <div key={l as string} className="stat" style={{ padding: "8px 10px" }}><div className="l">{l}</div><div className="v" style={{ fontSize: 15 }}>{v}</div></div>
                    ))}
                  </div>
                  <h2 style={{ marginTop: 14 }}>Paliers P(gain ≥ k × prix)</h2>
                  <table>
                    <thead><tr>{[1, 2, 5, 10, 50, 100, 500, 1000, 5000, 10000].map((k) => <th key={k} className="n">≥ {int(k)}×</th>)}</tr></thead>
                    <tbody><tr>{[1, 2, 5, 10, 50, 100, 500, 1000, 5000, 10000].map((k) => <td key={k} className="n">{fig[`p${k}`] === undefined ? "n.d." : oneInP(fig[`p${k}`])}</td>)}</tr></tbody>
                  </table>
                  {full?.reason && <div className="faint" style={{ marginTop: 8 }}>{full.reason}</div>}
                </Card>
              )}
            </>
          )}
        </div>
      </div>
    </Page>
  );
}

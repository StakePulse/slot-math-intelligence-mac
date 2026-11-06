import React, { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { call, VOL_CLASSES } from "../api";
import { useApp } from "../App";
import { HistogramChart, MarketScatter, ProvidersChart } from "../components/charts";
import { Card, Chips, Empty, ErrorBox, Field, JobBar, Nature, Note, NumInput, Page, Status, Tabs, useJob } from "../components/ui";
import { int, num, oneInP, pct, sci, x } from "../fmt";

const KINDS = ["base", "ante", "buy", "bust"];
const KIND_LABEL: Record<string, string> = { base: "jeu de base", ante: "ante / double chance", buy: "achat de bonus", bust: "achat tout-ou-rien" };

export function fmtMetric(fmt: string, v: any) {
  if (v === null || v === undefined) return "n.d.";
  if (fmt === "pct") return pct(v, 2);
  if (fmt === "x") return x(v);
  if (fmt === "one_in") return oneInP(v);
  return num(v, 2);
}

export function AggregatesTable({ aggs, onPick, picked }: { aggs: any[]; onPick?: (k: string) => void; picked?: string }) {
  return (
    <div className="tablewrap">
      <table>
        <thead>
          <tr>
            <th>Indicateur</th><th className="n">n</th><th className="n">Moyenne</th><th className="n">Médiane</th><th className="n">Pondérée bets</th><th className="n">Pondérée turnover</th>
            <th className="n">Écart-type</th><th className="n">P10</th><th className="n">Q1</th><th className="n">Q3</th><th className="n">P90</th><th className="n">Min – max</th><th className="n">Extrêmes</th>
          </tr>
        </thead>
        <tbody>
          {aggs.map((m) => {
            const a = m.agg;
            const f = (v: any) => fmtMetric(m.fmt, v);
            return (
              <tr key={m.key} className={`click ${picked === m.key ? "sel" : ""}`} onClick={() => onPick?.(m.key)}>
                <td>{m.label} {a.insufficient && <span className="badge b-warn" title="échantillon sous le minimum des réglages">n insuffisant</span>}</td>
                <td className="n">{a.n}</td><td className="n">{f(a.mean)}</td><td className="n">{f(a.median)}</td>
                <td className="n">{a.mean_weighted_bets === null ? <span className="faint">bets n.d.</span> : f(a.mean_weighted_bets)}</td>
                <td className="n">{f(a.mean_weighted_turnover)}</td>
                <td className="n">{m.fmt === "one_in" ? sci(a.sd) : m.fmt === "pct" ? pct(a.sd, 2) : num(a.sd, 2)}</td>
                <td className="n">{f(a.p10)}</td><td className="n">{f(a.q1)}</td><td className="n">{f(a.q3)}</td><td className="n">{f(a.p90)}</td>
                <td className="n">{f(a.min)} – {f(a.max)}</td><td className="n">{a.outliers.length}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

export default function Market() {
  const { go } = useApp();
  const [tab, setTab] = useState("reference");
  const [filter, setFilter] = useState<any>({ providers: [], kinds: ["base"], vol_classes: [], max_min: null, max_max: null, rtp_min: null, rtp_max: null, top_n: 50, include_ours: false, search: "" });
  const [data, setData] = useState<any>(null);
  const [ranking, setRanking] = useState<any[]>([]);
  const [error, setError] = useState<any>(null);
  const [metric, setMetric] = useState("sd_per_price");
  const [cmpProviders, setCmpProviders] = useState<string[]>([]);
  const [settings, setSettings] = useState<any>(null);
  const job = useJob();

  const load = () => {
    setError(null);
    const f = { ...filter, search: filter.search || null, rtp_min: filter.rtp_min === null ? null : filter.rtp_min / 100, rtp_max: filter.rtp_max === null ? null : filter.rtp_max / 100 };
    call("market_query", { filter: f }).then(setData).catch(setError);
    call("market_ranking").then(setRanking).catch(setError);
  };
  useEffect(load, [JSON.stringify(filter)]);
  useEffect(() => {
    call("settings_get").then((s) => setSettings(s.settings));
  }, []);

  const allProviders = useMemo(() => Array.from(new Set((ranking ?? []).map((r) => r.provider))).sort(), [ranking]);
  const agg = data?.aggregates?.find((a: any) => a.key === metric);
  const rows = data?.rows ?? [];

  async function importSnapshot() {
    const f = await open({ multiple: false, filters: [{ name: "market.json", extensions: ["json"] }] });
    if (!f) return;
    try {
      const r = await call("import_slotlab", { path: f });
      alert(r.message);
      load();
    } catch (e) {
      setError(e);
    }
  }
  async function importLut() {
    const f = await open({ multiple: false, filters: [{ name: "Lookup table", extensions: ["csv"] }] });
    if (!f) return;
    const slot = prompt("Nom de la slot ?");
    if (!slot) return;
    const provider = prompt("Provider ?", "Oracle Labs") ?? "Inconnu";
    const mode = prompt("Nom du mode ?", "base") ?? "base";
    const cost = Number((prompt("Prix du mode en mises de base ?", "1") ?? "1").replace(",", "."));
    const ours = confirm("Est-ce un de NOS jeux ? (OK = oui)");
    try {
      const r = await call("import_lut", { path: f, slotName: slot, provider, mode, cost, ours });
      alert(r.message);
      load();
    } catch (e) {
      setError(e);
    }
  }

  const collectorOn = settings?.collector?.enabled;
  return (
    <Page
      title="Market Intelligence"
      intro={<>Référence du marché : modes des slots les plus jouées, statistiques mathématiques <Nature n="EXACT" /> (calculées par notre moteur sur une lookup table) ou <Nature n="IMPORTÉ" /> (source tierce datée). Les volumes commerciaux (bets, turnover sur 30 jours) ne sont jamais des probabilités.</>}
      actions={<>
        <button className="btn" onClick={importSnapshot}>Importer un instantané</button>
        <button className="btn" onClick={importLut}>Importer une lookup table</button>
        <button className="btn primary" disabled={!collectorOn || job.job?.status === "running"} title={collectorOn ? "" : "Collecte désactivée (voir Réglages)"} onClick={() => job.start("collect_start", {}).then(load)}>Actualiser (StakeCruncher)</button>
      </>}
    >
      {!collectorOn && (
        <Note kind="warn">
          <b>Collecte automatique désactivée.</b> Le <span className="mono">robots.txt</span> de StakeCruncher interdit <span className="mono">/tracker-api/</span> aux robots, et ses pages n'affichent les données qu'en JavaScript via cette même API. L'activer est votre décision (Réglages → Collecte). En attendant, les données viennent de l'instantané tiers daté et des lookup tables importées.
        </Note>
      )}
      <JobBar j={job} />
      <ErrorBox error={error} />
      <Card title="Filtres">
        <div className="grid g4">
          <Field label="Nature du mode"><Chips options={KINDS} value={filter.kinds} onChange={(v) => setFilter({ ...filter, kinds: v })} /></Field>
          <Field label="Classe de volatilité"><Chips options={VOL_CLASSES} value={filter.vol_classes} onChange={(v) => setFilter({ ...filter, vol_classes: v })} /></Field>
          <Field label="Max win (× mise de base) min / max">
            <div className="row"><NumInput value={filter.max_min} onChange={(v) => setFilter({ ...filter, max_min: v })} placeholder="min" style={{ width: 100 }} /><NumInput value={filter.max_max} onChange={(v) => setFilter({ ...filter, max_max: v })} placeholder="max" style={{ width: 100 }} /></div>
          </Field>
          <Field label="RTP (%) min / max">
            <div className="row"><NumInput value={filter.rtp_min} onChange={(v) => setFilter({ ...filter, rtp_min: v })} placeholder="96" style={{ width: 80 }} /><NumInput value={filter.rtp_max} onChange={(v) => setFilter({ ...filter, rtp_max: v })} placeholder="97" style={{ width: 80 }} /></div>
          </Field>
          <Field label="Top N slots (popularité)"><NumInput value={filter.top_n} onChange={(v) => setFilter({ ...filter, top_n: v })} placeholder="toutes" /></Field>
          <Field label="Provider">
            <select value={filter.providers[0] ?? ""} onChange={(e) => setFilter({ ...filter, providers: e.target.value ? [e.target.value] : [] })}>
              <option value="">Tous les providers</option>
              {allProviders.map((p) => <option key={p}>{p}</option>)}
            </select>
          </Field>
          <Field label="Recherche"><input type="text" value={filter.search} onChange={(e) => setFilter({ ...filter, search: e.target.value })} placeholder="nom ou studio" /></Field>
          <Field label="Nos jeux"><label className="chk"><input type="checkbox" checked={filter.include_ours} onChange={(e) => setFilter({ ...filter, include_ours: e.target.checked })} /> inclure nos jeux</label></Field>
        </div>
        {data && <div className="faint" style={{ marginTop: 10 }}>{rows.length} modes · {new Set(rows.map((r: any) => r.slot_id)).size} slots · classement par <b>{data.basis}</b></div>}
      </Card>

      <Tabs tabs={[["reference", "Référence statistique"], ["ranking", "Classement"], ["providers", "Providers"], ["modes", "Modes"], ["sources", "Sources et collectes"]]} on={tab} set={setTab} />

      {tab === "reference" && data && (
        <>
          <Card title="Statistiques du groupe filtré" sub="un mode = une valeur ; cliquer une ligne pour sa distribution">
            <AggregatesTable aggs={data.aggregates} onPick={setMetric} picked={metric} />
            <div className="faint" style={{ marginTop: 6 }}>Quantiles par interpolation linéaire (type 7). Extrêmes : hors de [Q1 − 1,5·IQR ; Q3 + 1,5·IQR]. « n insuffisant » sous le minimum des réglages. Pondération par les bets impossible quand la source ne les fournit pas.</div>
          </Card>
          <div className="grid g2">
            <Card title={`Distribution : ${agg?.label ?? ""}`} sub={agg?.agg?.log_scale ? "classes en échelle log" : ""}>
              {agg && agg.agg.n > 0 ? <HistogramChart hist={agg.agg.histogram} label="modes" fmt={(v) => fmtMetric(agg.fmt, v)} /> : <Empty>Pas de valeur.</Empty>}
              {agg?.agg?.outliers?.length > 0 && <div className="faint">Valeurs extrêmes : {agg.agg.outliers.slice(0, 12).map((i: number) => rows.filter((r: any) => r[metric] !== null && r[metric] !== undefined && (metric !== "p_1000x" || r[metric] > 0))[i]?.name).filter(Boolean).join(", ")}</div>}
            </Card>
            <Card title="Max win × volatilité">
              <MarketScatter rows={rows.map((r: any) => ({ ...r, mm: r.max_mult, sd: r.sd_per_price }))} xKey="mm" yKey="sd" xLabel="max win (× prix)" yLabel="σ / prix" logX logY />
            </Card>
          </div>
          <div className="grid g2">
            <Card title="Hit rate × part du RTP au-delà de 100×">
              <MarketScatter rows={rows} xKey="hit" yKey="share_100x" xLabel="hit rate" yLabel="part du RTP ≥ 100×" />
            </Card>
            <Card title="Classes de volatilité du groupe">
              <table>
                <tbody>
                  {VOL_CLASSES.map((c) => {
                    const n = rows.filter((r: any) => r.vol_class === c).length;
                    return (
                      <tr key={c}><td>{c}</td><td className="n">{n}</td><td style={{ width: "60%" }}><div className="progress"><div style={{ width: `${rows.length ? (n * 100) / rows.length : 0}%` }} /></div></td></tr>
                    );
                  })}
                </tbody>
              </table>
              <div className="faint" style={{ marginTop: 6 }}>Score composite reproductible (σ/prix, pertes, part du RTP ≥ 100×, top 1 %, P(≥ 1 000×)) — seuils dans Réglages.</div>
            </Card>
          </div>
        </>
      )}

      {tab === "ranking" && (
        <Card title="Classement des slots" sub="popularité au nombre de bets sur 30 jours ; turnover à part">
          <div className="tablewrap tall">
            <table>
              <thead>
                <tr><th className="n">Rang bets</th><th className="n">Évol.</th><th>Slot</th><th>Provider</th><th className="n">Bets 7 j</th><th className="n">Bets 30 j</th><th className="n">Bets 90 j</th><th className="n">Rang turnover</th><th className="n">Part du turnover</th><th className="n">Joueurs</th><th>Volatilité (base)</th><th className="n">Max (base)</th><th>Source</th></tr>
              </thead>
              <tbody>
                {ranking.map((r: any, i: number) => {
                  const d = r.prev_rank_bets && r.rank_bets ? r.prev_rank_bets - r.rank_bets : null;
                  return (
                    <tr key={r.slot_id}>
                      <td className="n">{r.rank_bets ?? <span className="faint">n.d.</span>}</td>
                      <td className="n">{d === null ? "—" : d > 0 ? `▲ ${d}` : d < 0 ? `▼ ${-d}` : "="}</td>
                      <td>{r.name}</td><td>{r.provider}</td>
                      <td className="n">{r.bets_7d === null ? "n.d." : int(r.bets_7d)}</td><td className="n">{r.bets_30d === null ? "n.d." : int(r.bets_30d)}</td><td className="n">{r.bets_90d === null ? "n.d." : int(r.bets_90d)}</td>
                      <td className="n">{r.rank_turnover ?? "—"}</td><td className="n">{pct(r.turnover_share, 2)}</td><td className="n">{r.players_current ?? "n.d."}</td>
                      <td>{r.base_vol_class ?? "—"}</td><td className="n">{x(r.base_max)}</td><td className="faint">{r.source} · {r.collected_at}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </Card>
      )}

      {tab === "providers" && data && (
        <>
          <Card title="Comparer des providers" sub="modes de base du groupe filtré">
            <Chips options={data.providers.map((p: any) => p.studio)} value={cmpProviders} onChange={setCmpProviders} />
          </Card>
          <div className="grid g2">
            {[["median_sd", "σ / prix médian", (v: number) => num(v, 1)], ["median_max", "Max win médian", (v: number) => x(v)], ["median_hit", "Hit rate médian", (v: number) => pct(v, 1)], ["median_rtp", "RTP médian", (v: number) => pct(v, 2)]].map(([k, l, f]: any) => (
              <Card key={k} title={l}>
                <ProvidersChart providers={(cmpProviders.length ? data.providers.filter((p: any) => cmpProviders.includes(p.studio)) : data.providers.slice(0, 15))} metric={k} label={l} fmt={f} />
              </Card>
            ))}
          </div>
          <Card title="Tableau des providers">
            <div className="tablewrap">
              <table>
                <thead><tr><th>Provider</th><th className="n">Slots</th><th className="n">σ/prix médian</th><th className="n">Max médian</th><th className="n">Hit médian</th><th className="n">RTP médian</th><th>Slots les plus jouées</th></tr></thead>
                <tbody>
                  {data.providers.map((p: any) => (
                    <tr key={p.studio}>
                      <td>{p.studio}</td><td className="n">{p.slots}</td><td className="n">{num(p.median_sd, 1)}</td><td className="n">{x(p.median_max)}</td><td className="n">{pct(p.median_hit, 1)}</td><td className="n">{pct(p.median_rtp, 2)}</td>
                      <td className="faint">{Array.from(new Set(rows.filter((r: any) => r.provider === p.studio).map((r: any) => r.name))).slice(0, 4).join(", ")}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </Card>
        </>
      )}

      {tab === "modes" && data && (
        <Card title="Modes du groupe" sub="cliquer pour ouvrir dans Slot Explorer">
          <div className="tablewrap tall">
            <table>
              <thead><tr><th>Slot</th><th>Provider</th><th>Mode</th><th className="n">Prix</th><th className="n">RTP</th><th className="n">Hit</th><th className="n">σ/prix</th><th className="n">Max (× prix)</th><th className="n">Max win</th><th className="n">P90</th><th className="n">≥100× (RTP)</th><th>Volatilité</th><th>Nature</th></tr></thead>
              <tbody>
                {rows.map((r: any) => (
                  <tr key={r.mode_id} className="click" onClick={() => go("explorer", { slotId: r.slot_id, modeId: r.mode_id })}>
                    <td>{r.name}{r.ours && <span className="badge b-ok" style={{ marginLeft: 6 }}>NOTRE JEU</span>}</td><td>{r.provider}</td><td>{r.mode} <span className="faint">({KIND_LABEL[r.kind] ?? r.kind})</span></td>
                    <td className="n">{num(r.cost, 0)}</td><td className="n">{pct(r.rtp, 2)}</td><td className="n">{pct(r.hit, 1)}</td><td className="n">{num(r.sd_per_price, 1)}</td><td className="n">{x(r.max_mult)}</td><td className="n">{oneInP(r.p_max)}</td>
                    <td className="n">{x(r.q90)}</td><td className="n">{pct(r.share_100x, 0)}</td><td>{r.vol_class ?? "—"}</td><td><Nature n={r.nature} /></td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </Card>
      )}

      {tab === "sources" && data && (
        <Card title="Sources et collectes" sub="chaque donnée de marché est datée et rattachée à sa collecte">
          <table>
            <thead><tr><th className="n">#</th><th>Source</th><th>Type</th><th>Date des données</th><th>Statut</th><th className="n">Éléments</th><th className="n">Erreurs</th><th>Message</th></tr></thead>
            <tbody>
              {data.sources.map((s: any) => (
                <tr key={s.id}><td className="n">{s.id}</td><td>{s.source}<br /><span className="faint">{s.note}</span></td><td>{s.kind}</td><td>{s.date}</td><td><Status s={s.status} /></td><td className="n">{s.items}</td><td className="n">{s.errors}</td><td className="faint">{s.message}</td></tr>
              ))}
            </tbody>
          </table>
        </Card>
      )}
    </Page>
  );
}

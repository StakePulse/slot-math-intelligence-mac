import React, { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { call } from "../api";
import { int, num, oneIn, pct, x } from "../fmt";
import { Card, Empty, ErrorBox, JobBar, Note, Stat, Tabs, useJob } from "./ui";

const BADGE: Record<string, [string, string]> = {
  PASS: ["b-ok", "OK"],
  WARN: ["b-warn", "À REVOIR"],
  FAIL: ["b-bad", "ÉCHEC"],
  NOT_RUN: ["b-muted", "NON MESURABLE"],
};

function DBadge({ s }: { s: string }) {
  const [cls, label] = BADGE[s] ?? ["b-muted", s];
  return <span className={`badge ${cls}`}>{label}</span>;
}

const KIND: Record<string, string> = { base: "jeu de base", ante: "Ante", achat: "achat / feature" };
const TIER: Record<string, string> = { low: "bas", high: "haut", special: "spécial" };

/** Contrôles de conception d'un jeu TERMINÉ : lecture de son dossier publié (index.json, tables, books). */
export default function DesignChecks() {
  const job = useJob();
  const [list, setList] = useState<any[]>([]);
  const [sel, setSel] = useState<number | null>(null);
  const [rep, setRep] = useState<any>(null);
  const [mode, setMode] = useState<string>("");
  const [exported, setExported] = useState<string | null>(null);
  const [error, setError] = useState<any>(null);

  const loadList = () => call("design_reports_list").then(setList).catch(setError);
  useEffect(() => {
    loadList();
  }, []);
  useEffect(() => {
    setRep(null);
    setExported(null);
    if (sel) call("design_report_get", { id: sel }).then((r: any) => { setRep(r); setMode(r.modes[0]?.name ?? ""); }).catch(setError);
  }, [sel]);

  async function analyze() {
    setError(null);
    const dir = await open({ directory: true, multiple: false, title: "Dossier publié du jeu (celui qui contient index.json)" });
    if (!dir || Array.isArray(dir)) return;
    const end = await job.start("design_analyze_start", { path: dir });
    if (end?.status === "done") {
      await loadList();
      setSel(end.result.id);
    }
  }

  const m = rep?.modes.find((k: any) => k.name === mode);
  return (
    <div className="stack">
      <Note>
        Choisissez le <b>dossier publié</b> d'un jeu terminé : celui qui contient <code>index.json</code>, les lookup tables et les books
        (<code>books_*.jsonl.zst</code>). Le dossier est lu <b>sans aucune écriture</b> ; seul le rapport est enregistré ici. Toutes les mesures sont
        EXACTES sur les poids des tables. Seuils réglables dans Settings (bloc <code>design</code>) ; valeurs de départ à confirmer (DECISIONS.md D-17).
      </Note>
      <div className="row">
        <button className="btn primary" onClick={analyze}>Analyser un dossier publié…</button>
      </div>
      <JobBar j={job} />
      <ErrorBox error={error} />
      <div className="grid" style={{ gridTemplateColumns: "300px 1fr" }}>
        <Card title="Rapports" sub={`${list.length}`}>
          {list.length === 0 ? (
            <Empty>Aucun rapport.</Empty>
          ) : (
            <div className="tablewrap tall">
              <table>
                <tbody>
                  {list.map((r) => (
                    <tr key={r.id} className={`click ${sel === r.id ? "sel" : ""}`} onClick={() => setSel(r.id)}>
                      <td>
                        <b>{r.game}</b>
                        <br />
                        <span className="faint">{r.created_at}</span>
                      </td>
                      <td className="n">
                        <span className="b-ok">{r.passed}</span> · <span className="b-warn">{r.warnings}</span> · <span className="b-bad">{r.failed}</span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </Card>
        <div className="stack">
          {!rep && <Empty>Choisissez un rapport, ou analysez un dossier publié.</Empty>}
          {rep && (
            <>
              <Card
                title={`Contrôles de conception — ${rep.game}`}
                sub={rep.generated_at}
                right={
                  <button className="btn small" onClick={() => call<string>("design_report_export", { id: sel }).then(setExported).catch(setError)}>
                    Exporter (.md)
                  </button>
                }
              >
                <div className="faint" style={{ marginBottom: 8 }}>{rep.source} · moteur {rep.engine_version}</div>
                <div className="row">
                  <DBadge s="PASS" /> {rep.passed} <DBadge s="WARN" /> {rep.warnings} <DBadge s="FAIL" /> {rep.failed} <DBadge s="NOT_RUN" /> {rep.not_run}
                </div>
                {exported && (
                  <div className="row" style={{ marginTop: 8 }}>
                    <span className="faint">Écrit dans {exported}</span>
                    <button className="btn small" onClick={() => call("open_path", { path: exported })}>Ouvrir</button>
                  </div>
                )}
              </Card>
              <Tabs tabs={rep.modes.map((k: any) => [k.name, `${k.name} (${KIND[k.kind] ?? k.kind})`])} on={mode} set={setMode} />
              {m && <ModeView m={m} />}
            </>
          )}
        </div>
      </div>
    </div>
  );
}

function ModeView({ m }: { m: any }) {
  const flagged = m.bands.filter((b: any) => b.flag);
  return (
    <>
      <div className="grid g6">
        <Stat label="Books" value={int(m.books)} sub={`prix ${x(m.cost)}`} />
        <Stat label="RTP" value={pct(m.rtp, 3)} />
        <Stat label="Hit rate" value={pct(m.hit, 2)} sub={`mort ${pct(m.dead, 1)} · sous la mise ${pct(m.under_cost, 1)} · ≥ mise ${pct(m.ge_cost, 1)}`} />
        <Stat label="σ / prix" value={num(m.sd_per_price, 2)} />
        <Stat label="Max win" value={`${x(m.max_price)} le prix`} sub={oneIn(m.max_one_in)} />
        <Stat label="Bonus" value={m.bonus_one_in ? oneIn(m.bonus_one_in) : "—"} sub={m.bonus_share !== null && m.bonus_share !== undefined ? `${pct(m.bonus_share, 1)} du RTP` : undefined} />
      </div>
      <Card title="Contrôles">
        <table>
          <thead>
            <tr><th style={{ width: 110 }}>Statut</th><th>Contrôle</th><th>Mesuré</th><th>Cible</th></tr>
          </thead>
          <tbody>
            {m.checks.map((c: any) => (
              <tr key={c.key + c.observed}>
                <td><DBadge s={c.status} /></td>
                <td>{c.label}{c.detail && <><br /><span className="faint">{c.detail}</span></>}</td>
                <td>{c.observed}</td>
                <td className="muted">{c.target}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </Card>
      {m.scatters.length > 0 && (
        <Card title="Bonus selon le nombre de scatters au déclenchement" sub="gain du round en multiples de la mise de base">
          <table>
            <thead><tr><th>Bonus</th><th className="n">Scatters</th><th className="n">Books</th><th className="n">Fréquence</th><th className="n">Moyenne</th><th className="n">Médiane</th></tr></thead>
            <tbody>
              {m.scatters.map((s: any) => (
                <tr key={s.group + s.scatters} className={s.small ? "faint" : ""}>
                  <td>{s.group}</td><td className="n">{s.scatters}</td><td className="n">{int(s.books)}{s.small ? " (trop peu)" : ""}</td>
                  <td className="n">{oneIn(s.one_in)}</td><td className="n">{x(s.mean)}</td><td className="n">{x(s.median)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </Card>
      )}
      <div className="grid g2">
        {m.floors.length > 0 && (
          <Card title="Planchers" sub="minimum, médiane, part collée au plancher">
            <table>
              <thead><tr><th>Groupe</th><th className="n">Books</th><th className="n">Plancher</th><th className="n">Médiane</th><th className="n">Rapport</th><th className="n">≤ +10 %</th></tr></thead>
              <tbody>
                {m.floors.map((f: any) => (
                  <tr key={f.group} className={f.has_floor ? "" : "faint"}>
                    <td>{f.group}{!f.has_floor && " (pas de plancher voulu)"}</td><td className="n">{int(f.books)}</td><td className="n">{x(f.floor)}</td>
                    <td className="n">{x(f.median)}</td><td className="n">{f.ratio ? num(f.ratio, 2) : "—"}</td><td className="n">{f.has_floor ? pct(f.near_floor, 1) : "—"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </Card>
        )}
        {flagged.length > 0 && (
          <Card title="Tranches à revoir" sub="multiples de la mise de base">
            <table>
              <thead><tr><th>Classe</th><th>Tranche</th><th className="n">Books</th><th>État</th></tr></thead>
              <tbody>
                {flagged.map((b: any) => (
                  <tr key={b.class + b.label}><td>{b.class}</td><td>{b.label}</td><td className="n">{int(b.books)}</td><td className={b.flag === "trou" ? "b-bad" : "b-warn"}>{b.flag}</td></tr>
                ))}
              </tbody>
            </table>
          </Card>
        )}
        {m.symbols.length > 0 && (
          <Card title="Symboles lus sur les plateaux du jeu de base" sub="rang automatique, modifiable dans Settings (high_symbols / low_symbols)">
            <table>
              <thead><tr><th>Symbole</th><th>Rang</th><th className="n">Apparitions</th></tr></thead>
              <tbody>
                {m.symbols.map((s: any) => (
                  <tr key={s[0]}><td>{s[0]}</td><td>{TIER[s[1]] ?? s[1]}</td><td className="n">{int(s[2])}</td></tr>
                ))}
              </tbody>
            </table>
          </Card>
        )}
        {m.criteria.length > 0 && (
          <Card title="Critères des books" sub="champ criteria ; « bonus » d'après les critères et les déclenchements">
            <table>
              <thead><tr><th>Critère</th><th className="n">Books</th><th>Classé</th></tr></thead>
              <tbody>
                {m.criteria.map((c: any) => (
                  <tr key={c[0]}><td>{c[0] || "(vide)"}</td><td className="n">{int(c[1])}</td><td>{c[2] ? "bonus" : "sans bonus"}</td></tr>
                ))}
              </tbody>
            </table>
          </Card>
        )}
      </div>
    </>
  );
}

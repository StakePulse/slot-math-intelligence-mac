import React, { useEffect, useState } from "react";
import { call } from "../api";
import { useApp } from "../App";
import { Card, Empty, ErrorBox, Page, Status } from "../components/ui";
import { int, ms, pct, x } from "../fmt";

export default function Library() {
  const { go } = useApp();
  const [models, setModels] = useState<any[]>([]);
  const [sel, setSel] = useState<any>(null);
  const [detail, setDetail] = useState<any>(null);
  const [a, setA] = useState<number | null>(null);
  const [b, setB] = useState<number | null>(null);
  const [diff, setDiff] = useState<any[] | null>(null);
  const [error, setError] = useState<any>(null);
  useEffect(() => {
    call("models_list").then(setModels).catch(setError);
  }, []);
  useEffect(() => {
    setDiff(null);
    if (sel) call("model_get", { id: sel.id, version: null }).then((d) => { setDetail(d); const v = d.versions.map((x: any) => x.version); setA(v[1] ?? v[0]); setB(v[0]); }).catch(setError);
  }, [sel?.id]);
  return (
    <Page title="Math Library" intro="Tous les modèles : identifiant unique, versions immuables avec leur note de changement, statut de validation, sources, simulations. Deux versions d'une même slot se comparent champ par champ.">
      <ErrorBox error={error} />
      <Card title="Modèles" sub={`${models.length}`}>
        {models.length === 0 ? (
          <Empty>Aucun modèle pour l'instant.</Empty>
        ) : (
          <div className="tablewrap">
            <table>
              <thead><tr><th>Nom</th><th>Identifiant</th><th>Créé</th><th>Modifié</th><th className="n">Versions</th><th className="n">Max</th><th className="n">RTP</th><th>Volatilité</th><th className="n">Simulations</th><th>Sources</th><th>Statut</th></tr></thead>
              <tbody>
                {models.map((m) => (
                  <tr key={m.id} className={`click ${sel?.id === m.id ? "sel" : ""}`} onClick={() => setSel(m)}>
                    <td>{m.name}</td><td className="mono faint">{m.id.slice(0, 8)}…</td><td className="faint">{m.created_at}</td><td className="faint">{m.updated_at}</td><td className="n">{m.versions}</td>
                    <td className="n">{x(m.max_win)}</td><td className="n">{pct(m.rtp, 2)}</td><td>{m.volatility}</td><td className="n">{m.simulations}</td><td className="faint">{m.sources.join(", ")}</td><td><Status s={m.status} /></td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Card>
      {detail && (
        <div className="grid g2">
          <Card title={`Historique — ${detail.model.name}`} right={<div className="row"><button className="btn small" onClick={() => go("engine", { model: { id: sel.id } })}>Ouvrir</button><button className="btn small" onClick={() => go("generator", { spec: detail.model.spec, modelId: sel.id })}>Nouvelle version</button></div>}>
            <table>
              <thead><tr><th className="n">Version</th><th>Date</th><th>Changement</th><th>Empreinte</th><th>Statut</th></tr></thead>
              <tbody>
                {detail.versions.map((v: any) => (
                  <tr key={v.version} className="click" onClick={() => go("engine", { model: { id: sel.id, version: v.version } })}>
                    <td className="n">v{v.version}</td><td className="faint">{v.created_at}</td><td>{v.change_note}</td><td className="mono faint">{v.fingerprint}</td><td><Status s={v.status} /></td>
                  </tr>
                ))}
              </tbody>
            </table>
            <h2 style={{ marginTop: 14 }}>Simulations de ce modèle</h2>
            <table>
              <tbody>
                {detail.simulations.map((s: any) => (
                  <tr key={s.id}><td className="faint">{s.created_at}</td><td>{s.label}</td><td className="n">{int(s.spins)} spins</td><td className="n">{pct(s.rtp, 4)}</td><td className="n">{ms(s.elapsed_ms)}</td><td><Status s={s.completed ? "OK" : "PARTIEL"} /></td></tr>
                ))}
              </tbody>
            </table>
          </Card>
          <Card title="Comparer deux versions">
            {detail.versions.length < 2 ? (
              <Empty>Une seule version : créez-en une autre depuis le Math Generator.</Empty>
            ) : (
              <>
                <div className="row">
                  <select value={a ?? ""} onChange={(e) => setA(Number(e.target.value))}>{detail.versions.map((v: any) => <option key={v.version} value={v.version}>v{v.version}</option>)}</select>
                  <span className="muted">contre</span>
                  <select value={b ?? ""} onChange={(e) => setB(Number(e.target.value))}>{detail.versions.map((v: any) => <option key={v.version} value={v.version}>v{v.version}</option>)}</select>
                  <button className="btn primary" onClick={() => call("model_diff", { id: sel.id, a, b }).then(setDiff).catch(setError)}>Comparer</button>
                </div>
                {diff && (
                  <table style={{ marginTop: 10 }}>
                    <thead><tr><th>Champ</th><th>v{a}</th><th>v{b}</th></tr></thead>
                    <tbody>
                      {diff.map((d) => (
                        <tr key={d.field} style={d.changed ? { background: "#1d2733" } : {}}><td>{d.field}{d.changed && <span className="badge b-warn" style={{ marginLeft: 6 }}>modifié</span>}</td><td className="mono">{d.a}</td><td className="mono">{d.b}</td></tr>
                      ))}
                    </tbody>
                  </table>
                )}
              </>
            )}
          </Card>
        </div>
      )}
    </Page>
  );
}

import React, { useEffect, useState } from "react";
import { call } from "../api";
import { useApp } from "../App";
import { Card, Empty, ErrorBox, Note, Page, Stat, Status } from "../components/ui";
import { int, pct, x } from "../fmt";

export default function Dashboard() {
  const { go } = useApp();
  const [d, setD] = useState<any>(null);
  const [error, setError] = useState<any>(null);
  const [starter, setStarter] = useState<any>(null);
  const load = () => call("dashboard").then(setD).catch(setError);
  useEffect(() => {
    load();
    // Premier lancement : le pack de départ s'installe en arrière-plan ; on suit son état.
    let stop = false;
    const poll = () =>
      call("starter_status")
        .then((s: any) => {
          if (stop) return;
          setStarter(s);
          if (s.status === "running") setTimeout(poll, 1500);
          else if (s.status === "done") load();
        })
        .catch(() => {});
    poll();
    return () => {
      stop = true;
    };
  }, []);
  if (error) return <Page title="Dashboard"><ErrorBox error={error} /></Page>;
  if (!d) return <Page title="Dashboard"><Empty>Chargement…</Empty></Page>;
  const lc = d.last_collection;
  return (
    <Page title="Dashboard" intro="État de la bibliothèque : données de marché datées, modèles, simulations, documents, et les alertes de cohérence à traiter.">
      {starter?.status === "running" && (
        <Note>Premier lancement : installation des données de départ dans {starter.root} (référence datée du marché, modèle d'exemple). Comptez moins d'une minute.</Note>
      )}
      {starter?.status === "done" && (
        <Note>Données de départ installées dans {starter.root} : {starter.report?.copied ?? 0} fichier(s) copié(s){starter.report?.errors?.length ? `, ${starter.report.errors.length} erreur(s) (voir les alertes)` : ""}. Le modèle d'exemple est à valider dans Math Engine.</Note>
      )}
      {starter?.status === "error" && <ErrorBox error={`Installation des données de départ : ${starter.message}`} />}
      <div className="grid g6">
        <Stat label="Slots analysées" value={int(d.slots)} sub={`${int(d.modes)} modes`} />
        <Stat label="Providers" value={int(d.providers)} />
        <Stat label="Modèles mathématiques" value={int(d.models)} />
        <Stat label="Simulations" value={int(d.simulations)} />
        <Stat label="Documents" value={int(d.documents)} sub={`${int(d.facts_to_validate)} valeurs à valider`} />
        <Stat label="Dernière actualisation" value={lc ? lc[0].slice(0, 10) : "—"} sub={lc ? `${lc[1]} · ${lc[2]}` : "aucune collecte"} />
      </div>
      <div className="grid g2">
        <Card title="Alertes de cohérence" sub={`${d.alerts.length} ouverte(s)`}>
          {d.alerts.length === 0 ? (
            <Empty>Aucune alerte ouverte.</Empty>
          ) : (
            <table>
              <tbody>
                {d.alerts.map((a: any) => (
                  <tr key={a[0]}>
                    <td style={{ width: 90 }}><Status s={a[2] === "ERROR" ? "FAIL" : "WARN"} /></td>
                    <td><span className="faint">{a[1]} · {a[3]}</span><br />{a[4]}</td>
                    <td style={{ width: 70 }}><button className="btn small" onClick={() => call("alert_resolve", { id: a[0] }).then(load)}>Traitée</button></td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </Card>
        <Card title="Projets récents" right={<button className="btn small" onClick={() => go("generator")}>Nouveau modèle</button>}>
          {d.recent_models.length === 0 ? (
            <Empty>Aucun modèle. Ouvrez le Math Generator pour en créer un.</Empty>
          ) : (
            <table>
              <thead><tr><th>Modèle</th><th className="n">Version</th><th className="n">Max</th><th className="n">RTP</th><th>Volatilité</th><th>Statut</th></tr></thead>
              <tbody>
                {d.recent_models.map((m: any) => (
                  <tr key={m.id} className="click" onClick={() => go("engine", { model: { id: m.id } })}>
                    <td>{m.name}<br /><span className="faint">{m.updated_at}</span></td>
                    <td className="n">v{m.current_version}</td><td className="n">{x(m.max_win)}</td><td className="n">{pct(m.rtp, 2)}</td><td>{m.volatility}</td><td><Status s={m.status} /></td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </Card>
      </div>
      <div className="grid g2">
        <Card title="Dernières analyses et actions">
          <table>
            <tbody>
              {d.activity.map((a: any, i: number) => (
                <tr key={i}><td className="faint" style={{ width: 150 }}>{a[0]}</td><td style={{ width: 90 }}>{a[1]}</td><td>{a[2]}</td></tr>
              ))}
            </tbody>
          </table>
        </Card>
        <Card title="Dernière collecte">
          {lc ? (
            <div className="stack">
              <div><b>{lc[1]}</b> · <Status s={lc[2]} /> · {lc[0]}</div>
              <div className="muted">{lc[3]}</div>
              <button className="btn" onClick={() => go("market")}>Ouvrir Market Intelligence</button>
            </div>
          ) : (
            <Empty>Aucune donnée de marché. Importez un instantané ou une lookup table depuis Market Intelligence.</Empty>
          )}
        </Card>
      </div>
    </Page>
  );
}

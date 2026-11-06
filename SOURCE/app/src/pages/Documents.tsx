import React, { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { call } from "../api";
import { useApp } from "../App";
import { Card, Chips, Empty, ErrorBox, Field, Note, Page, Status } from "../components/ui";
import { bytes, int } from "../fmt";

const CATS = ["PDF", "SCREENSHOTS", "EXCEL", "OTHER"];

export default function Documents() {
  const { info } = useApp();
  const [docs, setDocs] = useState<any[]>([]);
  const [cats, setCats] = useState<string[]>([]);
  const [sel, setSel] = useState<any>(null);
  const [facts, setFacts] = useState<any[]>([]);
  const [text, setText] = useState<string | null>(null);
  const [showText, setShowText] = useState(false);
  const [q, setQ] = useState("");
  const [hits, setHits] = useState<any[] | null>(null);
  const [error, setError] = useState<any>(null);
  const [onlyPending, setOnlyPending] = useState(true);
  const [manual, setManual] = useState({ key: "rtp", label: "RTP", value: "", page: "", context: "" });

  const load = () => call("docs_list").then(setDocs).catch(setError);
  useEffect(() => {
    load();
  }, []);
  const pick = (d: any) => {
    setSel(d);
    setShowText(false);
    setText(null);
    call("doc_facts", { id: d.id }).then(setFacts);
  };
  async function importFiles() {
    const f = await open({ multiple: true, filters: [{ name: "Documents", extensions: ["pdf", "png", "jpg", "jpeg", "webp", "csv", "json", "txt", "md", "xlsx", "xls", "xlsm", "ods", "zip"] }] });
    if (!f) return;
    const paths = Array.isArray(f) ? f : [f];
    const r = await call("docs_import", { paths });
    const errs = r.filter((x: any) => x.error);
    alert(`${r.filter((x: any) => x.new).length} nouveau(x), ${r.filter((x: any) => !x.new && !x.error).length} déjà présent(s), ${errs.length} erreur(s)${errs.length ? "\n" + errs.map((e: any) => `${e.path} : ${e.error}`).join("\n") : ""}`);
    load();
  }
  async function review(id: number, status: string, corrected?: string) {
    await call("fact_review", { id, status, corrected: corrected ?? null });
    call("doc_facts", { id: sel.id }).then(setFacts);
    load();
  }
  const shown = docs.filter((d) => cats.length === 0 || cats.includes(d.category));
  const fShown = facts.filter((f) => !onlyPending || f.status === "À VALIDER");
  return (
    <Page
      title="Document Library"
      intro="PDF, captures, CSV, JSON, TXT, Excel : classés, dédoublonnés par empreinte SHA-256, texte extrait quand c'est possible. Les valeurs repérées sont des CANDIDATS « À VALIDER » : elles ne servent à rien tant qu'un humain ne les a pas confirmées."
      actions={<>
        <button className="btn" onClick={() => call("docs_sync").then((r) => { alert(`${r.added} document(s) intégré(s) depuis le dossier DOCUMENTS.`); load(); })}>Intégrer le dossier DOCUMENTS</button>
        <button className="btn" onClick={() => call("open_path", { path: info?.paths?.documents })}>Ouvrir le dossier</button>
        <button className="btn primary" onClick={importFiles}>Importer des fichiers</button>
      </>}
    >
      <ErrorBox error={error} />
      <Note>Dépôt manuel : posez vos fichiers dans <span className="mono">DOCUMENTS\INBOX</span> puis « Intégrer » — ils sont rangés dans leur catégorie. Pas de reconnaissance de caractères installée : pour une image ou un PDF sans texte, saisissez les valeurs à la main (elles passent aussi par « À VALIDER »).</Note>
      <div className="grid" style={{ gridTemplateColumns: "minmax(380px, 1fr) 1.4fr" }}>
        <Card title="Bibliothèque" sub={`${docs.length} documents`}>
          <div className="row" style={{ marginBottom: 8 }}>
            <Chips options={CATS} value={cats} onChange={setCats} />
          </div>
          <div className="row" style={{ marginBottom: 8 }}>
            <input type="text" placeholder="Rechercher dans les textes…" value={q} onChange={(e) => setQ(e.target.value)} style={{ flex: 1 }} />
            <button className="btn small" onClick={() => (q ? call("docs_search", { q }).then(setHits) : setHits(null))}>Chercher</button>
          </div>
          {hits && (
            <div className="note" style={{ marginBottom: 8 }}>
              {hits.length === 0 ? "Aucun résultat." : hits.map((h: any) => <div key={h[0]} className="click" style={{ cursor: "pointer" }} onClick={() => pick(docs.find((d) => d.id === h[0]))}><b>{h[1]}</b> — <span className="faint">{h[2].replace(/\s+/g, " ")}</span></div>)}
            </div>
          )}
          <div className="tablewrap tall">
            <table>
              <thead><tr><th>Document</th><th>Type</th><th className="n">Taille</th><th>Texte</th><th className="n">À valider</th></tr></thead>
              <tbody>
                {shown.map((d) => (
                  <tr key={d.id} className={`click ${sel?.id === d.id ? "sel" : ""}`} onClick={() => pick(d)}>
                    <td>{d.filename}{d.is_lut && <span className="badge b-ok" style={{ marginLeft: 6 }}>LOOKUP TABLE</span>}<br /><span className="faint">{d.imported_at} · {d.tags}</span></td>
                    <td>{d.kind}</td><td className="n">{bytes(d.size)}</td><td className="faint">{d.text_status}</td><td className="n">{d.facts_to_validate}/{d.facts_total}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </Card>
        <div className="stack">
          {!sel && <Empty>Choisissez un document.</Empty>}
          {sel && (
            <>
              <Card title={sel.filename} sub={`${sel.category} · ${sel.pages ?? "?"} page(s)`}>
                <div className="row">
                  <button className="btn small" onClick={() => call("open_path", { path: sel.path })}>Ouvrir</button>
                  <button className="btn small" onClick={() => { setShowText(!showText); if (text === null) call("doc_text", { id: sel.id }).then((t) => setText(t ?? "")); }}>{showText ? "Masquer le texte" : "Voir le texte extrait"}</button>
                  <button className="btn small" onClick={() => call("doc_reanalyze", { id: sel.id }).then((n) => { alert(`${n} valeur(s) repérée(s).`); pick(sel); })}>Réanalyser</button>
                  {sel.is_lut && <button className="btn small primary" onClick={async () => {
                    const slot = prompt("Nom de la slot ?"); if (!slot) return;
                    const provider = prompt("Provider ?", "Inconnu") ?? "Inconnu";
                    const mode = prompt("Mode ?", "base") ?? "base";
                    const cost = Number((prompt("Prix du mode (× mise) ?", "1") ?? "1").replace(",", "."));
                    try { const r = await call("import_lut", { path: sel.path, slotName: slot, provider, mode, cost, ours: confirm("Un de nos jeux ?") }); alert(r.message); } catch (e) { setError(e); }
                  }}>Analyser comme lookup table</button>}
                </div>
                <div className="grid g2" style={{ marginTop: 10 }}>
                  <Field label="Note"><input type="text" defaultValue={sel.note} onBlur={(e) => call("doc_meta", { id: sel.id, note: e.target.value, tags: sel.tags }).then(load)} /></Field>
                  <Field label="Étiquettes"><input type="text" defaultValue={sel.tags} onBlur={(e) => call("doc_meta", { id: sel.id, note: sel.note, tags: e.target.value }).then(load)} /></Field>
                </div>
                {showText && <textarea rows={16} readOnly value={text ?? "…"} style={{ marginTop: 10 }} />}
              </Card>
              <Card title="Valeurs extraites" sub={`${facts.length} candidat(s)`} right={<label className="chk"><input type="checkbox" checked={onlyPending} onChange={(e) => setOnlyPending(e.target.checked)} /> seulement à valider</label>}>
                <div className="tablewrap">
                  <table>
                    <thead><tr><th>Valeur</th><th>Type</th><th className="n">Page</th><th>Contexte</th><th>Statut</th><th /></tr></thead>
                    <tbody>
                      {fShown.map((f) => (
                        <tr key={f.id}>
                          <td><b>{f.validated_value ?? f.value}</b></td><td>{f.label}</td><td className="n">{f.page ?? "—"}</td><td className="faint" style={{ maxWidth: 380 }}>{f.context}</td><td><Status s={f.status} /></td>
                          <td style={{ whiteSpace: "nowrap" }}>
                            <button className="btn small" onClick={() => review(f.id, "VALIDÉ")}>Valider</button>{" "}
                            <button className="btn small" onClick={() => { const v = prompt("Valeur corrigée :", f.value); if (v) review(f.id, "VALIDÉ", v); }}>Corriger</button>{" "}
                            <button className="btn small danger" onClick={() => review(f.id, "REJETÉ")}>Rejeter</button>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
                {fShown.length === 0 && <Empty>Rien à afficher.</Empty>}
              </Card>
              <Card title="Saisie manuelle" sub="entre « À VALIDER »">
                <div className="grid g4">
                  <Field label="Type">
                    <select value={manual.key} onChange={(e) => setManual({ ...manual, key: e.target.value, label: e.target.selectedOptions[0].text })}>
                      {[["rtp", "RTP"], ["hit_rate", "Hit rate"], ["max_win", "Max win (×)"], ["one_in", "Fréquence « 1 sur N »"], ["sd", "Écart-type / volatilité"], ["bonus_freq", "Fréquence des bonus"], ["price", "Prix d'un mode"], ["other", "Autre"]].map(([k, l]) => <option key={k} value={k}>{l}</option>)}
                    </select>
                  </Field>
                  <Field label="Valeur"><input type="text" value={manual.value} onChange={(e) => setManual({ ...manual, value: e.target.value })} /></Field>
                  <Field label="Page"><input type="text" value={manual.page} onChange={(e) => setManual({ ...manual, page: e.target.value })} /></Field>
                  <Field label="Contexte"><input type="text" value={manual.context} onChange={(e) => setManual({ ...manual, context: e.target.value })} /></Field>
                </div>
                <button className="btn" style={{ marginTop: 8 }} disabled={!manual.value} onClick={() => call("fact_add", { documentId: sel.id, key: manual.key, label: manual.label, value: manual.value, page: manual.page ? int(Number(manual.page)) && Number(manual.page) : null, context: manual.context }).then(() => { setManual({ ...manual, value: "", context: "" }); pick(sel); load(); })}>Ajouter</button>
              </Card>
            </>
          )}
        </div>
      </div>
    </Page>
  );
}

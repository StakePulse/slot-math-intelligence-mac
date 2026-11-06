import React, { useEffect, useState } from "react";
import { call } from "../api";
import { useApp } from "../App";
import { Card, Empty, ErrorBox, Note, Page, Status } from "../components/ui";
import { ModelPicker } from "./Engine";

function esc(s: string) {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}
function inline(s: string) {
  return esc(s).replace(/\*\*(.+?)\*\*/g, "<b>$1</b>").replace(/`([^`]+)`/g, "<code>$1</code>");
}

/** Rendu Markdown minimal (titres, tableaux, listes, citations, code) pour l'aperçu. */
export function renderMarkdown(md: string): string {
  const lines = md.split("\n");
  const out: string[] = [];
  let i = 0;
  while (i < lines.length) {
    const l = lines[i];
    if (l.startsWith("```")) {
      const buf: string[] = [];
      i++;
      while (i < lines.length && !lines[i].startsWith("```")) buf.push(esc(lines[i++]));
      out.push(`<pre>${buf.join("\n")}</pre>`);
      i++;
      continue;
    }
    if (l.startsWith("|")) {
      const rows: string[][] = [];
      while (i < lines.length && lines[i].startsWith("|")) {
        const cells = lines[i].slice(1, -1).split("|").map((c) => c.trim());
        if (!cells.every((c) => /^:?-{2,}:?$/.test(c))) rows.push(cells);
        i++;
      }
      const [head, ...body] = rows;
      out.push(`<table><thead><tr>${head.map((c) => `<th>${inline(c)}</th>`).join("")}</tr></thead><tbody>${body.map((r) => `<tr>${r.map((c) => `<td>${inline(c)}</td>`).join("")}</tr>`).join("")}</tbody></table>`);
      continue;
    }
    const h = /^(#{1,3}) (.*)$/.exec(l);
    if (h) out.push(`<h${h[1].length}>${inline(h[2])}</h${h[1].length}>`);
    else if (l.startsWith("> ")) out.push(`<blockquote>${inline(l.slice(2))}</blockquote>`);
    else if (/^(- |\d+\. )/.test(l)) {
      const ordered = /^\d+\. /.test(l);
      const items: string[] = [];
      while (i < lines.length && /^(- |\d+\. )/.test(lines[i])) items.push(`<li>${inline(lines[i++].replace(/^(- |\d+\. )/, ""))}</li>`);
      out.push(ordered ? `<ol>${items.join("")}</ol>` : `<ul>${items.join("")}</ul>`);
      continue;
    } else if (l.trim()) out.push(`<p>${inline(l)}</p>`);
    i++;
  }
  return out.join("\n");
}

export default function PromptPage() {
  const { model, setModel, go } = useApp();
  const [data, setData] = useState<any>(null);
  const [error, setError] = useState<any>(null);
  const [view, setView] = useState<"rendu" | "texte">("rendu");
  const build = () => {
    if (!model?.id) return;
    setError(null);
    call("prompt_build", { id: model.id, version: model.version ?? null }).then(setData).catch(setError);
  };
  useEffect(build, [model?.id, model?.version]);
  async function exportWhat(what: string) {
    try {
      const r = await call("export_files", { id: model!.id, version: model!.version ?? null, what });
      alert(`Fichiers écrits :\n${r.files.join("\n")}`);
    } catch (e) {
      setError(e);
    }
  }
  const status = data?.status;
  return (
    <Page
      title="Prompt Generator"
      intro="Le cahier des charges mathématique destiné à Claude : objectifs, loi cible exacte (classes, bandes, percentiles, seuils), résultats de simulation, références du marché datées, contraintes techniques et critères d'acceptation. Il exige un modèle vérifiable, jamais des nombres arbitraires."
      actions={<ModelPicker value={model} onChange={setModel} />}
    >
      <ErrorBox error={error} />
      {!model?.id && <Empty>Choisissez un modèle.</Empty>}
      {data && (
        <>
          {status !== "VALIDATED" && status !== "READY_FOR_EXPORT" && (
            <Note kind="warn">
              Statut de la loi : <Status s={status ?? "DRAFT"} />. Le prompt le dit explicitement. Pour un prompt « prêt », lancez la validation complète dans le Math Engine.{" "}
              <button className="btn small" onClick={() => go("engine", { model })}>Ouvrir la validation</button>
            </Note>
          )}
          <Card
            title={`${data.name} — v${data.version}`}
            right={
              <div className="row">
                <button className="btn small" onClick={() => setView(view === "rendu" ? "texte" : "rendu")}>{view === "rendu" ? "Texte brut" : "Rendu"}</button>
                <button className="btn small primary" onClick={() => navigator.clipboard.writeText(data.markdown).then(() => alert("Prompt copié."))}>Copier</button>
                <button className="btn small" onClick={() => exportWhat("prompt_md")}>Exporter .md</button>
                <button className="btn small" onClick={() => exportWhat("prompt_txt")}>Exporter .txt</button>
                <button className="btn small" onClick={() => exportWhat("model_json")}>Données JSON + lookup table</button>
                <button className="btn small" onClick={() => exportWhat("validation")}>Rapport de validation</button>
              </div>
            }
          >
            {view === "rendu" ? <div className="md" dangerouslySetInnerHTML={{ __html: renderMarkdown(data.markdown) }} /> : <textarea rows={40} readOnly value={data.markdown} />}
          </Card>
        </>
      )}
    </Page>
  );
}

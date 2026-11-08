import { useEffect, useRef, useState } from "react";
import { CheckCircle2, CircleAlert, CircleHelp, CircleX, ExternalLink, Gamepad2, Loader2, RefreshCw, Search, Square } from "lucide-react";
import { api } from "../lib/api";
import type { GameCheck, GameVerdict, InstalledGame, SearchHit } from "../lib/types";

const verdictText: Record<GameVerdict, string> = {
  recommended: "Runs well",
  minimum: "Low settings",
  below: "Too slow",
  unknown: "Can't tell",
};

function VerdictBadge({ v }: { v: GameVerdict }) {
  const Icon = v === "recommended" ? CheckCircle2 : v === "minimum" ? CircleAlert : v === "below" ? CircleX : CircleHelp;
  return (
    <span className={`badge verdict v-${v}`}>
      <Icon size={13} /> {verdictText[v]}
    </span>
  );
}

const gb = (b: number) => (b >= 1e9 ? `${(b / 1e9).toFixed(1)} GB` : `${Math.round(b / 1e6)} MB`);

function Result({ c, onRefresh, busy }: { c: GameCheck; onRefresh: () => void; busy: boolean }) {
  return (
    <section className="card game-result">
      <div className="game-head">
        {c.image && <img src={c.image} alt="" className="game-art" />}
        <div className="game-head-text">
          <h2>{c.name}</h2>
          <div className="row-gap">
            <VerdictBadge v={c.overall} />
            {c.installed && <span className="badge">Installed</span>}
          </div>
          <p>{c.summary}</p>
          <div className="row-gap">
            <button className="btn ghost small" onClick={() => api.openPath(c.storeUrl)}>
              <ExternalLink size={13} /> Steam store page
            </button>
            <button className="btn ghost small" onClick={onRefresh} disabled={busy}>
              {busy ? <Loader2 size={13} className="spin" /> : <RefreshCw size={13} />} Check again
            </button>
            <span className="muted small">Requirements from Steam, {new Date(c.fetchedAt).toLocaleDateString()}</span>
          </div>
        </div>
      </div>
      <div className="game-table-wrap">
        <table className="simple-table game-table">
          <thead>
            <tr>
              <th></th>
              <th>This PC</th>
              <th>Minimum</th>
              <th>Recommended</th>
              <th>Result</th>
            </tr>
          </thead>
          <tbody>
            {c.parts.map((p) => (
              <tr key={p.part}>
                <th>{p.part}</th>
                <td>
                  {p.yours || "—"}
                  {p.note && <div className="muted small">{p.note}</div>}
                </td>
                <td>{p.minimum || "—"}</td>
                <td>{p.recommended || "—"}</td>
                <td>
                  <VerdictBadge v={p.verdict} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="muted small">Processors and graphics cards are compared with rough benchmark scores, so treat close calls as a guide. Memory, storage and Windows are compared exactly.</p>
    </section>
  );
}

/** Game check: will a game run on this PC? Installed Steam/Epic games plus any game on Steam. */
export function Games() {
  const [games, setGames] = useState<InstalledGame[] | null>(null);
  const [result, setResult] = useState<GameCheck | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [term, setTerm] = useState("");
  const [hits, setHits] = useState<SearchHit[] | null>(null);
  const [searching, setSearching] = useState(false);
  const [batch, setBatch] = useState<{ done: number; total: number } | null>(null);
  const stopBatch = useRef(false);
  const resultRef = useRef<HTMLDivElement>(null);

  const loadGames = () => api.gamesInstalled().then(setGames).catch((e) => setError(String(e)));
  useEffect(() => {
    loadGames();
  }, []);

  const setVerdict = (id: string, v: GameVerdict, appId?: number) =>
    setGames((gs) => gs?.map((g) => (g.id === id ? { ...g, lastVerdict: v, steamAppId: appId ?? g.steamAppId } : g)) ?? gs);

  /** Checks one game; Epic games are matched to Steam by name first. */
  const checkGame = async (g: InstalledGame, refresh = false, show = true): Promise<boolean> => {
    setBusyId(g.id);
    setError(null);
    try {
      let appId = g.steamAppId ?? null;
      if (!appId) {
        const hit = await api.gameFind(g.name);
        if (!hit) throw new Error(`Couldn't find "${g.name}" on Steam. DxTool reads requirements from Steam's store, so games sold only elsewhere can't be checked.`);
        appId = hit.appId;
      }
      const c = await api.gameCheck(appId, refresh);
      setVerdict(g.id, c.overall, appId);
      if (show) {
        setResult(c);
        setTimeout(() => resultRef.current?.scrollIntoView({ behavior: "smooth", block: "start" }), 50);
      }
      return true;
    } catch (e) {
      if (show) setError(String(e).replace(/^Error: /, ""));
      return false;
    } finally {
      setBusyId(null);
    }
  };

  const checkApp = async (appId: number, refresh = false) => {
    setBusyId(String(appId));
    setError(null);
    try {
      const c = await api.gameCheck(appId, refresh);
      setResult(c);
      setGames((gs) => gs?.map((g) => (g.steamAppId === appId ? { ...g, lastVerdict: c.overall } : g)) ?? gs);
      setTimeout(() => resultRef.current?.scrollIntoView({ behavior: "smooth", block: "start" }), 50);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusyId(null);
    }
  };

  const search = async () => {
    if (!term.trim()) return;
    setSearching(true);
    setError(null);
    try {
      setHits(await api.gameSearch(term));
    } catch (e) {
      setError(String(e));
    } finally {
      setSearching(false);
    }
  };

  const checkAll = async () => {
    if (!games) return;
    const todo = games.filter((g) => !g.lastVerdict);
    stopBatch.current = false;
    setBatch({ done: 0, total: todo.length });
    for (let i = 0; i < todo.length && !stopBatch.current; i++) {
      await checkGame(todo[i], false, false);
      setBatch({ done: i + 1, total: todo.length });
      // Be gentle with Steam's servers.
      await new Promise((r) => setTimeout(r, 1200));
    }
    setBatch(null);
  };

  const unchecked = games?.filter((g) => !g.lastVerdict).length ?? 0;

  return (
    <div className="page games">
      <div className="page-head">
        <div>
          <h1>
            <Gamepad2 size={22} /> Games
          </h1>
          <p className="muted">Will a game run on this PC? DxTool compares each game's minimum and recommended requirements from Steam with this PC's processor, graphics, memory, free space and Windows.</p>
        </div>
      </div>

      <section className="card">
        <div className="card-head">
          <h2>Check any game</h2>
        </div>
        <form
          className="game-search"
          onSubmit={(e) => {
            e.preventDefault();
            search();
          }}
        >
          <input value={term} onChange={(e) => setTerm(e.target.value)} placeholder="Game name, e.g. Cyberpunk 2077" />
          <button className="btn" type="submit" disabled={searching || !term.trim()}>
            {searching ? <Loader2 size={14} className="spin" /> : <Search size={14} />} Search Steam
          </button>
        </form>
        {hits && hits.length === 0 && <p className="muted">No games found on Steam with that name.</p>}
        {hits && hits.length > 0 && (
          <ul className="hit-list">
            {hits.slice(0, 8).map((h) => (
              <li key={h.appId}>
                {h.image ? <img src={h.image} alt="" /> : <span className="hit-noimg" />}
                <span className="hit-name">{h.name}</span>
                <button className="btn small" onClick={() => checkApp(h.appId)} disabled={!!busyId}>
                  {busyId === String(h.appId) ? <Loader2 size={13} className="spin" /> : null} Check
                </button>
              </li>
            ))}
          </ul>
        )}
        <p className="muted small">Searching and the first check of each game use the internet (Steam's public store). Results are saved, so checking again later works offline.</p>
      </section>

      {error && <div className="banner error">{error}</div>}

      <div ref={resultRef}>{result && <Result c={result} busy={busyId === String(result.appId)} onRefresh={() => checkApp(result.appId, true)} />}</div>

      <section className="card">
        <div className="card-head">
          <h2>Installed games {games ? <span className="muted small">({games.length})</span> : null}</h2>
          {games && games.length > 0 && (
            <div className="row-gap">
              {batch ? (
                <>
                  <span className="muted small">
                    Checking {batch.done} of {batch.total}…
                  </span>
                  <button className="btn ghost small" onClick={() => (stopBatch.current = true)}>
                    <Square size={12} /> Stop
                  </button>
                </>
              ) : (
                unchecked > 0 && (
                  <button className="btn ghost small" onClick={checkAll} disabled={!!busyId}>
                    Check all {unchecked} unchecked
                  </button>
                )
              )}
            </div>
          )}
        </div>
        {!games ? (
          <p className="muted">
            <Loader2 size={14} className="spin" /> Looking for games…
          </p>
        ) : games.length === 0 ? (
          <p className="muted">No Steam or Epic Games Launcher games found. Use the search above to check any game.</p>
        ) : (
          <div className="game-table-wrap">
            <table className="simple-table game-list">
              <thead>
                <tr>
                  <th>Game</th>
                  <th>From</th>
                  <th>Size</th>
                  <th>Result</th>
                  <th></th>
                </tr>
              </thead>
              <tbody>
                {games.map((g) => (
                  <tr key={`${g.launcher}-${g.id}`}>
                    <td>{g.name}</td>
                    <td className="muted">{g.launcher}</td>
                    <td className="muted">{g.sizeBytes ? gb(g.sizeBytes) : "—"}</td>
                    <td>{g.lastVerdict ? <VerdictBadge v={g.lastVerdict} /> : <span className="muted small">Not checked</span>}</td>
                    <td className="right">
                      <button className="btn small ghost" onClick={() => checkGame(g)} disabled={!!busyId || !!batch}>
                        {busyId === g.id ? <Loader2 size={13} className="spin" /> : null} {g.lastVerdict ? "View" : "Check"}
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </div>
  );
}

"""The plan as a page: `progress.py`'s numbers, drawn.

The terminal table answers "how much is left" in a form that has to be re-read
from the top every time it is redrawn. This answers it in a window instead: the
stage cards, the whole gate ledger and the code the repository owns, on one page,
with the two questions a reader actually has -- *what is open* and *what did that
gate say* -- a keystroke away.

Nothing here is fetched. The page is one HTML file with its styles and its script
in it, because a dashboard that needs a network is blank exactly when the network
is what is being worked on.

The four palettes are the launcher's own: the `LIGHT`/`DARK`/`OLED`/`RETRO`
columns of `theme_gen.rs`'s colour table, which `tools/gen_theme.py` transcribed
from the reference's stylesheets. The page and the thing the page is about are
painted from the same colours, and there is no fifth opinion about what the
launcher's green is.
"""

from __future__ import annotations

import html
import sys
import time
from pathlib import Path

# The package is a directory of scripts rather than an installed module: the
# tools are run the way `AGENTS.md` writes them, from the repository root.
sys.path.insert(0, str(Path(__file__).resolve().parent))

import progress  # noqa: E402  (the parsers, and the one place the totals are computed)

# `````````````````````````````````````````````````````````````````````````````
# The palettes: `Ink::` names as the launcher's table spells them, one column per
# theme. Repeating a hex here is deliberate -- the alternative is this file
# parsing Rust -- and the tests in `progress.py --check` cover the documents, not
# these colours, so the comment is the only thing keeping them honest.
# `````````````````````````````````````````````````````````````````````````````
PALETTES = {
    # name: (bg, raised, surface, border, ink, muted, brand, brand_soft, red, orange, blue)
    "light": ("#ebebeb", "#f8f8f8", "#ffffff", "#dddddd", "#1a202c", "#484d54",
              "#00af5c", "rgba(0, 175, 92, 0.14)", "#cb2245", "#e08325", "#1f68c0"),
    "dark": ("#16181c", "#27292e", "#34363c", "#42444a", "#ffffff", "#96a2b0",
             "#1bd96a", "rgba(27, 217, 106, 0.20)", "#ff496e", "#ffa347", "#4f9cff"),
    "oled": ("#000000", "#101013", "#1b1b20", "#25262b", "#ffffff", "#96a2b0",
             "#1bd96a", "rgba(27, 217, 106, 0.20)", "#ff496e", "#ffa347", "#4f9cff"),
    "retro": ("#191917", "#232421", "#3a3b38", "#5a5c58", "#e6e2d1", "#9b9e98",
              "#4d9227", "rgba(77, 146, 39, 0.28)", "#e8200d", "#e88d0d", "#099fef"),
}

THEME_ORDER = ("light", "dark", "oled", "retro")
DEFAULT_THEME = "dark"

CSS = """
*, *::before, *::after { box-sizing: border-box; }
:root {
  --bg: #16181c; --raised: #27292e; --surface: #34363c; --border: #42444a;
  --ink: #ffffff; --muted: #96a2b0; --brand: #1bd96a; --brand-soft: rgba(27,217,106,.20);
  --red: #ff496e; --orange: #ffa347; --blue: #4f9cff;
  --mono: ui-monospace, "Cascadia Mono", "SF Mono", Consolas, monospace;
  --sans: "Segoe UI", Inter, system-ui, -apple-system, sans-serif;
  --radius: 12px; --gap: 16px;
}
html[data-theme="light"] {
  --bg: #ebebeb; --raised: #f8f8f8; --surface: #ffffff; --border: #dddddd;
  --ink: #1a202c; --muted: #484d54; --brand: #00af5c; --brand-soft: rgba(0,175,92,.14);
  --red: #cb2245; --orange: #e08325; --blue: #1f68c0;
}
html[data-theme="oled"] { --bg: #000000; --raised: #101013; --surface: #1b1b20; --border: #25262b; }
html[data-theme="retro"] {
  --bg: #191917; --raised: #232421; --surface: #3a3b38; --border: #5a5c58;
  --ink: #e6e2d1; --muted: #9b9e98; --brand: #4d9227; --brand-soft: rgba(77,146,39,.28);
  --red: #e8200d; --orange: #e88d0d; --blue: #099fef;
}
body {
  margin: 0; padding: 0 0 48px; background: var(--bg); color: var(--ink);
  font: 14px/1.5 var(--sans); -webkit-font-smoothing: antialiased;
}
a { color: var(--brand); text-decoration: none; }
a:hover { text-decoration: underline; }
.wrap { max-width: 1180px; margin: 0 auto; padding: 0 24px; }

header.bar {
  position: sticky; top: 0; z-index: 5; background: var(--raised);
  border-bottom: 1px solid var(--border);
}
header.bar .wrap { display: flex; align-items: center; gap: 12px; height: 56px; }
.mark {
  width: 26px; height: 26px; border-radius: 8px; flex: 0 0 auto;
  background: linear-gradient(135deg, var(--brand), var(--brand-soft));
  box-shadow: inset 0 0 0 1px var(--border);
}
.wordmark { font-weight: 600; letter-spacing: .2px; }
.wordmark small { color: var(--muted); font-weight: 500; margin-left: 8px; }
header.bar .spacer { flex: 1; }
.stamp { color: var(--muted); font-size: 12px; font-variant-numeric: tabular-nums; }

button, .chip {
  font: inherit; color: var(--ink); background: var(--surface); cursor: pointer;
  border: 1px solid var(--border); border-radius: 999px; padding: 4px 11px;
}
button:hover, .chip:hover { border-color: var(--brand); }
button[aria-pressed="true"], .chip[aria-pressed="true"] {
  background: var(--brand-soft); border-color: var(--brand); color: var(--brand);
}
.themes { display: flex; gap: 4px; }
.themes button { padding: 3px 9px; font-size: 12px; }

.hero { display: grid; grid-template-columns: 200px 1fr; gap: var(--gap); margin: 24px 0; }
@media (max-width: 760px) { .hero { grid-template-columns: 1fr; } }
.card {
  background: var(--raised); border: 1px solid var(--border);
  border-radius: var(--radius); padding: 16px;
}
.ring { display: flex; flex-direction: column; align-items: center; gap: 8px; }
.ring svg { transform: rotate(-90deg); }
.ring .value { font-size: 30px; font-weight: 600; font-variant-numeric: tabular-nums; }
.ring .label { color: var(--muted); font-size: 12px; }
.metrics { display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: var(--gap); }
.metric .n { font-size: 24px; font-weight: 600; font-variant-numeric: tabular-nums; }
.metric .l { color: var(--muted); font-size: 12px; }
.metric .sub { color: var(--muted); font-size: 12px; margin-top: 4px; }

h2 { font-size: 15px; font-weight: 600; margin: 32px 0 12px; }
h2 .hint { color: var(--muted); font-weight: 400; font-size: 12px; margin-left: 8px; }

.stages {
  display: grid; grid-template-columns: repeat(auto-fit, minmax(330px, 1fr));
  gap: var(--gap); align-items: start;
}
.stage .head { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
.stage .num {
  font: 600 12px var(--mono); color: var(--muted); border: 1px solid var(--border);
  border-radius: 6px; padding: 1px 7px;
}
.pill {
  font-size: 11px; padding: 1px 8px; border-radius: 999px; white-space: nowrap;
  border: 1px solid var(--border); color: var(--muted);
}
.pill.done { color: var(--brand); border-color: var(--brand); background: var(--brand-soft); }
.stage .what { margin: 0 0 12px; font-size: 13px; }
.bar { height: 6px; border-radius: 999px; background: var(--surface); overflow: hidden; }
.bar > i { display: block; height: 100%; background: var(--brand); }
.bar-row { display: flex; align-items: center; gap: 10px; margin-bottom: 12px; }
.bar-row .bar { flex: 1; }
.bar-row .pct { font: 600 12px var(--mono); color: var(--muted); }
.gates { display: flex; flex-wrap: wrap; gap: 4px; margin-bottom: 10px; }
.gate-chip {
  font: 500 11px var(--mono); padding: 1px 7px; border-radius: 6px;
  border: 1px solid var(--border); color: var(--muted); background: var(--surface);
}
.gate-chip.met { color: var(--brand); border-color: var(--brand); }
.gate-chip:hover { border-color: var(--ink); color: var(--ink); cursor: pointer; }
.open-item { border-top: 1px solid var(--border); padding-top: 8px; margin-top: 8px; font-size: 12px; }
.open-item b { font-weight: 600; }
.open-item .why { color: var(--muted); }
.open-item .est { float: right; color: var(--muted); font: 500 11px var(--mono); }

.controls { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; margin-bottom: 12px; }
input[type="search"] {
  font: inherit; color: var(--ink); background: var(--raised); flex: 1 1 260px;
  border: 1px solid var(--border); border-radius: 999px; padding: 6px 12px;
}
input[type="search"]:focus { outline: none; border-color: var(--brand); }
.controls .count { color: var(--muted); font-size: 12px; font-variant-numeric: tabular-nums; }

.ledger { display: flex; flex-direction: column; gap: 6px; }
.gate {
  display: grid; grid-template-columns: 34px 62px 1fr auto; gap: 10px; align-items: baseline;
  background: var(--raised); border: 1px solid var(--border); border-radius: 10px; padding: 8px 12px;
}
.gate .box { font: 600 12px var(--mono); color: var(--muted); }
.gate.met .box { color: var(--brand); }
.gate .id { font: 600 12px var(--mono); color: var(--ink); }
.gate .subject { font-size: 13px; }
.gate .stage-tag { font: 500 11px var(--mono); color: var(--muted); }
.gate[hidden] { display: none; }
.empty { color: var(--muted); font-size: 13px; }

.code .row { display: grid; grid-template-columns: 190px 1fr 90px; gap: 12px; align-items: center; padding: 4px 0; }
.code .where { font: 500 12px var(--mono); }
.code .track { height: 8px; border-radius: 999px; background: var(--surface); overflow: hidden; display: flex; }
.code .track .hand { background: var(--brand); }
.code .track .gen { background: var(--brand-soft); }
.code .n { text-align: right; font: 500 12px var(--mono); color: var(--muted); font-variant-numeric: tabular-nums; }
.code .legend { color: var(--muted); font-size: 12px; margin-bottom: 8px; }
.code .legend i { display: inline-block; width: 10px; height: 10px; border-radius: 3px; margin-right: 6px; }
footer { color: var(--muted); font-size: 12px; margin-top: 32px; border-top: 1px solid var(--border); padding-top: 12px; }
kbd {
  font: 500 11px var(--mono); border: 1px solid var(--border); border-bottom-width: 2px;
  border-radius: 5px; padding: 0 5px; color: var(--muted);
}
"""

JS = """
const $ = (sel, root = document) => root.querySelector(sel);
const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];

// Themes, remembered: the page is left open for hours at a time.
const html = document.documentElement;
const wanted = localStorage.getItem('palantir-theme');
if (wanted) html.dataset.theme = wanted;
for (const button of $$('.themes button')) {
  button.setAttribute('aria-pressed', String(button.dataset.theme === html.dataset.theme));
  button.addEventListener('click', () => {
    html.dataset.theme = button.dataset.theme;
    localStorage.setItem('palantir-theme', button.dataset.theme);
    for (const other of $$('.themes button')) {
      other.setAttribute('aria-pressed', String(other === button));
    }
  });
}

// The ledger's filters. State is: a search string, a status, a stage.
const search = $('#search');
const gates = $$('.gate');
const count = $('#count');
const state = { text: '', status: 'all', stage: 'all' };

function apply() {
  let shown = 0;
  for (const gate of gates) {
    const haystack = gate.dataset.text;
    const ok = (!state.text || haystack.includes(state.text))
      && (state.status === 'all' || gate.dataset.status === state.status)
      && (state.stage === 'all' || gate.dataset.stage === state.stage);
    gate.hidden = !ok;
    if (ok) shown++;
  }
  count.textContent = `${shown} of ${gates.length} gates`;
  $('#none').hidden = shown !== 0;
}

for (const chip of $$('.controls .chip')) {
  chip.addEventListener('click', () => {
    state[chip.dataset.filter] = chip.dataset.value;
    for (const other of $$(`.controls .chip[data-filter="${chip.dataset.filter}"]`)) {
      other.setAttribute('aria-pressed', String(other === chip));
    }
    apply();
  });
}
search.addEventListener('input', () => { state.text = search.value.trim().toLowerCase(); apply(); });

// A gate chip on a stage card filters the ledger to that gate: the card is the
// summary and the ledger is the evidence, and this is the shortest path between
// them.
for (const chip of $$('.gate-chip')) {
  chip.addEventListener('click', () => {
    search.value = chip.dataset.id;
    state.text = chip.dataset.id.toLowerCase();
    state.status = 'all';
    state.stage = 'all';
    for (const other of $$('.controls .chip')) {
      other.setAttribute('aria-pressed', String(other.dataset.value === 'all'));
    }
    apply();
    $('#ledger').scrollIntoView({ behavior: 'smooth', block: 'start' });
  });
}

// `/` is the search box, Escape clears it: two keys, and no menu to learn.
document.addEventListener('keydown', (event) => {
  if (event.key === '/' && document.activeElement !== search) {
    event.preventDefault();
    search.focus();
  } else if (event.key === 'Escape') {
    search.value = '';
    state.text = '';
    apply();
    search.blur();
  } else if (/^[0-5]$/.test(event.key) && document.activeElement !== search) {
    state.stage = event.key;
    for (const chip of $$('.controls .chip[data-filter="stage"]')) {
      chip.setAttribute('aria-pressed', String(chip.dataset.value === event.key));
    }
    apply();
    $('#ledger').scrollIntoView({ behavior: 'smooth', block: 'start' });
  }
});
apply();
"""


def _ring(percent: float, size: int = 132) -> str:
    """The overall number as a ring, drawn rather than fetched."""
    stroke = 10
    radius = (size - stroke) / 2
    circumference = 2 * 3.141592653589793 * radius
    filled = circumference * max(0.0, min(100.0, percent)) / 100.0
    return f"""<div class="ring">
  <svg width="{size}" height="{size}" viewBox="0 0 {size} {size}" role="img"
       aria-label="{percent:g}% of the plan">
    <circle cx="{size / 2}" cy="{size / 2}" r="{radius}" fill="none"
            stroke="var(--surface)" stroke-width="{stroke}"/>
    <circle cx="{size / 2}" cy="{size / 2}" r="{radius}" fill="none"
            stroke="var(--brand)" stroke-width="{stroke}" stroke-linecap="round"
            stroke-dasharray="{filled:.1f} {circumference:.1f}"/>
  </svg>
  <div class="value">{percent:g}%</div>
</div>"""


def _stage_cards(
    stages: list[tuple[int, str, str]],
    sized: dict[int, tuple[int, int]],
    percents: dict[int, float],
    ledger: list[tuple[str, bool, str]],
    open_items: dict[int, list[str]],
) -> str:
    by_stage: dict[int, list[tuple[str, bool]]] = {}
    for gate_id, met, _ in ledger:
        owner = progress.owner_of(gate_id)
        if owner is None:
            continue
        by_stage.setdefault(int(owner) if owner != "before" else -1, []).append((gate_id, met))
    cards = []
    for number, what, state in stages:
        met, open_estimate = sized[number]
        gate_chips = "".join(
            f'<button class="gate-chip{" met" if is_met else ""}" '
            f'data-id="{html.escape(gate_id)}" title="{gate_id}">'
            f'{html.escape(gate_id)}</button>'
            for gate_id, is_met in by_stage.get(number, [])
        )
        lines = []
        for name in open_items.get(number, []):
            entry = next(
                (item for item in progress.OPEN.get(number, ()) if item[0] == name),
                (name, 0, ""),
            )
            lines.append(
                f'<div class="open-item"><span class="est">{entry[1]} gate(s)</span>'
                f"<b>{html.escape(name)}</b><br>"
                f'<span class="why">{html.escape(entry[2])}</span></div>'
            )
        cards.append(
            f"""<section class="card stage">
  <div class="head"><span class="num">stage {number}</span>
    <span class="pill{' done' if state == 'done' else ''}">"""
            f"""{'Done' if state == 'done' else 'In progress'}</span></div>
  <p class="what">{html.escape(what)}</p>
  <div class="bar-row">
    <div class="bar"><i style="width:{percents.get(number, 0):.1f}%"></i></div>
    <span class="pct">{percents.get(number, 0):.0f}%</span>
  </div>
  <div class="gates">{gate_chips or '<span class="why">no gates of its own</span>'}</div>
  {"".join(lines)}
</section>"""
        )
    return "".join(cards)


def _ledger(ledger: list[tuple[str, bool, str]]) -> str:
    rows = []
    for gate_id, met, subject in ledger:
        owner = progress.owner_of(gate_id)
        stage = "before this rewrite" if owner == "before" else f"stage {owner}"
        rows.append(
            f'<div class="gate{" met" if met else ""}" data-status="{"met" if met else "open"}" '
            f'data-stage="{html.escape(str(owner))}" '
            f'data-text="{html.escape((gate_id + " " + subject + " " + stage).lower())}">'
            f'<span class="box">{"[x]" if met else "[ ]"}</span>'
            f'<span class="id">{html.escape(gate_id)}</span>'
            f'<span class="subject">{html.escape(subject)}</span>'
            f'<span class="stage-tag">{html.escape(stage)}</span>'
            f"</div>"
        )
    return "".join(rows)


def _code(root: Path) -> tuple[str, str]:
    rows, (hand_lines, hand_code, gen_lines, gen_code) = progress.code_size(root)
    widest = max((lines for _, _, lines, _ in rows), default=1)
    body = []
    for name, note, lines, code in rows:
        generated = sum(1 for ch in note if ch.isdigit())
        gen = int(note.split()[0].replace(",", "")) if note and generated else 0
        hand = lines - gen
        body.append(
            f"""<div class="row">
  <span class="where">{html.escape(name)}</span>
  <span class="track" title="{lines:,} lines, {code:,} of them statements">
    <span class="hand" style="width:{100 * hand / widest:.2f}%"></span>
    <span class="gen" style="width:{100 * gen / widest:.2f}%"></span>
  </span>
  <span class="n">{lines:,}</span>
</div>"""
        )
    total = f"""<div class="row">
  <span class="where">hand-written total</span>
  <span class="track"><span class="hand" style="width:{100 * hand_lines / widest:.2f}%"></span></span>
  <span class="n">{hand_lines:,}</span>
</div>"""
    summary = (
        f"{hand_lines:,} lines written by hand ({hand_code:,} of them statements), "
        f"{gen_lines:,} of it generated ({gen_code:,} statements)"
    )
    return "".join(body) + total, summary


def render(root: Path) -> tuple[str, list[str]]:
    """The page, and the disagreements that stop it being written."""
    _, problems = progress.report(root)
    if problems:
        return "", problems
    next_steps = (root / "NEXT_STEPS.md").read_text(encoding="utf-8")
    gates_md = (root / "GATES.md").read_text(encoding="utf-8")
    stages = progress.parse_stages(next_steps)
    open_items = progress.parse_open_items(next_steps)
    ledger = progress.parse_ledger(gates_md)

    met_by_stage: dict[str, int] = {}
    sized: dict[int, tuple[int, int]] = {}
    unmet_by_stage: dict[int, list[str]] = {}
    for gate_id, met, _ in ledger:
        owner = progress.owner_of(gate_id)
        if owner is None:
            continue
        if met:
            met_by_stage[owner] = met_by_stage.get(owner, 0) + 1
        else:
            unmet_by_stage.setdefault(int(owner) if owner != "before" else -1, []).append(gate_id)
    for number, _, _ in stages:
        met = met_by_stage.get(number, 0)
        sized[number] = (met, sum(g for _, g, _ in progress.OPEN.get(number, ())) + len(unmet_by_stage.get(number, ())))
    totals = progress.plan_totals(stages, sized, met_by_stage)
    percents = totals["percents"]
    ledger_html = _ledger(ledger)
    code_html, code_summary = _code(root)
    stale = sum(1 for name in ("NEXT_STEPS.md", "GATES.md") if not (root / name).exists())
    stamps = ", ".join(
        f"{name} {time.strftime('%Y-%m-%d %H:%M', time.localtime((root / name).stat().st_mtime))}"
        for name in ("NEXT_STEPS.md", "GATES.md")
        if (root / name).exists()
    )
    stages_html = _stage_cards(stages, sized, percents, ledger, open_items)
    themes_html = "".join(
        f'<button data-theme="{name}">{name}</button>' for name in THEME_ORDER
    )
    status_chips = "".join(
        f'<button class="chip" data-filter="status" data-value="{value}" aria-pressed="false">{label}</button>'
        for value, label in (("all", "all"), ("open", "open"), ("met", "met"))
    )
    stage_chips = "".join(
        f'<button class="chip" data-filter="stage" data-value="{number}" aria-pressed="false">{number}</button>'
        for number, _, _ in stages
    ) + f'<button class="chip" data-filter="stage" data-value="before" aria-pressed="false">shell</button>'

    page = f"""<!DOCTYPE html>
<html lang="en" data-theme="{DEFAULT_THEME}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>PalantirMC — the plan</title>
<style>{CSS}</style>
</head>
<body>
<header class="bar"><div class="wrap">
  <span class="mark" aria-hidden="true"></span>
  <span class="wordmark">PalantirMC <small>the plan, and where each stage stands</small></span>
  <span class="spacer"></span>
  <span class="stamp">{html.escape(stamps)}</span>
  <span class="themes" role="group" aria-label="Theme">{themes_html}</span>
</div></header>

<main class="wrap">
  <section class="hero">
    <div class="card ring">{_ring(float(totals['overall']))}<span class="label">of the plan, weighted by stage</span></div>
    <div class="card">
      <div class="metrics">
        <div class="metric"><div class="n">{totals['met_total']}</div>
          <div class="l">slices met in stages 0-{stages[-1][0]}</div>
          <div class="sub">of {totals['slice_total']} there are</div></div>
        <div class="metric"><div class="n">{totals['slice_total'] - totals['met_total']}</div>
          <div class="l">slices still open</div>
          <div class="sub">{html.escape(code_summary)}</div></div>
        <div class="metric"><div class="n">{totals['met_before']}</div>
          <div class="l">gates before this rewrite</div>
          <div class="sub">the shell that is being replaced, still in the ledger</div></div>
        <div class="metric"><div class="n">{sum(1 for _, met, _ in ledger if not met)}</div>
          <div class="l">unmet gates</div>
          <div class="sub">{'every gate in the ledger is met' if all(met for _, met, _ in ledger) else 'listed below, open first'}</div></div>
      </div>
    </div>
  </section>

  <h2>Stages<span class="hint">the plan's own table, in full</span></h2>
  <div class="stages">{stages_html}</div>

  <h2 id="ledger">The ledger<span class="hint">every gate, with what it was for</span></h2>
  <div class="controls">
    <input id="search" type="search" placeholder="search the gates&hellip;  ( / )" aria-label="Search gates">
    {status_chips}
    <span class="spacer"></span>
    <button class="chip" data-filter="stage" data-value="all" aria-pressed="true">all stages</button>
    {stage_chips}
    <span class="count" id="count"></span>
  </div>
  <div class="ledger" id="gate-list">{ledger_html}</div>
  <p class="empty" id="none" hidden>nothing matches that.</p>

  <h2>What the repository owns<span class="hint">vendor/ and dist/ are not ours</span></h2>
  <div class="card code">
    <div class="legend">
      <span><i style="background:var(--brand)"></i>hand-written</span>
      <span style="margin-left:12px"><i style="background:var(--brand-soft)"></i>generated</span>
    </div>
    {code_html}
  </div>

  <footer>
    Generated {time.strftime('%Y-%m-%d %H:%M')} by <code>tools/progress.py --dashboard</code>
    from <code>NEXT_STEPS.md</code> and <code>GATES.md</code>.
    {'<b style="color:var(--red)">One of those documents is missing.</b>' if stale else ''}
    Keys: <kbd>/</kbd> search, <kbd>Esc</kbd> clear, <kbd>0</kbd>&ndash;<kbd>{stages[-1][0]}</kbd> a stage.
  </footer>
</main>
<script>{JS}</script>
</body>
</html>
"""
    return page, []


def check(root: Path) -> list[str]:
    """What the page must carry, so that it cannot quietly stop carrying it.

    A page is easy to break invisibly: a formatting change that drops a stage,
    an escape that eats a gate's subject, a template that stops being fed. The
    three claims below are the ones a reader relies on -- every gate is on the
    page, every stage has a card, and the headline number is the one the
    documents add up to -- and they are cheap to state.
    """
    page, problems = render(root)
    if problems:
        return problems
    next_steps = (root / "NEXT_STEPS.md").read_text(encoding="utf-8")
    gates_md = (root / "GATES.md").read_text(encoding="utf-8")
    ledger = progress.parse_ledger(gates_md)
    stages = progress.parse_stages(next_steps)
    found: list[str] = []
    missing = [gate for gate, _, _ in ledger if f'>{gate}</span>' not in page]
    if missing:
        found.append(f"the page does not carry these gates: {', '.join(missing)}")
    for number, _, _ in stages:
        if f'<span class="num">stage {number}</span>' not in page:
            found.append(f"the page has no card for stage {number}")
    # `data-stage` once per row: counting `class="gate"` would count the stage
    # cards' gate *chips* as well, which is how this check first failed.
    rows = page.count('data-stage="')
    if rows != len(ledger):
        found.append(f"the ledger is {len(ledger)} gates and the page draws {rows} rows")
    subjects = [subject for _, _, subject in ledger]
    eaten = [subject for subject in subjects if html.escape(subject) not in page]
    if eaten:
        found.append(f"a subject was mangled on the way into the page: {eaten[0]!r}")
    return found


def main() -> int:
    import argparse

    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", default=str(progress.REPO), help="the tree to read")
    parser.add_argument("--out", default=str(progress.REPO / ".scratch" / "progress.html"),
                        help="where the page is written (default: .scratch/progress.html)")
    parser.add_argument("--check", action="store_true",
                        help="write nothing; fail when the page would not carry the plan")
    args = parser.parse_args()
    root = Path(args.root)
    if args.check:
        problems = check(root)
        if problems:
            for problem in problems:
                print(f"dashboard: {problem}", file=sys.stderr)
            return 1
        ledger = progress.parse_ledger((root / "GATES.md").read_text(encoding="utf-8"))
        stages = progress.parse_stages((root / "NEXT_STEPS.md").read_text(encoding="utf-8"))
        print(
            f"CONFIRMED: the page carries all {len(ledger)} gates, "
            f"{len(stages)} stage cards and every subject as written"
        )
        return 0
    page, problems = render(root)
    if problems:
        for problem in problems:
            print(f"progress: {problem}", file=sys.stderr)
        return 1
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(page, encoding="utf-8")
    print(f"wrote {out}  ({len(page):,} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())

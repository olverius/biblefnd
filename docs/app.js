// biblefnd web app
//   Today   : the day's Mass readings for Denmark, in the English Catholic
//             lectionary text (Jerusalem Bible + Grail psalms) from Universalis
//   Look up : any reference in the World English Bible (modern English,
//             includes the deuterocanonical books), e.g. "luke 3:15"
//   Random  : a random reading from the Mass of a nearby day, same text as Today
"use strict";

const BIBLE_API = "https://bible-api.com";
// The Jerusalem Bible used at Mass can't be looked up freely, so look-ups use
// the World English Bible: public domain, modern English, all Catholic books.
const LOOKUP_TRANSLATION = { id: "web", short: "WEB" };

// Universalis calendar: Denmark, so Danish feasts (e.g. St Ansgar) are included.
const UNIVERSALIS_REGION = "europe.denmark";

// Universalis' free feed only serves days close to today (observed: about
// 3 days back to a week ahead; further away it refuses with HTTP 403).
const WINDOW_BACK = 3;
const WINDOW_AHEAD = 7;

// Labels for the readings Universalis sends. Anything not listed gets a
// label derived from its key, so special days (Easter Vigil etc.) still show.
const LABELS = {
  Mass_R1: "First Reading",
  Mass_Ps: "Responsorial Psalm",
  Mass_R2: "Second Reading",
  Mass_GA: "Gospel Acclamation",
  Mass_G: "Gospel",
};

const $ = (sel) => document.querySelector(sel);

// ------------------------------------------------------------------ helpers

/** Decode HTML entities and strip tags, e.g. "<h4>Sunday</h4>&#x2010;" -> "Sunday‐" */
function htmlToText(s) {
  const doc = new DOMParser().parseFromString(s || "", "text/html");
  return (doc.body.textContent || "").replace(/\s+/g, " ").trim();
}

/** The API text has stray line breaks inside verses. */
const clean = (s) => (s || "").replace(/\s+/g, " ").trim();

function localDateKey(d = new Date()) {
  const p = (n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

function shiftDays(days) {
  const now = new Date();
  return new Date(now.getFullYear(), now.getMonth(), now.getDate() + days);
}

function inWindow(dateKey) {
  return dateKey >= localDateKey(shiftDays(-WINDOW_BACK)) && dateKey <= localDateKey(shiftDays(WINDOW_AHEAD));
}

const OUT_OF_WINDOW = `Universalis only provides the readings from ${WINDOW_BACK} days ago to ${WINDOW_AHEAD} days ahead. Pick a date in that range.`;

function el(tag, attrs = {}, ...children) {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === "class") node.className = v;
    else node.setAttribute(k, v);
  }
  for (const c of children) {
    if (c == null) continue;
    node.append(c instanceof Node ? c : document.createTextNode(String(c)));
  }
  return node;
}

function skeleton() {
  const f = document.createDocumentFragment();
  f.append(el("div", { class: "skeleton short" }), el("div", { class: "skeleton" }), el("div", { class: "skeleton" }));
  return f;
}

/** "Mass_R3" -> "Reading 3", "Mass_Ps2" -> "Psalm" — fallback labels */
function labelFor(key) {
  if (LABELS[key]) return LABELS[key];
  const k = key.replace(/^Mass_/, "");
  if (/^R\d+/.test(k)) return `Reading ${k.slice(1)}`;
  if (/^Ps/.test(k)) return "Psalm";
  if (/^GA/.test(k)) return "Gospel Acclamation";
  if (/^G/.test(k)) return "Gospel";
  return k;
}

// Inline formatting allowed through from the Universalis text; everything
// else is reduced to its text. Attributes are always dropped.
const INLINE = new Set(["I", "EM", "B", "STRONG", "SUP", "SUB", "SMALL", "U"]);
const BLOCK = new Set(["P", "DIV", "BLOCKQUOTE", "H1", "H2", "H3", "H4", "H5", "H6", "LI"]);

/**
 * Turn a Universalis reading text into safe DOM nodes.
 * Handles both HTML (<p>, <br>, <i>…) and plain text with blank-line
 * paragraphs and *italic* markers (used for the psalm response).
 */
function readingBody(text) {
  const box = el("div", { class: "lectionary" });
  const src = (text || "").trim();

  if (/<[a-z][\s\S]*>/i.test(src)) {
    const doc = new DOMParser().parseFromString(src, "text/html");
    let para = null;
    const newPara = () => { para = el("p"); box.append(para); return para; };

    const walk = (node, into) => {
      for (const child of node.childNodes) {
        if (child.nodeType === Node.TEXT_NODE) {
          if (child.textContent.trim() || (into !== box && child.textContent)) {
            (into === box ? (para || newPara()) : into).append(child.textContent);
          }
        } else if (child.nodeType === Node.ELEMENT_NODE) {
          const tag = child.tagName;
          if (tag === "BR") {
            (into === box ? (para || newPara()) : into).append(el("br"));
          } else if (BLOCK.has(tag) && into === box) {
            para = null;
            walk(child, box);
            para = null;
          } else if (INLINE.has(tag)) {
            const wrap = el(tag.toLowerCase());
            (into === box ? (para || newPara()) : into).append(wrap);
            walk(child, wrap);
          } else if (tag !== "SCRIPT" && tag !== "STYLE") {
            walk(child, into);
          }
        }
      }
    };
    walk(doc.body, box);
    // drop empty paragraphs
    for (const p of box.querySelectorAll("p")) if (!p.textContent.trim()) p.remove();
  } else {
    for (const block of src.split(/\n\s*\n/)) {
      if (!block.trim()) continue;
      const p = el("p");
      block.trim().split("\n").forEach((line, i) => {
        if (i) p.append(el("br"));
        // *response* -> <i>response</i>
        line.split(/(\*[^*]+\*)/).forEach((part) => {
          if (/^\*[^*]+\*$/.test(part)) p.append(el("i", {}, part.slice(1, -1)));
          else if (part) p.append(part);
        });
      });
      box.append(p);
    }
  }
  return box;
}

// ------------------------------------------------------------------ fetching

async function getJson(url) {
  let resp;
  try {
    resp = await fetch(url);
  } catch {
    throw new Error(navigator.onLine ? "Couldn't reach the server." : "You're offline.");
  }
  if (resp.status === 404) throw new Error("Not found — check the book name and numbers.");
  if (resp.status === 429) throw new Error("Too many requests — wait a moment and try again.");
  if (!resp.ok) throw new Error(`Server returned HTTP ${resp.status}.`);
  return resp.json();
}

/** { reference, verses: [[n, text], ...] } */
async function fetchReference(ref) {
  const q = ref.trim().replace(/\s+/g, "+");
  if (!q) throw new Error("Type a reference, e.g. luke 3:15");
  const json = await getJson(`${BIBLE_API}/${encodeURI(q)}?translation=${LOOKUP_TRANSLATION.id}`);
  if (json.error) throw new Error(json.error);
  const verses = (json.verses || []).map((v) => [v.verse, clean(v.text)]);
  if (!verses.length) throw new Error("Not found.");
  return { reference: json.reference || ref, verses };
}

/** A random reading from the Mass of a random day within the Universalis window. */
async function fetchRandomReading() {
  const offset = Math.floor(Math.random() * (WINDOW_BACK + WINDOW_AHEAD + 1)) - WINDOW_BACK;
  const dateKey = localDateKey(shiftDays(offset));
  const data = await fetchMassReadings(dateKey);
  // readings and Gospels with real text; skip the short Gospel acclamation
  const keys = Object.keys(data).filter((k) => /^Mass_/.test(k) && !/^Mass_GA/.test(k) && data[k] && data[k].text);
  if (!keys.length) throw new Error("No readings found for the chosen day — try again.");
  const key = keys[Math.floor(Math.random() * keys.length)];
  return { key, reading: data[key], data, dateKey };
}

// Universalis only offers JSONP (a <script> that calls universalisCallback),
// so requests are queued one at a time to keep callbacks from crossing.
let jsonpQueue = Promise.resolve();

function loadUniversalis(ymd, attempt) {
  return new Promise((resolve, reject) => {
    const script = document.createElement("script");
    let finished = false;
    let afterLoad;
    const finish = (fn, arg) => {
      if (finished) return;
      finished = true;
      clearTimeout(timer);
      clearTimeout(afterLoad);
      delete window.universalisCallback;
      script.remove();
      fn(arg);
    };
    const timer = setTimeout(
      () => finish(reject, new Error("Universalis didn't answer in time (slow connection?).")),
      20000
    );
    window.universalisCallback = (data) => finish(resolve, data);
    // The script downloaded but never called us back: something emptied or blocked it.
    script.onload = () => {
      afterLoad = setTimeout(
        () => finish(reject, new Error("The readings were downloaded but blocked from running — an ad/content blocker in this browser may be stopping universalis.com.")),
        1500
      );
    };
    script.onerror = () =>
      finish(reject, new Error(navigator.onLine
        ? "Couldn't load the readings from universalis.com (blocked or unreachable)."
        : "You're offline."));
    // cache-buster on retries so a bad cached copy isn't reused
    script.src = `https://universalis.com/${UNIVERSALIS_REGION}/${ymd}/jsonpmass.js${attempt ? `?r=${Date.now()}` : ""}`;
    document.head.append(script);
  });
}

function fetchMassReadings(dateKey) {
  const ymd = dateKey.replaceAll("-", "");
  const job = () => loadUniversalis(ymd, 0).catch(() => loadUniversalis(ymd, 1)); // one retry
  const p = jsonpQueue.then(job, job);
  jsonpQueue = p.catch(() => {});
  return p;
}

// ------------------------------------------------------------------ rendering

/** Look-up passage from bible-api.com */
function renderPassage(p) {
  const wrap = el("article", { class: "reading" });
  wrap.append(el("h2", { class: "passage-ref" }, p.reference, el("span", { class: "tr" }, LOOKUP_TRANSLATION.short)));
  const body = el("p", { class: "passage" });
  const numbered = p.verses.length > 1;
  p.verses.forEach(([n, text], i) => {
    if (numbered) body.append(el("sup", {}, n));
    body.append(text + (i < p.verses.length - 1 ? " " : ""));
  });
  wrap.append(body);
  return wrap;
}

/** One reading from the Universalis feed */
function renderLectionary(key, r) {
  const wrap = el("article", { class: "reading" });
  wrap.append(el("p", { class: "reading-label" }, labelFor(key)));
  // "Heb4:12" -> "Heb 4:12"
  const source = htmlToText(r.source).replace(/^(\d?\s?[A-Za-z]+)(\d)/, "$1 $2");
  if (source) wrap.append(el("h2", { class: "passage-ref" }, source));
  const heading = htmlToText(r.heading);
  if (heading) wrap.append(el("p", { class: "reading-heading" }, heading));
  wrap.append(readingBody(r.text));
  return wrap;
}

/** Universalis' terms: their copyright notice must be visible, with a link to them. */
function universalisCredit(data, dateKey) {
  return [
    el("p", {}, htmlToText(data.copyright)),
    el("p", {}, "Readings provided by ",
      el("a", { href: `https://universalis.com/${UNIVERSALIS_REGION}/${dateKey.replaceAll("-", "")}/mass.htm`, rel: "noopener" }, "Universalis"),
      "."),
  ];
}

/** "Friday of week 27 ... or Saint Denis ..." -> ["Friday of week 27 ...", ["Saint Denis ..."]] */
function splitDay(data) {
  const [main, ...optional] = htmlToText(data.day).split(/ or (?=Saint|Saints|Blessed|Our Lady|The )/);
  return [main || htmlToText(data.date), optional];
}

function renderError(target, err) {
  target.replaceChildren(el("p", { class: "error" }, err.message || String(err)));
}

// ------------------------------------------------------------------ Today

let todayToken = 0;
let shownDate = null; // the date whose readings are on screen (or loading)

async function showReadings(dateKey) {
  const token = ++todayToken;
  shownDate = dateKey;
  const title = $("#day-title");
  const alt = $("#day-alt");
  const out = $("#readings");
  const credit = $("#readings-credit");

  $("#date").value = dateKey;
  $("#today-btn").hidden = dateKey === localDateKey();
  title.textContent = "";
  alt.textContent = "";
  credit.replaceChildren();
  out.replaceChildren(skeleton());

  if (!inWindow(dateKey)) {
    out.replaceChildren(el("p", { class: "note" }, OUT_OF_WINDOW));
    return;
  }

  let data;
  try {
    data = await fetchMassReadings(dateKey);
  } catch (e) {
    if (token === todayToken) renderError(out, e);
    return;
  }
  if (token !== todayToken) return;

  const [main, optional] = splitDay(data);
  title.textContent = main;
  alt.textContent = optional.length ? `Optional memorial${optional.length > 1 ? "s" : ""}: ${optional.join("; ")}` : "";
  document.title = `${main} · biblefnd`;

  const keys = Object.keys(data).filter((k) => /^Mass_/.test(k) && data[k] && (data[k].text || data[k].source));
  if (!keys.length) {
    out.replaceChildren(el("p", { class: "note" }, "No readings were found for this day."));
  } else {
    out.replaceChildren(...keys.map((k) => renderLectionary(k, data[k])));
  }

  credit.replaceChildren(...universalisCredit(data, dateKey));
}

// ------------------------------------------------------------------ Look up

let lastLookup = null;

async function showLookup(ref) {
  const out = $("#lookup-result");
  lastLookup = ref;
  $("#ref").value = ref;
  if (!ref.trim()) { out.replaceChildren(); return; }
  out.replaceChildren(skeleton());
  try {
    out.replaceChildren(renderPassage(await fetchReference(ref)));
    document.title = `${ref} · biblefnd`;
  } catch (e) {
    renderError(out, e);
  }
}

// ------------------------------------------------------------------ Random

async function showRandom() {
  const out = $("#random-result");
  const credit = $("#random-credit");
  const btn = $("#random-btn");
  btn.disabled = true;
  credit.replaceChildren();
  out.replaceChildren(skeleton());
  try {
    const { key, reading, data, dateKey } = await fetchRandomReading();
    const [main] = splitDay(data);
    out.replaceChildren(
      renderLectionary(key, reading),
      el("p", { class: "from-mass" }, "From the Mass of ",
        el("a", { href: `#today/${dateKey}` }, main), ` · ${htmlToText(data.date)}`)
    );
    credit.replaceChildren(...universalisCredit(data, dateKey));
  } catch (e) {
    renderError(out, e);
  } finally {
    btn.disabled = false;
  }
}

// ------------------------------------------------------------------ routing
//   #today            today's readings
//   #today/2026-10-11 readings for a date
//   #lookup/luke 3:15 a reference (shareable link)
//   #random

let randomLoaded = false;

function route() {
  const hash = decodeURIComponent(location.hash.slice(1));
  const [tab, ...rest] = hash.split("/");
  const arg = rest.join("/");
  const view = ["today", "lookup", "random"].includes(tab) ? tab : "today";

  for (const v of document.querySelectorAll(".view")) v.hidden = v.id !== `view-${view}`;
  for (const a of document.querySelectorAll(".tabs a")) {
    if (a.dataset.tab === view) a.setAttribute("aria-current", "page");
    else a.removeAttribute("aria-current");
  }

  if (view === "today") {
    const dateKey = /^\d{4}-\d{2}-\d{2}$/.test(arg) ? arg : localDateKey();
    if (dateKey !== shownDate) showReadings(dateKey);
  } else if (view === "lookup") {
    if (arg && arg !== lastLookup) showLookup(arg);
    else if (!arg) $("#ref").focus();
  } else if (view === "random" && !randomLoaded) {
    randomLoaded = true;
    showRandom();
  }
}

// keep the date picker inside the range Universalis serves
$("#date").min = localDateKey(shiftDays(-WINDOW_BACK));
$("#date").max = localDateKey(shiftDays(WINDOW_AHEAD));

$("#date").addEventListener("change", (e) => {
  if (e.target.value) location.hash = `today/${e.target.value}`;
});
$("#today-btn").addEventListener("click", () => { location.hash = "today"; });
$("#lookup-form").addEventListener("submit", (e) => {
  e.preventDefault();
  const ref = $("#ref").value.trim();
  if (!ref) return;
  $("#ref").blur(); // close the phone keyboard
  if (decodeURIComponent(location.hash) === `#lookup/${ref}`) showLookup(ref); // same ref again: refetch
  else location.hash = `lookup/${ref}`;
});
$("#random-btn").addEventListener("click", showRandom);

// When the app is reopened on a new day, move "today" forward.
document.addEventListener("visibilitychange", () => {
  if (document.visibilityState !== "visible") return;
  $("#date").min = localDateKey(shiftDays(-WINDOW_BACK));
  $("#date").max = localDateKey(shiftDays(WINDOW_AHEAD));
  if ((location.hash === "" || location.hash === "#today") && shownDate !== localDateKey()) {
    showReadings(localDateKey());
  }
});

window.addEventListener("hashchange", route);
route();

// offline support / installability
if ("serviceWorker" in navigator) {
  window.addEventListener("load", () => navigator.serviceWorker.register("sw.js").catch(() => {}));
}

// exposed for the in-browser tests
window.biblefnd = { readingBody, htmlToText, labelFor };

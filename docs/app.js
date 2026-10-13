// biblefnd web app — same features as the CLI:
//   Today   : the day's Mass readings (references from Universalis, text in KJV)
//   Look up : any reference, e.g. "luke 3:15", "john 3:16-18", "psalm 23"
//   Random  : a random KJV verse
"use strict";

const BIBLE_API = "https://bible-api.com";

const READINGS = [
  ["Mass_R1", "First Reading"],
  ["Mass_Ps", "Responsorial Psalm"],
  ["Mass_R2", "Second Reading"],
  ["Mass_G", "Gospel"],
];

// Catholic-canon books the KJV text on bible-api.com doesn't have.
const NOT_IN_KJV = ["Tobit", "Judith", "Wisdom", "Sirach", "Ecclesiasticus", "Baruch", "Maccabees"];

const $ = (sel) => document.querySelector(sel);

// ------------------------------------------------------------------ helpers

/** Decode HTML entities and strip tags, e.g. "<h4>Sunday</h4>&#x2010;" -> "Sunday‐" */
function htmlToText(s) {
  const doc = new DOMParser().parseFromString(s, "text/html");
  return (doc.body.textContent || "").replace(/\s+/g, " ").trim();
}

/**
 * Turn a Universalis reference into one bible-api.com understands:
 *   "Psalm 110(111):1&#x2010;6"   -> "Psalm 111:1-6"   (KJV psalm numbering)
 *   "Isaiah 55:10‐11a"            -> "Isaiah 55:10-11" (no half-verse letters)
 *   "Philippians 4:12‐14,19‐20"   -> "Philippians 4:12-14,19-20"
 */
function normalizeReference(src) {
  const s = htmlToText(src);
  const i = s.lastIndexOf(" ");
  if (i < 0) return s;
  const book = s.slice(0, i).trim();
  let nums = s.slice(i + 1).replace(/[‐-―]/g, "-");
  // "110(111):1-6" -> "111:1-6": Greek number first, Hebrew/KJV in brackets
  nums = nums.replace(/^\d+\((\d+)\)/, "$1");
  nums = nums.replace(/[A-Za-z]/g, "");
  return `${book} ${nums}`;
}

/** The API text has stray line breaks inside verses. */
const clean = (s) => (s || "").replace(/\s+/g, " ").trim();

function localDateKey(d = new Date()) {
  const p = (n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

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
  const json = await getJson(`${BIBLE_API}/${encodeURI(q)}?translation=kjv`);
  if (json.error) throw new Error(json.error);
  const verses = (json.verses || []).map((v) => [v.verse, clean(v.text)]);
  if (!verses.length) throw new Error("Not found.");
  return { reference: json.reference || ref, verses };
}

async function fetchRandom() {
  const json = await getJson(`${BIBLE_API}/data/kjv/random`);
  const v = json.random_verse;
  if (!v) throw new Error("Unexpected response.");
  return { reference: `${v.book} ${v.chapter}:${v.verse}`, verses: [[v.verse, clean(v.text)]] };
}

// Universalis only offers JSONP (a <script> that calls universalisCallback),
// so requests are queued one at a time to keep callbacks from crossing.
let jsonpQueue = Promise.resolve();

function loadUniversalis(ymd, attempt) {
  return new Promise((resolve, reject) => {
    const script = document.createElement("script");
    let finished = false;
    const finish = (fn, arg) => {
      if (finished) return;
      finished = true;
      clearTimeout(timer);
      clearTimeout(afterLoad);
      delete window.universalisCallback;
      script.remove();
      fn(arg);
    };
    let afterLoad;
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
    script.src = `https://universalis.com/${ymd}/jsonpmass.js${attempt ? `?r=${Date.now()}` : ""}`;
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

function renderPassage(p, { label } = {}) {
  const wrap = el("article", { class: "reading" });
  if (label) wrap.append(el("p", { class: "reading-label" }, label));
  wrap.append(el("h2", { class: "passage-ref" }, p.reference, el("span", { class: "tr" }, "KJV")));
  const body = el("p", { class: "passage" });
  const numbered = p.verses.length > 1;
  p.verses.forEach(([n, text], i) => {
    if (numbered) body.append(el("sup", {}, n));
    body.append(text + (i < p.verses.length - 1 ? " " : ""));
  });
  wrap.append(body);
  return wrap;
}

function renderError(target, err) {
  target.replaceChildren(el("p", { class: "error" }, err.message || String(err)));
}

// ------------------------------------------------------------------ Today

let todayToken = 0;

async function showReadings(dateKey) {
  const token = ++todayToken;
  const title = $("#day-title");
  const alt = $("#day-alt");
  const out = $("#readings");

  $("#date").value = dateKey;
  $("#today-btn").hidden = dateKey === localDateKey();
  title.textContent = "";
  alt.textContent = "";
  out.replaceChildren(skeleton());

  let data;
  try {
    data = await fetchMassReadings(dateKey);
  } catch (e) {
    if (token === todayToken) renderError(out, e);
    return;
  }
  if (token !== todayToken) return;

  // "Friday of week 27 ... or Saint Denis ... or Saint John Leonardi ..."
  const [main, ...optional] = htmlToText(data.day || "").split(/ or (?=Saint|Saints|Blessed|Our Lady|The )/);
  title.textContent = main || data.date || "";
  alt.textContent = optional.length ? `Optional memorial${optional.length > 1 ? "s" : ""}: ${optional.join("; ")}` : "";
  document.title = `${main} · biblefnd`;

  const present = READINGS.filter(([key]) => data[key] && data[key].source);
  const slots = present.map(([, label]) => {
    const slot = el("div", { class: "reading" }, el("p", { class: "reading-label" }, label), skeleton());
    return slot;
  });
  out.replaceChildren(...slots);

  await Promise.all(
    present.map(async ([key, label], i) => {
      const reference = normalizeReference(data[key].source);
      let node;
      try {
        node = renderPassage(await fetchReference(reference), { label });
      } catch (e) {
        const why = NOT_IN_KJV.some((b) => reference.includes(b))
          ? "This book isn't in the King James Version."
          : `Couldn't load the KJV text. ${e.message}`;
        node = el("article", { class: "reading" },
          el("p", { class: "reading-label" }, label),
          el("h2", { class: "passage-ref" }, reference),
          el("p", { class: "note" }, why));
      }
      if (token === todayToken) slots[i].replaceWith(node);
    })
  );
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
  out.replaceChildren(skeleton());
  try {
    out.replaceChildren(renderPassage(await fetchRandom()));
  } catch (e) {
    renderError(out, e);
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
    if ($("#date").value !== dateKey || !$("#readings").hasChildNodes()) showReadings(dateKey);
  } else if (view === "lookup") {
    if (arg && arg !== lastLookup) showLookup(arg);
    else if (!arg) $("#ref").focus();
  } else if (view === "random" && !randomLoaded) {
    randomLoaded = true;
    showRandom();
  }
}

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
  if (document.visibilityState === "visible" && (location.hash === "" || location.hash === "#today")
      && $("#date").value !== localDateKey()) {
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
window.biblefnd = { normalizeReference, htmlToText, clean };

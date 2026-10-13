//! biblefnd — Bible verses and the day's Mass readings from the terminal.
//!
//!   biblefnd luke 3:15        a specific verse (KJV)
//!   biblefnd john 3:16-18     a range (KJV)
//!   biblefnd psalm 23         a whole chapter (KJV)
//!   biblefnd random           a random verse (KJV)
//!   biblefnd verse            a single verse of the day (KJV)
//!   biblefnd today            today's Mass readings (Danish calendar)
//!   biblefnd today 2026-12-25 the readings for another date
//!
//! Verse look-ups use bible-api.com (King James Version, public domain).
//! The Mass readings come from Universalis: the English Catholic lectionary
//! text (Jerusalem Bible, Grail psalms) with its copyright notice.

use serde_json::Value;
use std::io::IsTerminal;
use std::process::ExitCode;

const BIBLE_API: &str = "https://bible-api.com";
const VERSE_OF_DAY_API: &str = "https://beta.ourmanna.com/api/v1/get?format=json&order=daily";

/// Universalis calendar: Denmark, so Danish feasts (e.g. St Ansgar) are included.
const UNIVERSALIS_REGION: &str = "europe.denmark";

/// A KJV passage ready to print.
struct Passage {
    reference: String,
    /// (verse number, text). Single-verse lookups have one entry.
    verses: Vec<(u64, String)>,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let result = match args.first().map(|s| s.to_lowercase()).as_deref() {
        None | Some("-h") | Some("--help") | Some("help") => {
            print_help();
            Ok(())
        }
        Some("today") if args.len() == 1 => show_readings(&chrono::Local::now().format("%Y%m%d").to_string()),
        Some("today") if args.len() == 2 => parse_date(&args[1]).and_then(|d| show_readings(&d)),
        Some("random") if args.len() == 1 => fetch_random().map(|p| print_passage(&p)),
        Some("verse") if args.len() == 1 => fetch_verse_of_day().map(|p| print_passage(&p)),
        _ => fetch_reference(&args.join(" ")).map(|p| print_passage(&p)),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("biblefnd: {e}");
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    println!(
        "biblefnd — Bible look-up and daily Mass readings

USAGE:
    biblefnd today                      today's Mass readings (Danish calendar)
    biblefnd today <YYYY-MM-DD>         the readings for another date
    biblefnd <book> <chapter>:<verse>   e.g. biblefnd luke 3:15      (KJV)
    biblefnd <book> <chapter>:<a>-<b>   e.g. biblefnd john 3:16-18   (KJV)
    biblefnd <book> <chapter>           e.g. biblefnd psalm 23       (KJV)
    biblefnd random                     a random verse               (KJV)
    biblefnd verse                      a single verse of the day    (KJV)"
    );
}

/// "2026-12-25" -> "20261225"
fn parse_date(s: &str) -> Result<String, String> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map(|d| d.format("%Y%m%d").to_string())
        .map_err(|_| format!("'{s}' isn't a date — use YYYY-MM-DD, e.g. 2026-12-25"))
}

// ---------------------------------------------------------------- fetching

fn get_text(url: &str) -> Result<String, String> {
    match ureq::get(url)
        .set("User-Agent", "biblefnd/0.3")
        .timeout(std::time::Duration::from_secs(15))
        .call()
    {
        Ok(resp) => resp.into_string().map_err(|e| format!("could not read response: {e}")),
        Err(ureq::Error::Status(404, _)) => Err("not found — check the book name and numbers".into()),
        Err(ureq::Error::Status(429, _)) => Err("rate limited by the server, try again in a moment".into()),
        Err(ureq::Error::Status(code, _)) => Err(format!("server returned HTTP {code}")),
        Err(ureq::Error::Transport(t)) => Err(format!("network error: {t}")),
    }
}

fn get_json(url: &str) -> Result<Value, String> {
    let body = get_text(url)?;
    serde_json::from_str(&body).map_err(|e| format!("could not parse response: {e}"))
}

/// `luke 3:15`, `1 john 4:8`, `john 3:16-18`, `psalm 23`
fn fetch_reference(reference: &str) -> Result<Passage, String> {
    let reference = reference.trim();
    if reference.is_empty() {
        return Err("no reference given".into());
    }
    let url = format!("{BIBLE_API}/{}?translation=kjv", encode_path(reference));
    let json = get_json(&url)?;

    if let Some(err) = json.get("error").and_then(Value::as_str) {
        return Err(err.to_string());
    }

    let verses = json["verses"]
        .as_array()
        .ok_or("unexpected response from bible-api.com")?
        .iter()
        .map(|v| (v["verse"].as_u64().unwrap_or(0), clean(v["text"].as_str().unwrap_or(""))))
        .collect::<Vec<_>>();

    if verses.is_empty() {
        return Err("not found".into());
    }

    Ok(Passage {
        reference: json["reference"].as_str().unwrap_or(reference).to_string(),
        verses,
    })
}

fn fetch_random() -> Result<Passage, String> {
    let json = get_json(&format!("{BIBLE_API}/data/kjv/random"))?;
    let v = &json["random_verse"];
    match (v["book"].as_str(), v["chapter"].as_u64(), v["verse"].as_u64()) {
        (Some(b), Some(c), Some(n)) => Ok(Passage {
            reference: format!("{b} {c}:{n}"),
            verses: vec![(n, clean(v["text"].as_str().unwrap_or("")))],
        }),
        _ => Err("unexpected response from bible-api.com".into()),
    }
}

fn fetch_verse_of_day() -> Result<Passage, String> {
    let json = get_json(VERSE_OF_DAY_API)?;
    let reference = json["verse"]["details"]["reference"]
        .as_str()
        .ok_or("could not get today's verse reference")?;
    // OurManna serves NIV text, so only take the reference and re-fetch it in KJV.
    fetch_reference(reference)
}

/// Print the Mass readings for a date given as YYYYMMDD.
fn show_readings(ymd: &str) -> Result<(), String> {
    let body = get_text(&format!("https://universalis.com/{UNIVERSALIS_REGION}/{ymd}/jsonpmass.js"))?;

    // The response is JSONP: universalisCallback({...});
    let (start, end) = (body.find('{'), body.rfind('}'));
    let json: Value = match (start, end) {
        (Some(s), Some(e)) if s < e => serde_json::from_str(&body[s..=e])
            .map_err(|e| format!("could not parse readings: {e}"))?,
        _ => return Err("unexpected response from universalis.com".into()),
    };
    let Some(obj) = json.as_object() else {
        return Err("unexpected response from universalis.com".into());
    };

    let st = Styles::new();

    // "Friday of week 27 ... or Saint Denis ... or Saint John Leonardi ..."
    let day = html_to_plain(json["day"].as_str().unwrap_or(""), &st).replace('\n', " ");
    let day = clean(&day);
    let (main, optional) = split_day_title(&day);
    println!("{}{}{}", st.bold, main, st.reset);
    if !optional.is_empty() {
        let s = if optional.len() > 1 { "s" } else { "" };
        println!("{}Optional memorial{s}: {}{}", st.dim, optional.join("; "), st.reset);
    }
    println!("{}{}{}", st.dim, json["date"].as_str().unwrap_or(""), st.reset);

    // Readings in the order Universalis sends them (serde_json "preserve_order").
    let mut found = false;
    for (key, reading) in obj.iter().filter(|(k, _)| k.starts_with("Mass_")) {
        let text = reading["text"].as_str().unwrap_or("");
        let source = reading["source"].as_str().unwrap_or("");
        if text.is_empty() && source.is_empty() {
            continue;
        }
        found = true;
        println!();
        println!("{}── {} ──{}", st.bold, label_for(key), st.reset);
        let source = spaced_source(&clean(&html_to_plain(source, &Styles::plain())));
        if !source.is_empty() {
            println!("{}{}{}", st.bold, source, st.reset);
        }
        let heading = clean(&html_to_plain(reading["heading"].as_str().unwrap_or(""), &Styles::plain()));
        if !heading.is_empty() {
            println!("{}{}{}", st.italic, heading, st.italic_off);
        }
        println!();
        println!("{}", html_to_plain(text, &st));
    }
    if !found {
        println!("\nNo readings were found for this day.");
    }

    // Universalis' terms: show their copyright notice and point to them.
    let copyright = clean(&html_to_plain(json["copyright"].as_str().unwrap_or(""), &Styles::plain()));
    println!();
    if !copyright.is_empty() {
        println!("{}{}{}", st.dim, copyright, st.reset);
    }
    println!("{}Readings provided by Universalis — https://universalis.com{}", st.dim, st.reset);
    Ok(())
}

// ---------------------------------------------------------------- helpers

struct Styles {
    bold: &'static str,
    dim: &'static str,
    italic: &'static str,
    italic_off: &'static str,
    reset: &'static str,
}

impl Styles {
    fn new() -> Self {
        if std::io::stdout().is_terminal() {
            Styles { bold: "\x1b[1m", dim: "\x1b[2m", italic: "\x1b[3m", italic_off: "\x1b[23m", reset: "\x1b[0m" }
        } else {
            Self::plain()
        }
    }
    fn plain() -> Self {
        Styles { bold: "", dim: "", italic: "", italic_off: "", reset: "" }
    }
}

fn label_for(key: &str) -> String {
    let k = key.trim_start_matches("Mass_");
    match k {
        "R1" => "First Reading".into(),
        "Ps" => "Responsorial Psalm".into(),
        "R2" => "Second Reading".into(),
        "GA" => "Gospel Acclamation".into(),
        "G" => "Gospel".into(),
        _ if k.starts_with("GA") => "Gospel Acclamation".into(),
        _ if k.starts_with("Ps") => "Psalm".into(),
        _ if k.starts_with('R') => format!("Reading {}", &k[1..]),
        _ if k.starts_with('G') => "Gospel".into(),
        _ => k.to_string(),
    }
}

/// "Friday of week 27 or Saint Denis or Saint John" -> ("Friday of week 27", ["Saint Denis", "Saint John"])
fn split_day_title(day: &str) -> (String, Vec<String>) {
    const STARTS: [&str; 5] = ["Saint", "Saints", "Blessed", "Our Lady", "The "];
    let mut parts = Vec::new();
    let mut last = 0;
    for (i, _) in day.match_indices(" or ") {
        if STARTS.iter().any(|s| day[i + 4..].starts_with(s)) {
            parts.push(day[last..i].to_string());
            last = i + 4;
        }
    }
    parts.push(day[last..].to_string());
    let mut it = parts.into_iter();
    let main = it.next().unwrap_or_default();
    (main, it.collect())
}

/// "Heb4:12" -> "Heb 4:12"
fn spaced_source(s: &str) -> String {
    let bytes: Vec<char> = s.chars().collect();
    for i in 1..bytes.len() {
        if bytes[i].is_ascii_digit() && bytes[i - 1].is_alphabetic() && !s.contains(' ') {
            let (a, b) = s.split_at(s.char_indices().nth(i).map(|(p, _)| p).unwrap_or(0));
            return format!("{a} {b}");
        }
    }
    s.to_string()
}

/// Convert the Universalis text (HTML, or plain text with *italic* markers)
/// into terminal text: paragraphs separated by a blank line, <br> as line
/// breaks, italics as ANSI italics when printing to a terminal.
fn html_to_plain(s: &str, st: &Styles) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c == '<' {
            if let Some(end) = s[i..].find('>') {
                let tag = s[i + 1..i + end].trim().to_ascii_lowercase();
                let name: String = tag
                    .trim_start_matches('/')
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric())
                    .collect();
                let closing = tag.starts_with('/');
                match name.as_str() {
                    "br" => out.push('\n'),
                    "p" | "div" | "blockquote" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "li" => out.push_str("\n\n"),
                    "i" | "em" => out.push_str(if closing { st.italic_off } else { st.italic }),
                    _ => {}
                }
                // skip to the end of the tag
                while let Some(&(j, _)) = chars.peek() {
                    if j > i + end {
                        break;
                    }
                    chars.next();
                }
                continue;
            }
        }
        out.push(c);
    }

    let out = decode_entities(&out);

    // plain-text feeds mark the psalm response as *text*
    let mut styled = String::with_capacity(out.len());
    let mut open = false;
    for c in out.chars() {
        if c == '*' {
            styled.push_str(if open { st.italic_off } else { st.italic });
            open = !open;
        } else {
            styled.push(c);
        }
    }
    if open {
        styled.push_str(st.italic_off);
    }

    // tidy whitespace: trim each line, at most one blank line in a row
    let mut lines: Vec<String> = Vec::new();
    for line in styled.lines() {
        let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if line.is_empty() && lines.last().is_none_or(|l| l.is_empty()) {
            continue;
        }
        lines.push(line);
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

/// Percent-encode a reference for the URL path; spaces become '+'.
fn encode_path(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b':' | b'-' | b',' | b'.' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The API text contains stray line breaks; collapse all whitespace runs.
fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Decode HTML entities: numeric (`&#8208;`, `&#x2010;`) and the common named ones.
/// Anything unrecognised is left as-is.
fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        let decoded = after
            .find(';')
            .filter(|&semi| semi <= 10)
            .and_then(|semi| {
                let name = &after[..semi];
                let c = if let Some(hex) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
                    u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
                } else if let Some(dec) = name.strip_prefix('#') {
                    dec.parse::<u32>().ok().and_then(char::from_u32)
                } else {
                    match name {
                        "nbsp" => Some(' '),
                        "amp" => Some('&'),
                        "lt" => Some('<'),
                        "gt" => Some('>'),
                        "quot" => Some('"'),
                        "apos" | "rsquo" | "lsquo" => Some('\''),
                        "ldquo" | "rdquo" => Some('"'),
                        "ndash" => Some('\u{2013}'),
                        "mdash" => Some('\u{2014}'),
                        "copy" => Some('©'),
                        _ => None,
                    }
                };
                c.map(|c| (c, semi))
            });
        match decoded {
            Some((c, semi)) => {
                out.push(c);
                rest = &after[semi + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn print_passage(p: &Passage) {
    let st = Styles::new();
    println!("{}{} (KJV){}", st.bold, p.reference, st.reset);
    if p.verses.len() == 1 {
        println!("{}", p.verses[0].1);
    } else {
        for (n, text) in &p.verses {
            println!("{}{n:>3}{} {text}", st.dim, st.reset);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_references() {
        assert_eq!(encode_path("luke 3:15"), "luke+3:15");
        assert_eq!(encode_path("1 john 4:8"), "1+john+4:8");
        assert_eq!(encode_path("song of solomon 2:1"), "song+of+solomon+2:1");
    }

    #[test]
    fn decodes_entities() {
        assert_eq!(decode_entities("a&#x2010;b"), "a\u{2010}b");
        assert_eq!(decode_entities("a&#8208;b"), "a\u{2010}b");
        assert_eq!(decode_entities("Tom &amp; Jerry"), "Tom & Jerry");
        assert_eq!(decode_entities("R&D; fine"), "R&D; fine");
    }

    #[test]
    fn converts_html_readings() {
        let p = Styles::plain();
        assert_eq!(
            html_to_plain("<div><p>First para.</p><p>Line one,<br>line two.</p></div>", &p),
            "First para.\n\nLine one,\nline two."
        );
        assert_eq!(html_to_plain("<h4>Sunday</h4>&#160;", &p), "Sunday");
        assert_eq!(html_to_plain("<p><i>Response.</i></p>", &p), "Response.");
    }

    #[test]
    fn converts_plain_readings() {
        let p = Styles::plain();
        assert_eq!(
            html_to_plain("\n\n*Response.*\nLine one,\nline two.\n\n\n\nNext.", &p),
            "Response.\nLine one,\nline two.\n\nNext."
        );
        let it = Styles { bold: "", dim: "", italic: "[", italic_off: "]", reset: "" };
        assert_eq!(html_to_plain("*Response.*", &it), "[Response.]");
        assert_eq!(html_to_plain("<i>Response.</i>", &it), "[Response.]");
    }

    #[test]
    fn splits_day_title() {
        let (main, alt) = split_day_title("Friday of week 27 in Ordinary Time or Saints Denis and Companions or Saint John Leonardi");
        assert_eq!(main, "Friday of week 27 in Ordinary Time");
        assert_eq!(alt, vec!["Saints Denis and Companions", "Saint John Leonardi"]);
        let (main, alt) = split_day_title("28th Sunday in Ordinary Time");
        assert_eq!(main, "28th Sunday in Ordinary Time");
        assert!(alt.is_empty());
    }

    #[test]
    fn labels_and_sources() {
        assert_eq!(label_for("Mass_R1"), "First Reading");
        assert_eq!(label_for("Mass_GA"), "Gospel Acclamation");
        assert_eq!(label_for("Mass_R3"), "Reading 3");
        assert_eq!(spaced_source("Heb4:12"), "Heb 4:12");
        assert_eq!(spaced_source("Luke 11:37-41"), "Luke 11:37-41");
    }
}

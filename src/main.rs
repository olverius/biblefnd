//! biblefnd — look up King James Version passages from the terminal.
//!
//!   biblefnd luke 3:15        a specific verse
//!   biblefnd john 3:16-18     a range
//!   biblefnd psalm 23         a whole chapter
//!   biblefnd random           a random verse
//!   biblefnd today            today's Mass readings (first reading, psalm,
//!                             second reading on Sundays/feasts, Gospel)
//!   biblefnd verse            a single verse of the day
//!
//! Verse text comes from bible-api.com (KJV, public domain).
//! The day's reading references come from universalis.com; only the
//! references are used, the text itself is always fetched in KJV.

use serde_json::Value;
use std::io::IsTerminal;
use std::process::ExitCode;

const BIBLE_API: &str = "https://bible-api.com";
const VERSE_OF_DAY_API: &str = "https://beta.ourmanna.com/api/v1/get?format=json&order=daily";

/// (JSON key in the Universalis response, label to print)
const READINGS: [(&str, &str); 4] = [
    ("Mass_R1", "First Reading"),
    ("Mass_Ps", "Responsorial Psalm"),
    ("Mass_R2", "Second Reading"),
    ("Mass_G", "Gospel"),
];

/// Books in the Catholic canon that the KJV text on bible-api.com doesn't have.
const NOT_IN_KJV: [&str; 7] = ["Tobit", "Judith", "Wisdom", "Sirach", "Ecclesiasticus", "Baruch", "Maccabees"];

/// A passage ready to print.
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
        Some("today") if args.len() == 1 => show_todays_readings(),
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
        "biblefnd — King James Version lookup

USAGE:
    biblefnd <book> <chapter>:<verse>   e.g. biblefnd luke 3:15
    biblefnd <book> <chapter>:<a>-<b>   e.g. biblefnd john 3:16-18
    biblefnd <book> <chapter>           e.g. biblefnd psalm 23
    biblefnd today                      today's Mass readings and Gospel
    biblefnd random                     a random verse
    biblefnd verse                      a single verse of the day"
    );
}

// ---------------------------------------------------------------- fetching

fn get_text(url: &str) -> Result<String, String> {
    match ureq::get(url)
        .set("User-Agent", "biblefnd/0.2")
        .timeout(std::time::Duration::from_secs(10))
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

/// `luke 3:15`, `1 john 4:8`, `john 3:16-18`, `psalm 23`, `phil 4:12-14,19-20`
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

fn show_todays_readings() -> Result<(), String> {
    let date = chrono::Local::now().format("%Y%m%d");
    let body = get_text(&format!("https://universalis.com/{date}/jsonpmass.js"))?;

    // The response is JSONP: universalisCallback({...});
    let (start, end) = (body.find('{'), body.rfind('}'));
    let json: Value = match (start, end) {
        (Some(s), Some(e)) if s < e => serde_json::from_str(&body[s..=e])
            .map_err(|e| format!("could not parse readings: {e}"))?,
        _ => return Err("unexpected response from universalis.com".into()),
    };

    let (bold, dim, reset) = styles();
    let date_str = json["date"].as_str().unwrap_or("");
    let day = strip_html(json["day"].as_str().unwrap_or(""));
    println!("{bold}{day}{reset}");
    println!("{dim}{date_str}{reset}");

    for (key, label) in READINGS {
        let Some(source) = json[key]["source"].as_str() else {
            continue; // e.g. no second reading on weekdays
        };
        let reference = normalize_reference(source);
        println!();
        println!("{bold}── {label} ──{reset}");

        match fetch_reference(&reference) {
            Ok(p) => print_passage(&p),
            Err(e) => {
                println!("{bold}{reference}{reset}");
                if NOT_IN_KJV.iter().any(|b| reference.contains(b)) {
                    println!("{dim}(this book isn't in the King James Version){reset}");
                } else {
                    println!("{dim}(couldn't fetch the KJV text: {e}){reset}");
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- helpers

/// Turn a Universalis reference into one bible-api.com understands:
///   "Psalm 110(111):1‐6"        -> "Psalm 111:1-6"   (Hebrew/KJV psalm numbering)
///   "Isaiah 55:10‐11a"          -> "Isaiah 55:10-11" (no half-verse letters)
///   "Philippians 4:12‐14,19‐20" -> "Philippians 4:12-14,19-20"
fn normalize_reference(src: &str) -> String {
    // Universalis sends the dash as an HTML entity ("&#x2010;"), so decode first.
    let decoded = decode_entities(src);
    let src = decoded.trim();
    let Some((book, nums)) = src.rsplit_once(char::is_whitespace) else {
        return src.to_string();
    };

    let mut nums: String = nums
        .chars()
        .map(|c| match c {
            '\u{2010}'..='\u{2015}' => '-',
            c => c,
        })
        .collect();

    // "110(111):1-6" -> "111:1-6": Universalis gives the Greek number first and
    // the Hebrew one (used by the KJV) in brackets.
    if let (Some(open), Some(close)) = (nums.find('('), nums.find(')')) {
        if open < close {
            nums = format!("{}{}", &nums[open + 1..close], &nums[close + 1..]);
        }
    }

    let nums: String = nums.chars().filter(|c| !c.is_ascii_alphabetic()).collect();
    format!("{} {}", book.trim(), nums)
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

/// Drop tags and decode the few entities Universalis uses in its titles.
fn strip_html(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    clean(&decode_entities(&out))
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
                        "quot" => Some('"'),
                        "apos" | "rsquo" | "lsquo" => Some('\''),
                        "ndash" => Some('\u{2013}'),
                        "mdash" => Some('\u{2014}'),
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

fn styles() -> (&'static str, &'static str, &'static str) {
    if std::io::stdout().is_terminal() {
        ("\x1b[1m", "\x1b[2m", "\x1b[0m")
    } else {
        ("", "", "")
    }
}

fn print_passage(p: &Passage) {
    let (bold, dim, reset) = styles();
    println!("{bold}{} (KJV){reset}", p.reference);
    if p.verses.len() == 1 {
        println!("{}", p.verses[0].1);
    } else {
        for (n, text) in &p.verses {
            println!("{dim}{n:>3}{reset} {text}");
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
        assert_eq!(encode_path("john 3:16-18"), "john+3:16-18");
        assert_eq!(encode_path("song of solomon 2:1"), "song+of+solomon+2:1");
    }

    #[test]
    fn cleans_whitespace() {
        assert_eq!(clean("shall be\nof fine flour;\n"), "shall be of fine flour;");
    }

    #[test]
    fn normalizes_universalis_references() {
        assert_eq!(normalize_reference("Galatians 3:7\u{2010}14"), "Galatians 3:7-14");
        assert_eq!(normalize_reference("Psalm 110(111):1\u{2010}6"), "Psalm 111:1-6");
        assert_eq!(normalize_reference("Psalm 22(23)"), "Psalm 23");
        assert_eq!(normalize_reference("Philippians 4:12\u{2010}14,19\u{2010}20"), "Philippians 4:12-14,19-20");
        assert_eq!(normalize_reference("Isaiah 55:10\u{2010}11a"), "Isaiah 55:10-11");
        assert_eq!(normalize_reference("1 John 4:7\u{2010}10"), "1 John 4:7-10");
        // what Universalis actually sends
        assert_eq!(normalize_reference("Galatians 3:7&#x2010;14"), "Galatians 3:7-14");
        assert_eq!(normalize_reference("Psalm 110(111):1&#x2010;6"), "Psalm 111:1-6");
        assert_eq!(normalize_reference("Luke 11:15&#8208;26"), "Luke 11:15-26");
        assert_eq!(normalize_reference("Mark 1:7&ndash;11"), "Mark 1:7-11");
    }

    #[test]
    fn decodes_entities() {
        assert_eq!(decode_entities("a&#x2010;b"), "a\u{2010}b");
        assert_eq!(decode_entities("a&#8208;b"), "a\u{2010}b");
        assert_eq!(decode_entities("Tom &amp; Jerry"), "Tom & Jerry");
        assert_eq!(decode_entities("R&D; fine"), "R&D; fine");
        assert_eq!(decode_entities("lone & sign"), "lone & sign");
    }

    #[test]
    fn strips_html_titles() {
        assert_eq!(
            strip_html("<h4>28th Sunday in Ordinary Time</h4>&nbsp;"),
            "28th Sunday in Ordinary Time"
        );
    }
}

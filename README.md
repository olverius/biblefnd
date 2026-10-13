# biblefnd

Today's Catholic Mass readings in English, following the calendar of the Catholic
Church in Denmark, plus quick Bible look-ups. Made for students in our parish
who want to follow what's read at Mass in Danish.

Available as a **web app for phones** and as a **command-line tool**.

## Web app (Android & iPhone)

**https://olverius.github.io/biblefnd/**

- **Today:** the day's readings: first reading, psalm, second reading (Sundays and
  feasts), Gospel acclamation and Gospel. Pick any other date with the date picker.
- **Look up:** any passage, e.g. `luke 3:15`, `john 3:16-18`, `psalm 23` (KJV).
- **Random:** a random verse (KJV).

To install it on your phone:

- **Android (Chrome):** open the link, tap ⋮ → *Add to Home screen* / *Install app*.
- **iPhone (Safari):** open the link, tap Share → *Add to Home Screen*.

It opens full-screen like a normal app, and readings you've already opened
still work offline. Shareable links: `#today/2026-12-25`, `#lookup/john 3:16`.

The source is in [`docs/`](docs/) (plain HTML/CSS/JS, no build step) and is
served by GitHub Pages.

## Command-line tool

```sh
biblefnd today              # today's Mass readings
biblefnd today 2026-12-25   # the readings for another date
biblefnd luke 3:15          # a single verse (KJV)
biblefnd john 3:16-18       # a range (KJV)
biblefnd psalm 23           # a whole chapter (KJV)
biblefnd random             # a random verse (KJV)
biblefnd verse              # a single verse of the day (KJV)
```

### Install

Requires a Rust toolchain ([rustup](https://rustup.rs)).

```sh
git clone https://github.com/olverius/biblefnd.git
cd biblefnd
cargo install --path .
```

This puts `biblefnd` in `~/.cargo/bin`. If the command isn't found afterwards,
add that folder to your `PATH`:

```sh
export PATH="$HOME/.cargo/bin:$PATH"   # e.g. in ~/.bashrc
```

## Where the texts come from

- **Mass readings** come from [Universalis](https://universalis.com), using its
  calendar for Denmark (so Danish feasts such as St Ansgar are included). The
  text is the English Catholic lectionary: the *Jerusalem Bible* with the *Grail*
  psalms. These translations are under copyright; Universalis provides them
  for use on web pages provided their copyright notice is shown and they are
  credited. Both the web app and the CLI show the notice under the readings.
- **Look up and Random** use the King James Version from
  [bible-api.com](https://bible-api.com) (public domain). It's a Protestant
  translation in older English and doesn't include the deuterocanonical books
  (Tobit, Wisdom, Sirach, …), so for following Mass, use the Today readings.
- **Verse of the day** (`biblefnd verse`, CLI only) takes its reference from
  [OurManna](https://ourmanna.com) and fetches the text in KJV.

## Development

```sh
cargo test               # text-conversion and parsing tests
cargo run -- today
```

For the web app, serve `docs/` locally (e.g. `python -m http.server -d docs`)
and open http://localhost:8000.

Dependencies: [`ureq`](https://crates.io/crates/ureq) (HTTP),
[`serde_json`](https://crates.io/crates/serde_json),
[`chrono`](https://crates.io/crates/chrono) (dates).

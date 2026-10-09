# biblefnd

Look up King James Version Bible passages and today's Mass readings from the terminal.

```
$ biblefnd luke 3:15
Luke 3:15 (KJV)
And as the people were in expectation, and all men mused in their hearts of John, whether he were the Christ, or not;
```

## Usage

```sh
biblefnd luke 3:15          # a single verse
biblefnd 1 john 4:8         # books with numbers work too
biblefnd john 3:16-18       # a range
biblefnd psalm 23           # a whole chapter
biblefnd today              # today's Mass readings and Gospel
biblefnd random             # a random verse
biblefnd verse              # a single verse of the day
biblefnd --help
```

`biblefnd today` prints the first reading, responsorial psalm, the second reading
(on Sundays and feasts) and the Gospel for today's date, all in KJV:

```
Friday of week 27 in Ordinary Time
Friday 9 October 2026

── First Reading ──
Galatians 3:7-14 (KJV)
  7 Know ye therefore that they which are of faith, the same are the children of Abraham.
  ...

── Responsorial Psalm ──
Psalms 111:1-6 (KJV)
  ...

── Gospel ──
Luke 11:15-26 (KJV)
  ...
```

## Install

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

## How it works

- **Verse text** comes from [bible-api.com](https://bible-api.com), using the
  King James Version (public domain). No API key needed.
- **Today's readings** take the day's references from
  [Universalis](https://universalis.com), then fetch each passage in KJV. Only the
  references are used, not Universalis' own text.
  - Catholic sources number most psalms one lower than the KJV (Greek vs Hebrew
    numbering). Universalis gives both, e.g. `Psalm 110(111)`, and biblefnd uses
    the KJV number.
  - Half-verse references like `11a` are fetched as the whole verse.
  - Some first readings come from Tobit, Judith, Wisdom, Sirach, Baruch or
    Maccabees, which the KJV doesn't include. On those days the reference is
    shown with a note and the other readings still print.
- **Verse of the day** (`biblefnd verse`) takes its reference from
  [OurManna](https://ourmanna.com) and fetches the text in KJV.

The date used for `today` is your local date.

## Development

```sh
cargo test     # reference parsing and HTML-cleanup tests
cargo run -- today
```

Dependencies: [`ureq`](https://crates.io/crates/ureq) (HTTP),
[`serde_json`](https://crates.io/crates/serde_json),
[`chrono`](https://crates.io/crates/chrono) (local date).

import json
import re
import sys
import time
import urllib.parse
import urllib.request
from urllib.error import HTTPError
from pathlib import Path

try:
    from deep_translator import GoogleTranslator
except Exception:
    GoogleTranslator = None

SEPARATOR = " <1234567890> "
SEPARATOR_PATTERN = re.compile(r"<\s*1234567890\s*>")
TRANSLATION_URL = "https://translate.googleapis.com/translate_a/single"
MYMEMORY_URL = "https://api.mymemory.translated.net/get"
BATCH_SIZE = 25
INVISIBLE_CHARACTERS = {
    "\u200b",
    "\u200c",
    "\u200d",
    "\ufeff",
}

LANGS = [
    ("fr", "fr", "French"),
    ("es", "es", "Spanish"),
    ("ar", "ar", "Arabic"),
    ("de", "de", "German"),
    ("he", "iw", "Hebrew"),
    ("hi", "hi", "Hindi"),
    ("id", "id", "Indonesian"),
    ("it", "it", "Italian"),
    ("ja", "ja", "Japanese"),
    ("ko", "ko", "Korean"),
    ("nl", "nl", "Dutch"),
    ("pl", "pl", "Polish"),
    ("ru", "ru", "Russian"),
    ("th", "th", "Thai"),
    ("tr", "tr", "Turkish"),
    ("uk", "uk", "Ukrainian"),
    ("vi", "vi", "Vietnamese"),
]


def rust_escape(value: str) -> str:
    return (
        value.replace("\\", "\\\\")
        .replace('"', '\\"')
        .replace("\r", "\\r")
        .replace("\n", "\\n")
        .replace("\t", "\\t")
    )


def sanitize_translation_text(value: str) -> str:
    return "".join(ch for ch in value if ch not in INVISIBLE_CHARACTERS).strip()


def extract_english_translation_keys(source: str) -> list[str]:
    needle = 'self.t('
    keys: list[str] = []
    i = 0

    while i + len(needle) <= len(source):
        if source[i : i + len(needle)] != needle:
            i += 1
            continue

        start = i + len(needle)
        j = start
        depth = 1
        in_string = False
        escape = False

        while j < len(source) and depth > 0:
            ch = source[j]
            if in_string:
                if escape:
                    escape = False
                elif ch == "\\":
                    escape = True
                elif ch == '"':
                    in_string = False
            else:
                if ch == '"':
                    in_string = True
                elif ch == "(":
                    depth += 1
                elif ch == ")":
                    depth -= 1
            j += 1

        inner = source[start : max(start, j - 1)]
        literals: list[str] = []
        k = 0

        while k < len(inner):
            if inner[k] != '"':
                k += 1
                continue

            k += 1
            literal_chars: list[str] = []
            inner_escape = False

            while k < len(inner):
                ch = inner[k]
                if inner_escape:
                    literal_chars.append(
                        {
                            "n": "\n",
                            "r": "\r",
                            "t": "\t",
                            "\\": "\\",
                            '"': '"',
                        }.get(ch, ch)
                    )
                    inner_escape = False
                elif ch == "\\":
                    inner_escape = True
                elif ch == '"':
                    break
                else:
                    literal_chars.append(ch)
                k += 1

            literals.append("".join(literal_chars))
            k += 1

        if literals:
            en = literals[-1]
            if en not in keys:
                keys.append(en)

        i = j

    return keys


def fetch_translations(target_lang: str, keys: list[str]) -> list[str]:
    values: list[str] = []

    for start in range(0, len(keys), BATCH_SIZE):
        batch = keys[start : start + BATCH_SIZE]
        if GoogleTranslator is not None:
            translator = GoogleTranslator(source="en", target=target_lang)
            batch_values = translator.translate_batch(batch)
            if len(batch_values) != len(batch):
                raise RuntimeError(
                    f"deep-translator count mismatch for {target_lang}: expected {len(batch)}, got {len(batch_values)} in batch starting at {start}"
                )
            values.extend(batch_values)
            time.sleep(0.3)
            continue

        payload = {
            "client": "gtx",
            "sl": "en",
            "tl": target_lang,
            "dt": "t",
            "q": SEPARATOR.join(batch),
        }
        body = urllib.parse.urlencode(payload).encode("utf-8")
        request = urllib.request.Request(TRANSLATION_URL, data=body, method="POST")
        data = None
        google_failed = False
        for attempt in range(5):
            try:
                with urllib.request.urlopen(request, timeout=60) as response:
                    data = json.loads(response.read().decode("utf-8"))
                break
            except HTTPError as exc:
                if exc.code != 429:
                    raise
                if attempt == 4:
                    google_failed = True
                    break
                time.sleep(5 * (attempt + 1))
        if data is not None:
            translated = "".join(segment[0] for segment in data[0])
            batch_values = split_translated_values(translated)
        else:
            batch_values = []
        if google_failed or len(batch_values) != len(batch):
            query = urllib.parse.urlencode(
                {
                    "q": SEPARATOR.join(batch),
                    "langpair": f"en|{target_lang}",
                }
            )
            with urllib.request.urlopen(f"{MYMEMORY_URL}?{query}", timeout=60) as response:
                fallback_data = json.loads(response.read().decode("utf-8"))
            translated = fallback_data["responseData"]["translatedText"]
            batch_values = split_translated_values(translated)
        if len(batch_values) != len(batch):
            raise RuntimeError(
                f"translation count mismatch for {target_lang}: expected {len(batch)}, got {len(batch_values)} in batch starting at {start}"
            )
        values.extend(batch_values)
        time.sleep(0.6)

    return values


def split_translated_values(translated: str) -> list[str]:
    return [sanitize_translation_text(segment) for segment in SEPARATOR_PATTERN.split(translated)]


def render_table(lang_code: str, lang_name: str, keys: list[str], values: list[str]) -> str:
    fn_suffix = lang_code
    lines = [
        f"translation_table!(lookup_{fn_suffix}, {{",
    ]
    for key, value in zip(keys, values):
        lines.append(
            f'    "{rust_escape(key)}" => "{rust_escape(sanitize_translation_text(value))}",'
        )
    lines.append("});")
    lines.append("")
    lines.append(f"pub(crate) fn translate_{fn_suffix}(en: &str) -> &str {{")
    lines.append(f"    lookup_{fn_suffix}(en).unwrap_or(en)")
    lines.append("}")
    lines.append("")
    lines.append("#[cfg(test)]")
    lines.append(f"pub(crate) fn has_translation_{fn_suffix}(en: &str) -> bool {{")
    lines.append(f"    lookup_{fn_suffix}(en).is_some()")
    lines.append("}")
    lines.append("")
    return "\n".join(lines)


def main() -> None:
    if len(sys.argv) < 3:
        raise SystemExit(
            "usage: generate_ui_translations.py <source-lib.rs> <output-rs> [lang-code ...]"
        )

    source_path = Path(sys.argv[1])
    output_path = Path(sys.argv[2])
    requested_langs = set(sys.argv[3:])
    langs = [lang for lang in LANGS if not requested_langs or lang[0] in requested_langs]
    if not langs:
        raise RuntimeError("no languages selected")

    source = source_path.read_text(encoding="utf-8")
    source = source.split("\n#[cfg(test)]\nmod ui_tests", 1)[0]
    keys = extract_english_translation_keys(source)
    if not keys:
        raise RuntimeError("no translation keys were found")

    parts = [
        "// Generated by scripts/generate_ui_translations.py",
        "// Do not hand-edit this file; regenerate it from the source UI keys instead.",
        "",
        "macro_rules! translation_table {",
        "    ($lookup:ident, { $($en:literal => $translated:literal,)* }) => {",
        "        fn $lookup(en: &str) -> Option<&'static str> {",
        "            match en {",
        "                $($en => Some($translated),)*",
        "                _ => None,",
        "            }",
        "        }",
        "    };",
        "}",
        "",
    ]

    for lang_code, target_lang, lang_name in langs:
        print(f"Generating {lang_name} ({target_lang}) for {len(keys)} keys...")
        values = fetch_translations(target_lang, keys)
        parts.append(render_table(lang_code, lang_name, keys, values))
        time.sleep(0.4)

    output_path.write_text("\n".join(parts), encoding="utf-8")
    print(f"Wrote {output_path}")


if __name__ == "__main__":
    main()

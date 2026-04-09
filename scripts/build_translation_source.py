from pathlib import Path


FILES = [
    "lib.rs",
    "view_models.rs",
    "dashboard_impl.rs",
    "advanced_pages.rs",
    "duplicates_pages.rs",
    "result_pages.rs",
    "settings_pages.rs",
    "cleanup.rs",
]


def main() -> None:
    ui_src = Path("crates/dirotter-ui/src")
    output_path = ui_src / "_translation_source_all.rs"
    parts: list[str] = []

    for name in FILES:
        source_path = ui_src / name
        content = source_path.read_text(encoding="utf-8").rstrip()
        parts.append(f"// BEGIN {name}\n")
        parts.append(content)
        parts.append(f"\n// END {name}\n")

    output_path.write_text("\n".join(parts) + "\n", encoding="utf-8")
    print(f"Wrote {output_path}")


if __name__ == "__main__":
    main()

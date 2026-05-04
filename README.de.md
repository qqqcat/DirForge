# DirOtter

<p align="center">
  <img src="docs/assets/dirotter-icon.png" alt="DirOtter-App-Symbol" width="160">
</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh-CN.md">中文</a> |
  <a href="README.fr.md">Français</a> |
  <a href="README.es.md">Español</a> |
  <a href="README.de.md">Deutsch</a>
</p>

**DirOtter** ist ein quelloffener, lokal ausgerichteter Festplattenanalysator und Aufräumassistent, gebaut mit Rust.

Er hilft Nutzern zu verstehen, wo Speicherplatz verbraucht wird, große Ordner und Dateien zu finden, Kandidaten für doppelte Dateien zu prüfen und risikoarme Cache- oder temporäre Dateien sicher zu bereinigen, ohne Dateisystemdaten an einen Cloud-Dienst hochzuladen.

DirOtter ist transparent, datenschutzfreundlich und praktisch für alltägliche Nutzer konzipiert, die eine sicherere Alternative zu undurchsichtigen Aufräumwerkzeugen suchen.

## Projektstatus

DirOtter befindet sich derzeit in einer frühen, aber produktionsbereiten Phase.

Die zentrale Windows-Anwendung ist funktionsfähig, getestet und als portable Build verpackt. Das Projekt hat die aktuelle Qualitätsprüfung für Formatierung, Kompilierung, Tests, Linting und Workspace-Build-Validierung bestanden.

Aktueller Validierungsstatus:

- `cargo fmt --all -- --check` besteht
- `cargo check --workspace` besteht mit 0 Fehlern und 0 Warnungen
- `cargo test --workspace` besteht mit 94 Tests
- `cargo clippy --workspace --all-targets -- -D warnings` besteht
- `cargo build --workspace` ist erfolgreich

Das Repository enthält bereits CI-Workflows, Windows-Release-Packaging, portable Installationsskripte und optionale Code-Signing-Hooks.

## Warum DirOtter existiert

Moderne Betriebssysteme und Anwendungen erzeugen viele Caches, temporäre Dateien, heruntergeladene Installer, duplizierte Assets und versteckte Speicherbelegung. Bestehende Aufräumwerkzeuge sind häufig zu undurchsichtig, zu aggressiv oder zu stark von plattformspezifischen Annahmen abhängig.

DirOtter verfolgt einen sichereren und transparenteren Ansatz:

1. Lokale Laufwerke mit vorhersehbaren Strategien scannen.
2. Erklären, was Speicherplatz verbraucht.
3. Aufräumkandidaten mit Risikostufen empfehlen.
4. Nutzer vor dem Löschen prüfen lassen.
5. Umkehrbare Aktionen wie das Verschieben in den Papierkorb bevorzugen.
6. Dateisystemdaten standardmäßig lokal halten.

Das langfristige Ziel ist ein zuverlässiges Open-Source-Werkzeug zur Festplattenanalyse und -bereinigung für Windows, macOS und Linux.

## Kernfunktionen

### Festplattenscan

DirOtter scannt ausgewählte Verzeichnisse und erstellt eine strukturierte Ansicht der Speicherbelegung.

Die Scan-Pipeline unterstützt:

- paralleles Scannen
- Veröffentlichung in Batches
- gedrosselte UI-Aktualisierungen
- Abbruch
- Behandlung des Abschlusszustands
- leichte Sitzungs-Snapshots

Der standardmäßige nutzerseitige Scanmodus konzentriert sich auf eine empfohlene Strategie; erweitertes Scanverhalten kann für komplexe Verzeichnisse oder große externe Laufwerke angepasst werden.

### Aufräumempfehlungen

DirOtter nutzt regelbasierte Analyse, um potenzielle Aufräumkandidaten zu identifizieren.

Empfehlungskategorien umfassen:

- temporäre Dateien
- Cache-Verzeichnisse
- Browser- oder App-Cache-Pfade
- heruntergeladene Installer
- häufige risikoarme generierte Dateien
- große Dateien und Ordner, die eine Prüfung verdienen

Empfehlungen werden bewertet und nach Risikostufe gruppiert, damit sicherere Elemente zuerst angezeigt werden.

### Prüfung doppelter Dateien

DirOtter kann Kandidaten für doppelte Dateien mit einer zuerst größenbasierten Strategie und anschließendem Hintergrund-Hashing identifizieren.

Der Ablauf zur Dublettenprüfung ist darauf ausgelegt, aggressive automatische Löschung zu vermeiden. Er zeigt Kandidatengruppen, empfiehlt eine zu behaltende Datei und vermeidet automatische Auswahl an Hochrisikoorten.

### Aufräumausführung

Unterstützte Aufräumaktionen sind:

- in den Papierkorb verschieben
- dauerhaft löschen
- schnelle Bereinigung für risikoarme Cache-Kandidaten

Die Aufräumausführung meldet Fortschritt und Ergebniszahlen, während Dateien im Hintergrund verarbeitet werden.

### Local-First-Speicherung

DirOtter benötigt für normale Nutzung keine Datenbank.

Einstellungen werden in einer leichten `settings.json` gespeichert. Sitzungsergebnisse werden nur als temporäre komprimierte Snapshots gespeichert und entfernt, wenn sie nicht mehr benötigt werden.

Wenn das Einstellungsverzeichnis nicht beschreibbar ist, fällt DirOtter auf temporären Sitzungsspeicher zurück und meldet dies klar in der Einstellungs-UI.

### Internationalisierung

DirOtter unterstützt die Auswahl von 19 Sprachen:

- Arabisch
- Chinesisch
- Niederländisch
- Englisch
- Französisch
- Deutsch
- Hebräisch
- Hindi
- Indonesisch
- Italienisch
- Japanisch
- Koreanisch
- Polnisch
- Russisch
- Spanisch
- Thai
- Türkisch
- Ukrainisch
- Vietnamesisch

Die aktuelle UI-Übersetzungsprüfung deckt alle unterstützten Sprachen für die ausgelieferten UI-Texte ab. Neue nutzersichtbare UI-Zeichenketten sollten vor dem Merge für jede auswählbare Sprache übersetzt werden.

## Sicherheitsmodell

DirOtter ist beim Löschen bewusst konservativ.

Das Projekt behandelt Aufräumen als sicherheitssensible Operation, weil Fehler zu Datenverlust führen können. Daher basiert DirOtter auf mehreren Sicherheitsprinzipien:

- Aufräumkandidaten vor der Ausführung anzeigen
- Empfehlungen nach Risikostufe klassifizieren
- umkehrbares Löschen über den Papierkorb bevorzugen
- Hochrisiko-Dubletten nicht automatisch auswählen
- dauerhaftes Löschen ausdrücklich machen
- schnelle Bereinigung auf risikoarme Cache- oder temporäre Pfade beschränken
- Operationsergebnisse und Fehler klar darstellen

Zukünftige Arbeit umfasst tiefere Sicherheitsaudits für plattformspezifisches Papierkorbverhalten, Hochrisikopfade, symbolische Links, Berechtigungsfehler und Grenzfälle irreversibler Löschung.

## Workspace-Struktur

```text
crates/
  dirotter-app        # Einstiegspunkt der nativen Anwendung
  dirotter-ui         # UI, Seiten, View Models, Interaktionszustand
  dirotter-core       # Node Store, Aggregation, Abfragen
  dirotter-scan       # Scan-Ereignisstrom und Aggregationsveröffentlichung
  dirotter-dup        # Erkennung von Kandidaten doppelter Dateien
  dirotter-cache      # settings.json und Sitzungs-Snapshot-Speicher
  dirotter-platform   # Explorer-Integration, Papierkorb, Volumes, Cleanup-Staging
  dirotter-actions    # Löschplanung und Aufräumausführung
  dirotter-report     # Export von Text-, JSON- und CSV-Berichten
  dirotter-telemetry  # Diagnose und Laufzeitmetriken
  dirotter-testkit    # Werkzeuge für Regression und Performance-Tests
```

## Bauen und ausführen

### Voraussetzungen

- Rust stable toolchain
- Cargo
- Eine unterstützte Desktop-Plattform

Windows ist derzeit das ausgereifteste Ziel. macOS- und Linux-Unterstützung sind Teil der plattformübergreifenden Roadmap.

### Anwendung starten

```bash
cargo run -p dirotter-app
```

### Release-Build

```bash
cargo build --release -p dirotter-app
```

### Qualitätsprüfung

Vor dem Zusammenführen von Änderungen sollten folgende Prüfungen bestehen:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
```

## Release und Packaging

Das Repository enthält einen Windows-Release-Workflow und Packaging-Skripte.

Aktuelle releasebezogene Komponenten sind:

- CI-Workflow für Formatierung, Checks, Tests und Linting
- Windows-Release-Workflow
- portables Windows-Packaging-Skript
- optionales Windows-Code-Signing-Skript
- portables Installationsskript
- portables Deinstallationsskript

Aktuelle Windows-Artefakte umfassen eine portable ZIP-Build und eine SHA-256-Prüfsummendatei.

Code Signing wird von der Release-Pipeline unterstützt, erfordert aber konfigurierte Secrets, bevor signierte Builds erzeugt werden.

## Roadmap

DirOtter konzentriert sich derzeit auf bessere Zuverlässigkeit, Sicherheit und plattformübergreifende Unterstützung.

Hoch- und mittelpriorisierte Punkte umfassen:

1. Windows-Code-Signing-Secrets für signierte Release-Artefakte konfigurieren.
2. Automatisierte visuelle Regressionstests für die UI hinzufügen.
3. Linux-Abdeckung für Dateisystem- und trash/delete-Verhalten erweitern.
4. macOS-Abdeckung für Dateisystem- und trash/delete-Verhalten erweitern.
5. Sicherheitsgrenzen für Aufräumen und Löschen auditieren.
6. Release-Automatisierung und Changelog-Erzeugung verbessern.
7. Contributor-Dokumentation verbessern.
8. Mehr Integrationstests für große Verzeichnisse, symbolische Links, Berechtigungsfehler und externe Laufwerke hinzufügen.
9. Die Abdeckung aller 19 UI-Sprachen bei neuen nutzersichtbaren Texten beibehalten.
10. Optionale Verlaufspersistenz evaluieren, während die Standarderfahrung leichtgewichtig und local-first bleibt.

## Wie Codex diesem Projekt helfen kann

DirOtter eignet sich gut für KI-gestützte Open-Source-Wartung, weil das Projekt eine reale Multi-Crate-Rust-Codebasis, sicherheitssensibles Dateisystemverhalten, plattformübergreifende Ziele und laufende Wartungslast hat.

Mögliche Codex-unterstützte Open-Source-Wartungsaufgaben sind:

- Rust-Änderungen im Workspace prüfen
- Issues triagieren und Bugs reproduzieren
- Testabdeckung für Scan-, Aufräum-, Dubletten- und Reporting-Logik verbessern
- Aufräum-Sicherheitsregeln auditieren
- plattformspezifische Grenzfälle prüfen
- CI- und Release-Workflows verbessern
- Dokumentationsänderungen erstellen und prüfen
- Übersetzungskonsistenz pflegen helfen
- Pull-Request-Zusammenfassungen und Release Notes entwerfen

Codex-Unterstützung würde helfen, das Projekt vollständig open source zu halten und zugleich die Wartungslast zu reduzieren, die nötig ist, um DirOtter sicherer, zuverlässiger und auf mehreren Plattformen nützlicher zu machen.

## Beitragen

Beiträge sind willkommen.

Nützliche Beitragsbereiche sind:

- Performance beim Dateisystemscan
- Sicherheitsregeln für Aufräumen
- UX der Dublettenprüfung
- Windows-Papierkorbverhalten
- Linux- und macOS-Unterstützung
- UI-Tests
- visuelle Regressionstests
- Barrierefreiheitsverbesserungen
- Dokumentation
- Übersetzungen
- Packaging- und Release-Automatisierung

Bitte führe vor einer Pull Request die vollständige Qualitätsprüfung aus:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Detailliertere Contributor-Dokumentation sollte in `CONTRIBUTING.md` ergänzt werden.

## Sicherheit

DirOtter arbeitet mit lokalen Dateisystemdaten und Aufräumoperationen, daher sind Sicherheit und Schutz vor Datenverlust wichtige Projektanliegen.

Bitte melde potenzielle Sicherheits- oder Datenverlustprobleme nach Möglichkeit privat. Eine dedizierte `SECURITY.md`-Richtlinie sollte den bevorzugten Meldekanal, unterstützte Versionen und den Offenlegungsprozess definieren.

Besonders wichtige Bereiche sind:

- unsicheres Löschverhalten
- falsche Klassifizierung von Hochrisikopfaden
- Probleme mit symbolischen Links oder Junction-Traversal
- Berechtigungsgrenzen
- plattformspezifische Papierkorb-/Trash-Fehler
- Bugs bei irreversibler Löschung
- falsche Aufräumempfehlungen

## Datenschutz

DirOtter ist local-first.

Die Anwendung ist dafür ausgelegt, lokale Dateisystem-Metadaten zu analysieren, ohne standardmäßig Scan-Ergebnisse, Dateipfade oder Aufräumempfehlungen an einen Cloud-Dienst hochzuladen.

Künftige Telemetrie oder Crash-Reports sollten opt-in, klar dokumentiert und datenschutzfreundlich sein.

## Lizenz

Der Workspace deklariert derzeit MIT als Projektlizenz in `Cargo.toml`. Vor breiterer Verteilung sollte eine `LICENSE`-Datei im Repository-Root ergänzt werden.

## Projektziel

DirOtter soll ein transparentes, local-first, quelloffenes Werkzeug zur Festplattenanalyse und -bereinigung werden, dem Nutzer vertrauen können.

Das Projekt priorisiert:

- Sicherheit vor aggressiver Bereinigung
- Erklärbarkeit vor undurchsichtiger Automatisierung
- lokale Verarbeitung vor Cloud-Abhängigkeit
- Wartbarkeit vor kurzfristiger Funktionsanhäufung
- plattformübergreifende Zuverlässigkeit vor plattformspezifischen Abkürzungen

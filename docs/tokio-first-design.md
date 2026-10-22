# Erstes Design: Tokio / `std`

## Ziel

Der erste nutzbare Prototyp läuft auf Tokio. Er validiert das Actor-Modell mit Makros, statischen Routen und begrenzten Mailboxen. Es gibt keine dynamische Registry und keine `Box<dyn Future>`. Tokio darf intern Ressourcen allokieren; heap-freie Embassy-Ausführung ist ein späteres Ziel.

## Begriffe

- **Actor:** State mit exklusivem Zugriff, verarbeitet eingehende Nachrichten seriell.
- **Command:** Nachricht mit genau einem Ziel-Actor, gesendet über `send`.
- **Event:** Nachricht mit null oder mehreren Subscriber-Actors, verbreitet über `publish`.
- **Actor-Modul:** Wiederverwendbares Manifest, das Actor-Instanzen, Mailbox-Kapazitäten und Command-/Event-Routen beschreibt.
- **Actor-System:** Komposition expliziter Module mit generiertem typisiertem Dispatcher.

## Vorgeschlagene Nutzung

Nachrichtentypen sind gewöhnliche Rust-Typen. Die Manifest-Sektion legt fest, ob der Typ Command oder Event ist. Es gibt kein vom Nutzer definiertes Nachrichten-Enum. Actor-State wird beim Systemstart über den generierten Builder eingesetzt, nicht im wiederverwendbaren Manifest.

```rust
pub struct GetAnalogInput {
    pub channel: u8,
}

#[derive(Clone)]
pub struct AnalogValueChanged {
    pub channel: u8,
    pub value: u16,
}

pub struct SensorActor {
    adc: Adc,
}

async fn get_analog_input(actor: &mut SensorActor, command: GetAnalogInput) {
    let _value = actor.adc.read(command.channel).await;
}

async fn record_analog_value(actor: &mut TelemetryActor, event: AnalogValueChanged) {
    actor.record(event).await;
}

actor_module! {
    pub actor sensor: SensorActor {
        mailbox_capacity: 8;
        commands { GetAnalogInput => get_analog_input; }
        events {}
    };
}

actor_module! {
    pub actor telemetry: TelemetryActor {
        mailbox_capacity: 4;
        commands {}
        events { AnalogValueChanged => record_analog_value; }
    };
}

actor_system! {
    pub struct AppActors {
        actors: [sensors, sensors_telemetry];
    }
}

#[tokio::main]
async fn main() -> Result<(), StartError> {
    let actors = AppActors::builder()
        .sensor(SensorActor { adc: Adc::new() })
        .telemetry(TelemetryActor::new())
        .start()?;

    actors.send(GetAnalogInput { channel: 0 }).await?;
    actors.publish(AnalogValueChanged { channel: 0, value: 742 }).await?;
    Ok(())
}
```

Das Beispiel zeigt die implementierte API-Syntax; konkrete App-Typen und Nachrichtentypen sind dabei exemplarisch.

## Makro-Aufgaben

### `actor_module!`

Jeder `actor_module!`-Aufruf deklariert genau einen Actor mit `actor: name: State { … };`. Das Manifest ist die einzige Quelle der Wahrheit für dessen Routen, State-Typ, explizite Mailbox-Kapazität und freie Handler-Funktionen. Mehrere Actoren werden ausschließlich über die Manifestliste in `actor_system!` komponiert. Die [Migration](actor-module-migration.md) beschreibt die Umstellung. Ein Handler hat die Form `async fn handler(state: &mut ActorState, message: Message) -> ()` und wird im Manifest als `Message => handler` eingetragen. Handler-Kommunikation wird separat entworfen.

Das Makro erzeugt intern pro Actor einen Nachrichten-Enum und eine bounded Mailbox. Der Enum ist Implementierungsdetail: Nutzer müssen ihn nicht schreiben oder benennen. Die Manifest-Einträge liefern die Zuordnung der öffentlichen Nachrichtentypen zu den internen Mailbox-Varianten und Handlern.

Der Makro-Output stellt `actor_system!` ein statisches Manifest-Makro bereit. Systeme komponieren die Manifeste explizit; es gibt keine implizite Registrierung.

### `actor_system!`

Das System komponiert eine explizite Modulliste, besitzt die Actor-Handles und erzeugt `send(message)` sowie `publish(event)`-Routen:

- Jeder Command-Typ muss genau einen Handler in den ausgewählten Modulen haben.
- Ein Event-Typ darf null oder mehrere Handler haben.
- Doppelte Command-Routen müssen einen Compile-Fehler ergeben.
- Event ohne Subscriber ist ein erfolgreicher No-op.

Es gibt keine Registry per `HashMap`, String oder Laufzeit-ID. Die Routen sind statisch und typisiert.

### `Command` und `Event`

Keine Marker-derive-Makros sind nötig: die Manifest-Sektionen bestimmen die Routing-Art. Events müssen in V1 `Clone` sein, weil pro Subscriber ein Wert in dessen Mailbox gelegt wird; Commands werden einmal bewegt. Handler müssen in V1 `()` zurückgeben; andere Rückgabetypen werden durch den generierten Aufruf abgewiesen.

## Handler-Kommunikation

Handler-Kommunikation ist noch nicht Teil der API. Anwendungscode kann Nachrichten über das von `actor_system!` erzeugte System senden oder veröffentlichen. Ein separates Design für crate-übergreifende Actor-Kommunikation wird in [#22](https://github.com/klab365/actor-framework-rs/issues/22) verfolgt.

## Mailbox- und Fehlersemantik (Tokio)

Pro Actor wird `tokio::sync::mpsc::channel(N)` verwendet.

| Operation | Verhalten |
| --- | --- |
| `send(command).await` | Wartet bei voller Ziel-Mailbox; Fehler bei geschlossenem Empfänger. |
| `try_send(command)` | Wartet nicht; gibt bei voller/geschlossener Mailbox die Nachricht zurück. |
| `publish(event).await` | Klont für alle Subscriber und wartet, bis alle die Nachricht akzeptieren; Fehler, falls ein Empfänger geschlossen ist. |
| Event ohne Subscriber | Erfolgreicher No-op. |
| Actor-Funktion endet | Empfänger wird geschlossen; spätere Sends schlagen fehl. |
| `shutdown().await` | Verhindert neue Sends, lässt den gerade laufenden Handler beenden und wartet auf alle Actor-Tasks. Noch wartende Mailbox-Nachrichten werden beim Beenden verworfen. |

`publish` kann durch einen langsamen Subscriber blockieren. Das ist die verlustfreie V1-Semantik. Best-effort oder `try_publish` wären separate APIs.

## Instanziierung und Lifecycle

Der generierte Builder erhält konkrete State-Instanzen (`SensorActor::new(...)` oder Werte). `shutdown().await` signalisiert allen Tasks das Ende und wartet, bis laufende Handler abgeschlossen sind. Neue Sends schlagen nach dem Shutdown fehl; noch wartende Mailbox-Nachrichten werden verworfen. Panics oder unerwartet beendete Tasks werden als `ActorTaskError` mit dem Actor-Feldnamen zurückgegeben. Wird das System ohne expliziten Shutdown gedroppt, signalisiert sein `Drop` ebenfalls das Ende, kann aber naturgemäß weder auf Tasks warten noch Panics melden.

V1 umfasst keinen Restart/Supervision, Timer oder Request/Response.

## Plattformgrenzen

Tokio-Actors benötigen die `Send + 'static`-Grenzen von `tokio::spawn`. Diese Bounds dürfen nicht unbesehen in gemeinsame Actor-APIs übernommen werden; Embassy kann lokale `!Send`-Tasks nutzen. Makro-Expansion und konkreter Spawn-Code sind runtime-spezifisch, während Nachrichtentypen, Manifeste und Routingregeln geteilt werden sollen.

## Prototypstatus

Ein Tokio-Vertical-Slice mit `actor_module!`-Manifesten, expliziter Mehrmodul-Komposition über `actor_system!`  ist implementiert. Builder nehmen die konkreten State-Instanzen entgegen. Die Crate-Grenze ist mit `examples/module-lib` und `examples/module-app` geprüft: eine Library exportiert ein context-freies Manifest; Module exportieren ihre Manifeste ohne Handler-Kommunikations-API. Integrationstests unter `src/actor-framework/tests/actor_module.rs` decken Modul-Komposition, Command-Orchestrierung, Fan-out, Events ohne Subscriber und fehlenden State ab.

Compile-Fail-Tests in `src/actor-framework/tests/ui/` prüfen doppelte Command-Routen und den Versuch, einen unbekannten Command zu senden. Der unbekannte Command schlägt derzeit beim Typabgleich fehl; eine gezieltere Diagnose wäre eine DX-Verbesserung.

`try_send` gibt die ursprüngliche Nachricht bei voller oder geschlossener Mailbox zurück. Integrationstests prüfen volle Mailbox, das Schließen nach Ende eines Actor-Tasks sowie `shutdown()` während eines laufenden Handlers und die Meldung eines Actor-Panics. `publish` sendet sequenziell in Manifest-Reihenfolge. Bei einem geschlossenen Subscriber gibt es einen Fehler; bereits vorher akzeptierte Zustellungen werden nicht zurückgerollt und spätere Subscriber erhalten das Event nicht. Diese partielle Zustellung ist eine bewusste Konsequenz der V1-Implementierung und muss vor Stabilisierung der API entweder so bestätigt oder durch anderes Verhalten ersetzt werden.

Events ohne Subscriber werden bei einem Actor mit einer leeren Route wie `events { Event => []; }` registriert. Der Dispatcher erzeugt dafür einen erfolgreichen `publish`-No-op.

## Runtime-neutraler Core (Issue #4)

Die gemeinsamen Routing-Verträge sind jetzt unabhängig von Tokio verfügbar;
`Actor`, `Receiver`, `run` und eine inline gespeicherte `Mailbox<M, N>` definieren
den allocatorfreien Integrationspfad. Siehe [Architektur](architecture.md).
Die Tokio-Makros und ihre Shutdown-/Fan-out-Semantik bleiben unverändert und
werden nur mit dem Feature `tokio` exportiert. Der gemeinsame `run`-Loop drainiert
bis zum Ende; er ersetzt nicht den Tokio-Loop mit dessen explizitem Shutdown.

## Nicht in V1

- WASM-Runtime-Adapter
- Generierte Embassy-Systeme (der manuelle [Embassy-Prototyp](embassy.md)
  unterstützt inzwischen heap-freien Task-/Mailbox-Speicher)
- Request/Response
- verzögerte Nachrichten und Timer
- Supervision/Restart
- dynamische Actor-Registrierung oder globaler Name-/ID-Lookup

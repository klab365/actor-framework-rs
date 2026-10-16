# Wann sind Actors sinnvoll?

Das Actor-Modell ist ein Architekturmuster für nebenläufige Systeme: Jeder Actor besitzt seinen eigenen Zustand, verarbeitet Nachrichten nacheinander und kommuniziert mit anderen Actors über Nachrichten statt über gemeinsam veränderlichen Zustand. Es ist kein Selbstzweck und nicht automatisch schneller oder einfacher als normale Funktionen.

## Actors passen gut, wenn …

- **Zustand klar abgegrenzt ist.** Eine Komponente verwaltet beispielsweise eine Verbindung, einen Gerätekanal oder eine Session. Andere Teile des Programms greifen nicht direkt auf ihren veränderlichen Zustand zu, sondern senden Befehle.
- **Viele unabhängige Abläufe gleichzeitig aktiv sind.** Verbindungen, Sessions oder Geräte können jeweils unabhängig Nachrichten entgegennehmen und weiterarbeiten.
- **Ereignisse und Befehle asynchron eintreffen.** Eingaben aus Netzwerk, UI, Sensoren, Timer oder Interrupt-nahen Queues lassen sich als Nachrichten modellieren.
- **Komponenten entkoppelt werden sollen.** Sender müssen die interne Implementierung des Empfängers nicht kennen. Austauschbare Transport- und Laufzeitadapter können dieselben fachlichen Nachrichten verarbeiten.
- **Fehler und Lebenszyklen isoliert behandelt werden sollen.** Ein Actor kann beendet, neu gestartet oder kontrolliert heruntergefahren werden, ohne dass andere Komponenten direkten Zugriff auf seinen Zustand haben.
- **Backpressure wichtig ist.** Begrenzte Mailboxen machen sichtbar, wenn Verbraucher nicht mit der Nachrichtenrate Schritt halten, sofern das Framework Überlauf- und Sendeverhalten explizit festlegt.

## Typische Einsatzfälle

| Bereich | Mögliche Actors | Warum das Modell hilft |
| --- | --- | --- |
| Netzwerkserver | Actor pro Verbindung oder Session | Verbindungszustand bleibt lokal; Nachrichten serialisieren Zustandsänderungen. |
| Geräte und IoT | Actor pro Gerät, Sensor oder Buskanal | Unabhängige Geräteabläufe und Ereignisverarbeitung lassen sich isolieren. |
| UI und Anwendungen | Actor pro Dokument, Fenster oder Hintergrundaufgabe | Lange Aufgaben und UI-Ereignisse können über Nachrichten koordiniert werden. |
| Echtzeitnahe Systeme | Actor pro Steuerungsfunktion oder Ressource | Zuständigkeiten und Nachrichtenflüsse sind explizit; für harte Echtzeit gelten zusätzliche Anforderungen. |
| Verteilte Systeme | Actor pro Partition, Sitzung oder Service-Instanz | Das lokale Nachrichtenmodell kann eine Grundlage für spätere Verteilung sein, ersetzt aber kein verteiltes Protokoll. |

## Wann Actors eher nicht sinnvoll sind

- Für einfache, synchrone Berechnungen ohne geteilten Zustand: Ein Funktionsaufruf ist meist klarer und günstiger.
- Wenn zwei Schritte unmittelbar aufeinander warten und keine unabhängige Arbeit möglich ist: Nachrichten und Mailboxen schaffen dann unnötige Komplexität.
- Wenn Daten groß und unveränderlich sind und problemlos geteilt werden können: Kopieren oder Weiterreichen vieler Nachrichten kann teuer sein.
- Wenn gemeinsame Transaktionen über mehrere Zustände erforderlich sind: Actor-Grenzen machen atomare Änderungen nicht automatisch möglich.
- Wenn extrem niedrige Latenz oder deterministische Ausführung gefordert ist, ohne dass Mailboxen, Scheduler und Speicherverhalten darauf ausgelegt sind.

## Was Actors nicht automatisch lösen

Actors verhindern nicht alle Fehler: Nachrichten können verspätet eintreffen, Sender können scheitern, Mailboxen können voll werden und ein Empfänger kann dauerhaft überlastet sein. Neustartstrategien, Nachrichtenreihenfolge, Zustellgarantien, Timeout-Verhalten und Shutdown müssen bewusst definiert werden. Insbesondere bedeutet eine erfolgreiche Zustellung an eine Mailbox nicht zwingend, dass die Nachricht fachlich verarbeitet oder dauerhaft gespeichert wurde.

## Bedeutung für mehrere Rust-Laufzeiten

Ein Framework für Tokio/std, WebAssembly und Embassy sollte das Actor-Modell von der Ausführung trennen. Fachliche Actor-Logik und Nachrichten sollten möglichst laufzeitneutral bleiben; Scheduling, Timer, Synchronisation und Mailbox-Transport können je Plattform variieren. Tokio/std eignet sich für allgemeine asynchrone Server und Anwendungen, WASM für Browser- oder eingebettete Web-Ausführung mit deren Einschränkungen, Embassy für ressourcenknappe Embedded-Systeme. Ein gemeinsames API sollte dabei nicht voraussetzen, dass jede Plattform Threads, `std`, dynamische Allokation oder dieselben Async-Primitiven unterstützt.

## Faustregel

Actors lohnen sich, wenn eine Komponente **eigenen Zustand besitzt**, **unabhängig Nachrichten verarbeitet** und eine **klare Nebenläufigkeits- oder Isolationsgrenze** bildet. Gibt es diese Grenze nicht, zunächst bei gewöhnlichen Funktionen und Datenstrukturen bleiben.

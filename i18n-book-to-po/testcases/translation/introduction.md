# Kapitel 1: Einführung in Markdown

Dies ist das erste Kapitel unseres Beispielbuchs. Es zeigt, was dieses Tool
kann.

## Unterkapitel

Alle Seiten dieses Buches sind in Markdown geschrieben und kann verschiedene
Markdown Elemente enthalten:

> Dieser Einschub der nur in der Übersetzung zu finden ist, wird ignoriert

- Punkt 1
- Punkt 2
  - Unterpunkt 2.1
- Punkt 3

Sie können auch problemlos Links wie [Anhang][appendix] zu anderen Ressourcen
einfügen.

## Codeblöcke

Das Anzeigen von Code ist unkompliziert. Hier ist ein Beispiel in Rust:

```rust
fn gruessen(name: &str) {
    println!("Hallo, {}!", name);
}

fn main() {
    gruessen("Welt");
}
```

### Weitere Beispiele

Sie können **fettgedruckten Text**, _kursiven Text_ oder sogar
`inline Code Blöcke` verwenden.

[appendix]: some-appendix.html

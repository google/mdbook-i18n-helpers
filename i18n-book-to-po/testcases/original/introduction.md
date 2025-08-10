# Chapter 1: Introduction to Markdown

This is the first chapter of our example book. It shows what this tool can do.

## Subchapter

All pages of this book are written in Markdown and can contain different
Markdown elements:

- Item 1
- Item 2
  - Sub-item 2.1
- Item 3

You can also include links like [appendix][appendix] to other resources easily

## Code Blocks

Displaying code is straightforward. Here's an example in Rust:

```rust
fn greet(name: &str) {
    println!("Hello, {}!", name);
}

fn main() {
    greet("World");
}
```

## Untranslated sub chapter

This entire subchapter is not translated but the tool can deal with it and align
everything properly.

### Further Examples

You can use **bold text**, _italic text_, or even `inline code blocks`.

[appendix]: some-appendix.html

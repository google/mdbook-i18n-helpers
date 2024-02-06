use arbitrary::Arbitrary;
use mdbook::book::{Book, Chapter};
use polib::catalog::Catalog;
use polib::message::Message;
use polib::metadata::CatalogMetadata;
use std::path::PathBuf;

/// Generate a random Catalog for fuzzing.
pub fn create_catalog(translations: Vec<(&str, &str)>) -> Catalog {
    let mut catalog = Catalog::new(CatalogMetadata::new());
    for (idx, (msgid, msgstr)) in translations.iter().enumerate() {
        let message = Message::build_singular()
            .with_source(format!("foo.md:{idx}"))
            .with_msgid(String::from(*msgid))
            .with_msgstr(String::from(*msgstr))
            .done();
        catalog.append_or_update(message);
    }
    catalog
}

/// Generate a random Book for fuzzing.
pub fn create_book(book_items: Vec<BookItem>) -> Book {
    let mut book = Book::new();
    for item in book_items.into_iter() {
        book.push_item(item);
    }
    book
}

/// Wrapper enum for generating arbitrary `BookItem`s.
#[derive(Arbitrary, Debug)]
pub enum BookItem {
    Chapter { name: String, content: String },
    Separator,
    PartTitle(String),
}

impl From<BookItem> for mdbook::book::BookItem {
    fn from(other: BookItem) -> mdbook::book::BookItem {
        match other {
            BookItem::Chapter { name, content } => mdbook::book::BookItem::Chapter(Chapter::new(
                &name,
                content,
                PathBuf::new(),
                Vec::new(),
            )),
            BookItem::Separator => mdbook::book::BookItem::Separator,
            BookItem::PartTitle(title) => mdbook::book::BookItem::PartTitle(title),
        }
    }
}

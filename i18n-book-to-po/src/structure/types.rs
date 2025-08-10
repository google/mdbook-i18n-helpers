use clap::ValueEnum;
use pulldown_cmark::{BlockQuoteKind, HeadingLevel, LinkType, MetadataBlockKind};

/// copy of pulldown_cmark::Tag without any data
#[derive(Debug, PartialEq, Clone)]
pub enum CmarkTagStart {
    Paragraph,
    Heading { level: HeadingLevel },
    BlockQuote(Option<BlockQuoteKind>),
    CodeBlock,
    HtmlBlock,
    List(Option<u64>),
    Item,
    FootnoteDefinition,
    Table,
    TableHead,
    TableRow,
    TableCell,
    Emphasis,
    Strong,
    Strikethrough,
    // link type might be worthwhile but this could also be different
    // skipping for now
    // Link { link_type: LinkType },
    Link,
    Image { link_type: LinkType },
    MetadataBlock(MetadataBlockKind),
    DefinitionList,
    DefinitionListTitle,
    DefinitionListDefinition,
    Superscript,
    Subscript,
}

impl From<&pulldown_cmark::Tag<'_>> for CmarkTagStart {
    fn from(value: &pulldown_cmark::Tag) -> Self {
        match value {
            pulldown_cmark::Tag::Paragraph => Self::Paragraph,
            pulldown_cmark::Tag::Heading { level, .. } => Self::Heading { level: *level },
            pulldown_cmark::Tag::BlockQuote(kind) => Self::BlockQuote(*kind),
            pulldown_cmark::Tag::CodeBlock(..) => Self::CodeBlock,
            pulldown_cmark::Tag::HtmlBlock => Self::HtmlBlock,
            pulldown_cmark::Tag::List(number) => Self::List(*number),
            pulldown_cmark::Tag::Item => Self::Item,
            pulldown_cmark::Tag::FootnoteDefinition(..) => Self::FootnoteDefinition,
            pulldown_cmark::Tag::Table(..) => Self::Table,
            pulldown_cmark::Tag::TableHead => Self::TableHead,
            pulldown_cmark::Tag::TableRow => Self::TableRow,
            pulldown_cmark::Tag::TableCell => Self::TableCell,
            pulldown_cmark::Tag::Emphasis => Self::Emphasis,
            pulldown_cmark::Tag::Strong => Self::Strong,
            pulldown_cmark::Tag::Strikethrough => Self::Strikethrough,
            pulldown_cmark::Tag::Link { .. } => Self::Link,
            pulldown_cmark::Tag::Image { link_type, .. } => Self::Image {
                link_type: *link_type,
            },
            pulldown_cmark::Tag::MetadataBlock(kind) => Self::MetadataBlock(*kind),
            pulldown_cmark::Tag::DefinitionList => Self::DefinitionList,
            pulldown_cmark::Tag::DefinitionListTitle => Self::DefinitionListTitle,
            pulldown_cmark::Tag::DefinitionListDefinition => Self::DefinitionListDefinition,
            pulldown_cmark::Tag::Superscript => Self::Superscript,
            pulldown_cmark::Tag::Subscript => Self::Subscript,
        }
    }
}

/// copy of pulldown_cmark::TagEnd without any data
#[derive(Debug, PartialEq, Clone)]
pub enum CmarkTagEnd {
    Paragraph,
    Heading(HeadingLevel),
    BlockQuote(Option<BlockQuoteKind>),
    CodeBlock,
    HtmlBlock,
    List,
    Item,
    FootnoteDefinition,
    Table,
    TableHead,
    TableRow,
    TableCell,
    Emphasis,
    Strong,
    Strikethrough,
    Link,
    Image,
    MetadataBlock(MetadataBlockKind),
    DefinitionList,
    DefinitionListTitle,
    DefinitionListDefinition,
    Superscript,
    Subscript,
}

impl From<&pulldown_cmark::TagEnd> for CmarkTagEnd {
    fn from(value: &pulldown_cmark::TagEnd) -> Self {
        match value {
            pulldown_cmark::TagEnd::Paragraph => Self::Paragraph,
            pulldown_cmark::TagEnd::Heading(heading_level) => Self::Heading(*heading_level),
            pulldown_cmark::TagEnd::BlockQuote(kind, ..) => Self::BlockQuote(*kind),
            pulldown_cmark::TagEnd::CodeBlock => Self::CodeBlock,
            pulldown_cmark::TagEnd::HtmlBlock => Self::HtmlBlock,
            pulldown_cmark::TagEnd::List(_) => Self::List,
            pulldown_cmark::TagEnd::Item => Self::Item,
            pulldown_cmark::TagEnd::FootnoteDefinition => Self::FootnoteDefinition,
            pulldown_cmark::TagEnd::Table => Self::Table,
            pulldown_cmark::TagEnd::TableHead => Self::TableHead,
            pulldown_cmark::TagEnd::TableRow => Self::TableRow,
            pulldown_cmark::TagEnd::TableCell => Self::TableCell,
            pulldown_cmark::TagEnd::Emphasis => Self::Emphasis,
            pulldown_cmark::TagEnd::Strong => Self::Strong,
            pulldown_cmark::TagEnd::Strikethrough => Self::Strikethrough,
            pulldown_cmark::TagEnd::Link => Self::Link,
            pulldown_cmark::TagEnd::Image => Self::Image,
            pulldown_cmark::TagEnd::MetadataBlock(kind) => Self::MetadataBlock(*kind),
            pulldown_cmark::TagEnd::DefinitionList => Self::DefinitionList,
            pulldown_cmark::TagEnd::DefinitionListTitle => Self::DefinitionListTitle,
            pulldown_cmark::TagEnd::DefinitionListDefinition => Self::DefinitionListDefinition,
            pulldown_cmark::TagEnd::Superscript => Self::Superscript,
            pulldown_cmark::TagEnd::Subscript => Self::Subscript,
        }
    }
}

/// copy of pulldown_cmark::Event without data
#[derive(Debug, PartialEq, Clone)]
pub enum CmarkEvent {
    Start(CmarkTagStart),
    End(CmarkTagEnd),
    Text,
    Code,
    Html,
    InlineHtml,
    FootnoteReference,
    SoftBreak,
    HardBreak,
    Rule,
    TaskListMarker,
    InlineMath,
    DisplayMath,
    /// custom variant to support more (paragraph) internal structure
    SentenceElement,
}

impl From<&pulldown_cmark::Event<'_>> for CmarkEvent {
    fn from(value: &pulldown_cmark::Event) -> Self {
        match value {
            pulldown_cmark::Event::Start(start_tag) => Self::Start(start_tag.into()),
            pulldown_cmark::Event::End(end_tag) => Self::End(end_tag.into()),
            pulldown_cmark::Event::Text(_) => Self::Text,
            pulldown_cmark::Event::Code(_) => Self::Code,
            pulldown_cmark::Event::Html(_) => Self::Html,
            pulldown_cmark::Event::InlineHtml(_) => Self::InlineHtml,
            pulldown_cmark::Event::FootnoteReference(_) => Self::FootnoteReference,
            pulldown_cmark::Event::SoftBreak => Self::SoftBreak,
            pulldown_cmark::Event::HardBreak => Self::HardBreak,
            pulldown_cmark::Event::Rule => Self::Rule,
            pulldown_cmark::Event::TaskListMarker(_) => Self::TaskListMarker,
            pulldown_cmark::Event::InlineMath(_) => Self::InlineMath,
            pulldown_cmark::Event::DisplayMath(_) => Self::DisplayMath,
        }
    }
}

/// describes an action to modify the original documents.
/// Source(Event) indicates that Source has a Event element that is missing in the translation
/// Both(Event) indicates that both sides have the given element
#[derive(Debug, PartialEq)]
pub enum AlignAction {
    /// only available in the source
    Source(CmarkEvent),
    /// only available in the translation
    Translation(CmarkEvent),
    /// available in source and translation
    Both(CmarkEvent),
    /// this element seems to have changed in the translation
    Different(CmarkEvent, CmarkEvent),
}

/// Supported Diff algorithms
#[derive(Default, Clone, ValueEnum)]
pub enum DiffAlgorithm {
    Lcs,
    #[default]
    NeedlemanWunsch,
}

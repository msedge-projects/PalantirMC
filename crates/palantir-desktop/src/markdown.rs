//! A project's description: markdown, and the HTML a body is allowed to carry.
//!
//! `ProjectPageDescription.vue` is one line long and hands `project.body` to
//! `renderHighlightedString` from `@modrinth/utils`, which is markdown-it and
//! then an XSS filter. That filter is the reason HTML in a description is
//! *rendered* rather than shown: `packages/utils/parse.ts` whitelists `summary`
//! itself, and the list it extends already carries `details`, `kbd`, `iframe`,
//! `img` with a `usemap`, `map`, `area`, `picture`, `source` and a paragraph's
//! `align`. So a body that opens `<details>` is a disclosure on modrinth.com and
//! in the app, and a renderer that draws `<details>` as eight characters is not a
//! partial port of that renderer: it is drawing text the reference never shows.
//! The screenshot this module was written from had both halves of one defect on
//! one page — the tags as literal text, and every hanzi as a box.
//!
//! What is parsed here is what a project description actually holds: ATX
//! headings, paragraphs that soft-wrap, list items, fenced code, blockquotes,
//! rules, and the two elements the reference's own editor writes into a body, a
//! `<details>` and its `<summary>`. Every other tag is dropped and its text kept.
//! An HTML engine is not what this is, and the rule is the one that keeps a body
//! legible: a tag this launcher cannot draw must not become text either, or the
//! reader is left reading `<div align="center">` where the reference drew a
//! centred caption.
//!
//! The look is `.markdown-body` in `assets/styles/classes.scss` plus Tailwind's
//! preflight, and two of its rules surprise enough to be named:
//!
//! * **A heading is not bigger than a paragraph.** `.markdown-body` sets
//!   `h1, h2, h3, h4 { color: var(--color-contrast) }` and, for `h1` and `h2`,
//!   `padding: 10px 0 5px` with a `1px solid var(--color-divider)` under it — and
//!   nothing anywhere sets a heading's size or weight, because preflight resets
//!   both to `inherit` and no typography plugin is installed. So a `##` is
//!   body-sized text in the contrast ink with a rule beneath it, which is what
//!   this draws.
//! * **A list has no markers.** Preflight is `ol, ul, menu { list-style: none;
//!   margin: 0; padding: 0 }`, and `.markdown-body` adds nothing back, so `- one`
//!   is a line of text with no bullet and no indent. Faithful here means
//!   markerless, and an item is drawn as the line it is.
//!
//! Emphasis is *not* drawn. iced 0.12's `Text` is one font for one widget and has
//! no spans, so `**bold**` can only lose its markers or keep them; they are
//! stripped, because the words are the reference's and the asterisks are not, and
//! a slice that can set a run in another weight is what would draw them.

use std::collections::BTreeSet;

use iced::widget::{column, container, mouse_area, row, Space};
use iced::{Alignment, Background, Border, Element, Length, Padding, Theme};

use crate::icon;
use crate::icons_gen::Glyph;
use crate::style::{semibold, INK_CONTRAST, INK_DEFAULT, INK_SECONDARY};
use crate::theme_gen::{self, Ink, Span, Theme as Gen};
use crate::ui::{self, text};

/// The size a code block is set at: `pre code { font-size: 80% }` of the
/// paragraph around it.
const CODE_SIZE: f32 = 11.2;

/// The width of a blockquote's own rule: `border-left: 0.25em solid`, which is
/// four pixels at the reference's sixteen-pixel root.
const QUOTE_RULE: f32 = 4.0;

/// How far a blockquote's text is inset from its own bar: `padding: 0 1em`.
const QUOTE_INSET: f32 = 16.0;

/// One block of a description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    /// `# ` through `###### `, with the level it was written at.
    Heading {
        /// One to six, as written.
        level: u8,
        /// The heading's text, with no markup left.
        text: String,
    },
    /// A paragraph. Soft-wrapped lines are joined, which is what a renderer does
    /// with them.
    Paragraph(String),
    /// One list item. Its marker is not kept, because the reference draws none.
    Item(String),
    /// A fenced code block's contents, verbatim.
    Code(String),
    /// A blockquote, whose lines are joined the way a paragraph's are.
    Quote(String),
    /// `---`, `***` or `___`.
    Rule,
    /// A `<summary>` line. A [`Block::Details`] takes it as its caption; one
    /// written outside a disclosure is drawn as the line of text it is.
    Summary(String),
    /// A `<details>` and what it holds.
    Details {
        /// Which `<details>` of the body this is, counting from zero in document
        /// order.
        ///
        /// A name rather than a position, because it is what the page's open set
        /// is keyed by: it has to be the same number when a disclosure is
        /// collapsed as when it is open, which is why it is assigned by the parse
        /// rather than while drawing. A nested disclosure is numbered in the
        /// body's order too, and not in its parent's.
        number: usize,
        /// The `<summary>`'s text: the line a reader clicks. Empty when the body
        /// opened a `<details>` that has none.
        summary: String,
        /// What the disclosure holds, with the summary taken out of it.
        body: Vec<Block>,
    },
}

/// Parse a description into the blocks above.
pub fn parse(body: &str) -> Vec<Block> {
    let body = body.replace('\r', "");
    let lines: Vec<&str> = body.lines().map(str::trim_end).collect();
    let mut next_details = 0;
    let (blocks, _) = parse_from(&lines, 0, None, &mut next_details);
    blocks
}

/// Parse from `at`, stopping at `</closing>` when a tag name is given.
///
/// Answers the blocks and the index of the line after whatever stopped it — the
/// end of the body, or the line after the tag that closed a `<details>`.
fn parse_from(
    lines: &[&str],
    start: usize,
    closing: Option<&str>,
    next_details: &mut usize,
) -> (Vec<Block>, usize) {
    let mut blocks: Vec<Block> = Vec::new();
    let mut paragraph: Vec<String> = Vec::new();
    let mut at = start;

    while at < lines.len() {
        let line = lines[at];
        let trimmed = line.trim();

        if let Some(name) = closing {
            if is_closing_tag(trimmed, name) {
                // The caller resumes after the tag that ended this one.
                flush(&mut blocks, &mut paragraph);
                return (blocks, at + 1);
            }
        }

        // A fence runs to its own closing fence and its contents are text: what is
        // between the fences is not markdown, and `#` inside a fence is a comment
        // in some language rather than a heading.
        if let Some(fence) = fence(trimmed) {
            flush(&mut blocks, &mut paragraph);
            at += 1;
            let mut source: Vec<String> = Vec::new();
            while at < lines.len() && !lines[at].trim_start().starts_with(fence) {
                source.push(lines[at].to_string());
                at += 1;
            }
            at += 1; // the closing fence, or the end of the body
            blocks.push(Block::Code(source.join("\n")));
            continue;
        }

        if trimmed.is_empty() {
            flush(&mut blocks, &mut paragraph);
            at += 1;
            continue;
        }

        if is_opening_tag(trimmed, "details") {
            flush(&mut blocks, &mut paragraph);
            let number = *next_details;
            *next_details += 1;
            let (inner, after) = parse_from(lines, at + 1, Some("details"), next_details);
            let (captions, body): (Vec<Block>, Vec<Block>) =
                inner.into_iter().partition(|block| matches!(block, Block::Summary(_)));
            let summary = match captions.into_iter().next() {
                Some(Block::Summary(caption)) => caption,
                _ => String::new(),
            };
            blocks.push(Block::Details { number, summary, body });
            at = after;
            continue;
        }

        if let Some(inner) = summary_text(trimmed) {
            flush(&mut blocks, &mut paragraph);
            blocks.push(Block::Summary(inner));
            at += 1;
            continue;
        }

        // A line that holds nothing but HTML: a tag the reference would render and
        // this cannot draw. Dropped rather than shown.
        if is_tag_line(trimmed) {
            flush(&mut blocks, &mut paragraph);
            at += 1;
            continue;
        }

        if let Some((level, heading)) = heading(trimmed) {
            flush(&mut blocks, &mut paragraph);
            blocks.push(Block::Heading { level, text: inline(heading) });
            at += 1;
            continue;
        }

        if is_rule(trimmed) {
            flush(&mut blocks, &mut paragraph);
            blocks.push(Block::Rule);
            at += 1;
            continue;
        }

        if let Some(first) = quote(trimmed) {
            flush(&mut blocks, &mut paragraph);
            let mut quoted = vec![inline(first)];
            at += 1;
            while at < lines.len() {
                match quote(lines[at].trim()) {
                    Some(more) => {
                        quoted.push(inline(more));
                        at += 1;
                    }
                    None => break,
                }
            }
            blocks.push(Block::Quote(quoted.join(" ")));
            continue;
        }

        if let Some(first) = list_item(trimmed) {
            flush(&mut blocks, &mut paragraph);
            blocks.push(Block::Item(inline(first)));
            at += 1;
            continue;
        }

        paragraph.push(inline(trimmed));
        at += 1;
    }

    flush(&mut blocks, &mut paragraph);
    (blocks, at)
}

/// End the paragraph being collected, and answer the blocks.
///
/// One function rather than a call and a `return` at each of the nine places a
/// block ends, because "the paragraph in hand is a block too" is the one thing
/// every one of them has to remember.
fn flush(blocks: &mut Vec<Block>, paragraph: &mut Vec<String>) {
    if !paragraph.is_empty() {
        blocks.push(Block::Paragraph(paragraph.join(" ")));
        paragraph.clear();
    }
}

/// A fence marker: three or more backticks or tildes, as markdown writes them.
fn fence(line: &str) -> Option<&'static str> {
    if line.starts_with("```") {
        Some("```")
    } else if line.starts_with("~~~") {
        Some("~~~")
    } else {
        None
    }
}

/// `# ` through `###### `, as the level and the text after it.
///
/// The space is what makes a heading a heading: `#hashtag` is a hashtag, and this
/// refuses it for the same reason a renderer does.
fn heading(line: &str) -> Option<(u8, &str)> {
    let hashes = line.chars().take_while(|character| *character == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &line[hashes..];
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    // A closing run of hashes is the heading's own punctuation, not its text.
    Some((hashes as u8, rest.trim().trim_end_matches('#').trim_end()))
}

/// `---`, `***` or `___`: three or more of one of those, and nothing else.
fn is_rule(line: &str) -> bool {
    ['-', '*', '_']
        .iter()
        .any(|mark| line.len() >= 3 && line.chars().all(|character| character == *mark))
}

/// A blockquote's own text, with its `>` and one space taken off.
fn quote(line: &str) -> Option<&str> {
    let rest = line.strip_prefix('>')?;
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

/// A list item's own text: `- `, `* `, `+ `, or `1. ` and its friends.
///
/// The marker has to be followed by a space, which is what keeps `-5 degrees` and
/// `2.5 out of 10` out of this and in the paragraph they belong to.
fn list_item(line: &str) -> Option<&str> {
    for mark in ["- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(mark) {
            return Some(rest);
        }
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    line[digits..].strip_prefix(". ")
}

/// Whether a line opens the named element.
fn is_opening_tag(line: &str, name: &str) -> bool {
    tag_at(line, name).is_some_and(|(closing, rest)| {
        !closing && rest.starts_with(['>', ' ', '/'])
    })
}

/// Whether a line closes the named element.
fn is_closing_tag(line: &str, name: &str) -> bool {
    tag_at(line, name).is_some_and(|(closing, _)| closing)
}

/// The named tag at the start of a line, with the rest of the line after its name.
///
/// The name has to end where it ends: `<details>` is a details and `<detailsx>` is
/// not, which is why the character after the name is checked rather than the name
/// alone.
fn tag_at<'a>(line: &'a str, name: &str) -> Option<(bool, &'a str)> {
    let line = line.strip_prefix('<')?;
    let (closing, rest) = match line.strip_prefix('/') {
        Some(rest) => (true, rest),
        None => (false, line),
    };
    let rest = rest.strip_prefix(name)?;
    if rest.starts_with(|character: char| character.is_ascii_alphanumeric() || character == '-') {
        return None;
    }
    Some((closing, rest))
}

/// A line that holds nothing but HTML.
///
/// The whole line has to be tags and whitespace, which is what keeps a paragraph
/// that has a `<` in it — `a < b` — out of this and in the paragraph it is.
fn is_tag_line(line: &str) -> bool {
    if !line.starts_with('<') {
        return false;
    }
    let mut rest = line;
    while let Some(start) = rest.find('<') {
        if !rest[..start].trim().is_empty() {
            return false;
        }
        let Some(end) = rest[start..].find('>') else {
            return false;
        };
        rest = &rest[start + end + 1..];
    }
    rest.trim().is_empty()
}

/// The text of a `<summary>` line, with the element's own tags taken off.
fn summary_text(line: &str) -> Option<String> {
    let rest = line.strip_prefix("<summary")?;
    let rest = rest.split_once('>')?.1;
    let rest = rest.rsplit_once("</summary>").map_or(rest, |(inner, _)| inner);
    Some(inline(rest))
}

/// A line of markdown, as the text a reader should see.
///
/// Markdown's inline markup is stripped rather than drawn, and so is HTML's: here
/// the two are the same job, because iced's `Text` is one font for one widget and
/// has no span to set a run in another weight with. A link keeps its text and
/// loses its target, an image keeps the alt text it was written with, and an
/// entity becomes the character it names, because `&amp;` in a description is an
/// ampersand.
pub fn inline(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while !rest.is_empty() {
        if let Some(after) = line_break(rest) {
            out.push('\n');
            rest = after;
            continue;
        }
        // The marker has to be at the head of what is left, so that the prose in
        // front of a link is prose and not the link's own label.
        if let Some((label, after)) = markup(rest) {
            out.push_str(label);
            rest = after;
            continue;
        }
        let character = rest.chars().next().unwrap_or(' ');
        // A `<` that opens a letter, a `/` or a `!` is a tag; anything else is a
        // character a body is allowed to contain.
        if character == '<'
            && rest[1..].starts_with(|next: char| next.is_alphanumeric() || next == '/' || next == '!')
        {
            match rest.find('>') {
                Some(end) => {
                    rest = &rest[end + 1..];
                    continue;
                }
                None => {
                    out.push(character);
                    rest = &rest[1..];
                    continue;
                }
            }
        }
        out.push(character);
        rest = &rest[character.len_utf8()..];
    }
    strip_markers(&entity(&out))
}

/// A `<br>` at the head of what is left, and what follows it.
fn line_break(line: &str) -> Option<&str> {
    ["<br>", "<br/>", "<br />", "<br  />", "<BR>", "<BR/>", "<BR />"]
        .iter()
        .find_map(|tag| line.strip_prefix(tag))
}

/// A markdown image or link at the head of what is left, as its label and what
/// follows it.
///
/// The destination is dropped rather than drawn: a picture is a picture, and a URL
/// drawn as prose is the same defect as a tag drawn as prose.
fn markup(line: &str) -> Option<(&str, &str)> {
    let body = line
        .strip_prefix("![")
        .or_else(|| line.strip_prefix('['))?;
    let (label, after) = body.split_once("](")?;
    let (_, after) = after.split_once(')')?;
    Some((label, after))
}

/// Emphasis and code markers, which iced's single-font `Text` cannot draw.
///
/// A pair of asterisks is markup; a lone one — `2 * 3` — is an asterisk. `_` is
/// left alone unless it is doubled, because an underscore inside a word is far
/// more often part of a name than emphasis in a body that has no way to draw it.
fn strip_markers(line: &str) -> String {
    let paired = line.replace("**", "").replace("__", "").replace("~~", "").replace('`', "");
    if paired.matches('*').count() % 2 == 0 {
        paired.replace('*', "")
    } else {
        paired
    }
}

/// The entities a description is written with, as the characters they name.
fn entity(line: &str) -> String {
    line.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}

/// Draw a description, as the reference's `Card` holds it.
pub fn render<'a, Message, F>(
    theme: Gen,
    blocks: &[Block],
    open: &BTreeSet<usize>,
    toggle: F,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
    F: Fn(usize) -> Message + Copy + 'a,
{
    let mut content = column![].spacing(16.0).width(Length::Fill);
    for block in blocks {
        content = content.push(draw(theme, block, open, toggle));
    }
    content.into()
}

/// One block, drawn the way `.markdown-body` draws it.
fn draw<'a, Message, F>(
    theme: Gen,
    block: &Block,
    open: &BTreeSet<usize>,
    toggle: F,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
    F: Fn(usize) -> Message + Copy + 'a,
{
    match block {
        Block::Heading { level, text: heading } => {
            // `h1, h2, h3, h4 { color: var(--color-contrast) }`, and only the
            // first two carry `padding: 10px 0 5px` with a rule under them.
            let ink = if *level <= 4 { INK_CONTRAST } else { INK_DEFAULT };
            let line = text(heading.clone())
                .size(14.0)
                .font(semibold())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, ink)));
            if *level > 2 {
                let margin = theme_gen::span(Span::GapMd);
                return container(line)
                    .padding(Padding { top: margin, right: 0.0, bottom: margin, left: 0.0 })
                    .into();
            }
            column![]
                .width(Length::Fill)
                .spacing(5.0)
                .push(container(line).padding(Padding { top: 10.0, right: 0.0, bottom: 0.0, left: 0.0 }))
                .push(hairline(theme, 1.0, Ink::Divider))
                .into()
        }
        Block::Paragraph(paragraph) => ui::paragraph(theme, paragraph),
        // Preflight is `list-style: none` and nothing puts the markers back, so an
        // item is the line of text it is.
        Block::Item(item) => ui::paragraph(theme, item),
        // Never at the top of a body, but a `<summary>` written outside a
        // `<details>` is text the reference draws, so it is drawn here too.
        Block::Summary(summary) => ui::paragraph(theme, summary),
        // `blockquote { padding: 0 1em; color: var(--color-base);
        // border-left: 0.25em solid var(--color-button-bg); margin-inline: 0 }`.
        //
        // The rule is painted rather than laid out beside the text, which is how
        // it was written first: a `Length::Fill`-height sibling in a row with the
        // text. A row lays its children out before it knows how tall it will be,
        // so the child that fills the cross axis takes the row's own *maximum* --
        // and inside a page's scroll region that maximum is `f32::MAX`, because
        // iced measures scroll content with an unbounded axis so it can be taller
        // than the window. The card this body sits in is drawn with a rounded
        // corner, and a corner on an `f32::MAX`-tall rectangle is a path the
        // rasteriser cannot build: any project whose description quoted a line
        // panicked with `Build rounded rectangle path` in `iced_tiny_skia` before
        // a pixel of that page was drawn. Two containers give the same picture at
        // a height each of them takes from its own content: the outer one paints
        // the rule and holds the inner one back from its left edge, and the inner
        // one paints the card's own surface across everything that is left.
        Block::Quote(quoted) => container(
            container(ui::paragraph(theme, quoted))
                .width(Length::Fill)
                .padding(Padding {
                    top: 0.0,
                    right: QUOTE_INSET,
                    bottom: 0.0,
                    left: QUOTE_INSET,
                })
                .style(move |_theme: &Theme| container::Appearance {
                    // The card's own fill (`--surface-3`, `ui::card`), so what the
                    // rule frames is the card and not a second surface.
                    background: Some(Background::Color(theme_gen::ink(theme, Ink::Surface3))),
                    ..container::Appearance::default()
                }),
        )
        .width(Length::Fill)
        .padding(Padding { top: 0.0, right: 0.0, bottom: 0.0, left: QUOTE_RULE })
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(theme_gen::ink(theme, Ink::ButtonBg))),
            ..container::Appearance::default()
        })
        .into(),
        // `pre { bg-surface-2 rounded-xl p-4 }` with a `border-surface-5` hairline,
        // and `pre code { font-size: 80% }` inside it.
        Block::Code(source) => container(
            text(source.clone())
                .size(CODE_SIZE)
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
        )
        .width(Length::Fill)
        .padding(Padding::from(16.0))
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(theme_gen::ink(theme, Ink::Surface2))),
            border: Border {
                color: theme_gen::ink(theme, Ink::Surface5),
                width: 1.0,
                radius: theme_gen::span(Span::RadiusXl).into(),
            },
            ..container::Appearance::default()
        })
        .into(),
        Block::Rule => hairline(theme, 1.0, Ink::ButtonBg),
        Block::Details { number, summary, body } => {
            let expanded = open.contains(number);
            let marker = if expanded { Glyph::ChevronDown } else { Glyph::ChevronRight };
            let mut caption = row![]
                .spacing(6.0)
                .align_items(Alignment::Center)
                .push(icon::icon(marker, 16.0, theme_gen::ink(theme, INK_SECONDARY)));
            if !summary.is_empty() {
                // The caption is drawn as the heading its own editor usually puts
                // inside one, and the markdown markers are not drawn.
                let caption_text = summary.trim_start_matches('#').trim();
                caption = caption.push(
                    text(caption_text.to_string())
                        .size(14.0)
                        .font(semibold())
                        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
                );
            }
            let mut disclosure = column![]
                .spacing(16.0)
                .width(Length::Fill)
                .push(mouse_area(caption).on_release(toggle(*number)));
            if expanded {
                let mut inner = column![].spacing(16.0).width(Length::Fill);
                for block in body {
                    inner = inner.push(draw(theme, block, open, toggle));
                }
                disclosure = disclosure.push(inner);
            }
            disclosure.into()
        }
    }
}

/// A hairline in a token's ink: the reference's `border-bottom`, which iced has no
/// per-side border for, and its `hr`.
fn hairline<'a, Message: 'a>(theme: Gen, height: f32, ink: Ink) -> Element<'a, Message> {
    container(Space::with_width(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fixed(height))
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(theme_gen::ink(theme, ink))),
            ..container::Appearance::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The body of `Zombie Invade 100 Days` (`l9m9tuPN`), trimmed to the lines the
    /// two defects were measured on: markdown headings, a paragraph in Chinese,
    /// and the `<details>` the reader's screenshot drew as text.
    const BODY: &str = "# Zombie Invade 100 Days\n\n\
        ## 简介 (Simplified Chinese)\n\n\
        这是一个僵尸入侵的模组 &amp; it works.\n\n\
        ### 版本 v2.3 的改动\n\n\
        <details>\n<summary>更新日志</summary>\n\n\
        - 修复了一个崩溃\n- 调整了生成权重\n\n\
        </details>\n\n\
        <div align=\"center\">\n<img src=\"https://cdn.modrinth.com/x.png\">\n</div>\n";

    #[test]
    fn a_heading_keeps_its_level_because_the_rule_is_only_drawn_for_two_of_them() {
        let blocks = parse("# One\n\n## Two\n\n### Three\n\n#### Four");
        assert_eq!(
            blocks,
            vec![
                Block::Heading { level: 1, text: "One".to_string() },
                Block::Heading { level: 2, text: "Two".to_string() },
                Block::Heading { level: 3, text: "Three".to_string() },
                Block::Heading { level: 4, text: "Four".to_string() },
            ]
        );
    }

    #[test]
    fn a_details_is_a_disclosure_rather_than_its_own_source() {
        let blocks = parse(BODY);
        let (number, summary, body) = blocks
            .iter()
            .find_map(|block| match block {
                Block::Details { number, summary, body } => Some((*number, summary, body)),
                _ => None,
            })
            .expect("the body opens a `<details>`");
        assert_eq!(number, 0, "the first disclosure of a body is number zero");
        assert_eq!(summary, "更新日志", "the summary is the caption, not the tag");
        assert_eq!(
            body,
            &vec![
                Block::Item("修复了一个崩溃".to_string()),
                Block::Item("调整了生成权重".to_string()),
            ]
        );
        // And no block anywhere in the tree is the text of a tag.
        for block in &blocks {
            let drawn = format!("{block:?}");
            for tag in ["<details>", "<summary>", "</summary>", "<div"] {
                assert!(!drawn.contains(tag), "{tag} reached the renderer: {drawn}");
            }
        }
    }

    #[test]
    fn every_disclosure_is_numbered_in_the_body_s_order() {
        // The open set is keyed by these numbers, so they have to come from the
        // document rather than from drawing: a collapsed disclosure is not drawn
        // through, and a number that came from drawing would move when it opened.
        let blocks = parse(
            "<details>\n<summary>one</summary>\n\ntext\n\n</details>\n\n\
             <details>\n<summary>two</summary>\n\n\
             <details>\n<summary>three</summary>\n\nx\n\n</details>\n\n</details>",
        );
        let numbers: Vec<usize> = blocks
            .iter()
            .filter_map(|block| match block {
                Block::Details { number, .. } => Some(*number),
                _ => None,
            })
            .collect();
        assert_eq!(numbers, vec![0, 1]);
        let nested: Vec<usize> = match &blocks[1] {
            Block::Details { body, .. } => body
                .iter()
                .filter_map(|block| match block {
                    Block::Details { number, .. } => Some(*number),
                    _ => None,
                })
                .collect(),
            other => panic!("expected a disclosure, got {other:?}"),
        };
        assert_eq!(nested, vec![2], "a nested disclosure is numbered in the body's order");
    }

    #[test]
    fn a_body_with_no_html_still_parses_as_the_markdown_it_is() {
        let blocks = parse(
            "Intro line\nsecond line\n\n- one\n- two\n\n1. three\n\n> quoted\n> more\n\n```\ncode <b>kept</b>\n```\n\n---",
        );
        assert_eq!(
            blocks,
            vec![
                Block::Paragraph("Intro line second line".to_string()),
                Block::Item("one".to_string()),
                Block::Item("two".to_string()),
                Block::Item("three".to_string()),
                Block::Quote("quoted more".to_string()),
                Block::Code("code <b>kept</b>".to_string()),
                Block::Rule,
            ]
        );
    }

    #[test]
    fn a_marker_iced_cannot_draw_is_not_drawn_as_a_character() {
        // The inline half of the same rule: `**` and `[](…)` are markup, and the
        // one-font `Text` this port draws with has no way to set a run in another
        // weight, so the words stay and the punctuation goes.
        assert_eq!(inline("**bold** and *italic*"), "bold and italic");
        assert_eq!(inline("see [the wiki](https://example.com/x) now"), "see the wiki now");
        assert_eq!(inline("![a picture](https://example.com/a.png)"), "a picture");
        assert_eq!(inline("`code`"), "code");
        assert_eq!(inline("a &amp; b &lt;c&gt;"), "a & b <c>");
        assert_eq!(inline("one<br>two"), "one\ntwo");
        assert_eq!(inline("a < b"), "a < b", "a less-than is prose, not a tag");
        assert_eq!(inline("2 * 3"), "2 * 3", "a lone asterisk is an asterisk");
        assert_eq!(inline("snake_case_name"), "snake_case_name");
    }

    #[test]
    fn a_tag_that_is_not_a_disclosure_is_dropped_and_its_text_kept() {
        // The reference renders `<div align="center">` and `<img>`; this draws
        // neither, and neither may it draw them as text. What it can draw is the
        // paragraph inside one, which is why a tag line is dropped.
        let blocks = parse("<div align=\"center\">\nA caption.\n</div>\n\n<kbd>Ctrl</kbd>");
        assert_eq!(
            blocks,
            vec![
                Block::Paragraph("A caption.".to_string()),
                // `<kbd>` opens a line that is not only tags, so it is a paragraph
                // whose tags come off.
                Block::Paragraph("Ctrl".to_string()),
            ]
        );
    }

    #[test]
    fn prose_that_looks_like_markup_is_left_as_prose() {
        // The parse is line-based and a body is arbitrary text: a rule is three
        // markers and nothing else, a heading's `#` needs a space after it, and a
        // list marker needs its own space. Each of these is a line somebody wrote.
        let blocks = parse("#hashtag\n\n2.5 out of 10\n\n--\n\n-5 degrees\n\na > b");
        assert_eq!(
            blocks,
            vec![
                Block::Paragraph("#hashtag".to_string()),
                Block::Paragraph("2.5 out of 10".to_string()),
                Block::Paragraph("--".to_string()),
                Block::Paragraph("-5 degrees".to_string()),
                Block::Paragraph("a > b".to_string()),
            ]
        );
    }

    #[test]
    fn a_fence_ends_at_its_own_marker_rather_than_at_the_body() {
        // A code block is the one place a marker is text: a `#` inside a fence is
        // not a heading and a `-` is not an item.
        let blocks = parse("```\n# not a heading\n- not an item\n```\n\nAfter.");
        assert_eq!(
            blocks,
            vec![
                Block::Code("# not a heading\n- not an item".to_string()),
                Block::Paragraph("After.".to_string()),
            ]
        );
        // An unclosed fence runs to the end of the body rather than dropping it.
        assert_eq!(parse("```\nunclosed"), vec![Block::Code("unclosed".to_string())]);
    }
}

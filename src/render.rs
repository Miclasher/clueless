//! Markdown rendering into a GTK text buffer.

use gtk::pango;
use gtk::prelude::*;
use gtk4 as gtk;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

pub fn install_tags(buffer: &gtk::TextBuffer) {
    let table = buffer.tag_table();
    let add = |t: gtk::TextTag| {
        table.add(&t);
    };
    add(gtk::TextTag::builder()
        .name("you")
        .weight(700)
        .foreground("#8ab4f8")
        .build());
    add(gtk::TextTag::builder()
        .name("claude")
        .weight(700)
        .foreground("#f0a070")
        .build());
    add(gtk::TextTag::builder()
        .name("dim")
        .foreground("#9aa0aa")
        .scale(0.9)
        .build());
    add(gtk::TextTag::builder().name("error").foreground("#ff8a80").build());
    add(gtk::TextTag::builder().name("bold").weight(700).build());
    add(gtk::TextTag::builder()
        .name("italic")
        .style(pango::Style::Italic)
        .build());
    add(gtk::TextTag::builder().name("heading").weight(700).scale(1.15).build());
    add(gtk::TextTag::builder()
        .name("code")
        .family("monospace")
        .background("rgba(255,255,255,0.10)")
        .build());
    add(gtk::TextTag::builder()
        .name("block")
        .family("monospace")
        .paragraph_background("rgba(0,0,0,0.40)")
        .left_margin(14)
        .build());
}

pub fn append(buffer: &gtk::TextBuffer, text: &str, tags: &[&str]) {
    let mut end = buffer.end_iter();
    buffer.insert_with_tags_by_name(&mut end, text, tags);
}

fn newline(buffer: &gtk::TextBuffer) {
    let end = buffer.end_iter();
    if end.offset() > 0 && !end.starts_line() {
        append(buffer, "\n", &[]);
    }
}

/// Appends `source` at the end of the buffer.
pub fn markdown(buffer: &gtk::TextBuffer, source: &str) {
    let mut tags: Vec<&'static str> = Vec::new();
    // One entry per open list: the next number, or `None` for bullets.
    let mut lists: Vec<Option<u64>> = Vec::new();
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    for event in Parser::new_ext(source, options) {
        match event {
            Event::Start(tag) => match tag {
                Tag::Strong => tags.push("bold"),
                Tag::Emphasis => tags.push("italic"),
                Tag::Heading { .. } => {
                    newline(buffer);
                    tags.push("heading");
                }
                Tag::CodeBlock(_) => {
                    newline(buffer);
                    tags.push("block");
                }
                Tag::List(start) => {
                    newline(buffer);
                    lists.push(start);
                }
                Tag::Item => {
                    newline(buffer);
                    let indent = "  ".repeat(lists.len().saturating_sub(1));
                    let marker = match lists.last_mut() {
                        Some(Some(n)) => {
                            let m = format!("{n}. ");
                            *n += 1;
                            m
                        }
                        _ => "• ".to_string(),
                    };
                    append(buffer, &format!("{indent}{marker}"), &[]);
                }
                _ => {}
            },
            Event::End(end) => match end {
                TagEnd::Strong | TagEnd::Emphasis => {
                    tags.pop();
                }
                TagEnd::Heading(_) | TagEnd::CodeBlock => {
                    tags.pop();
                    newline(buffer);
                }
                TagEnd::List(_) => {
                    lists.pop();
                    newline(buffer);
                }
                TagEnd::Paragraph | TagEnd::Item | TagEnd::TableRow | TagEnd::TableHead => newline(buffer),
                TagEnd::TableCell => append(buffer, "   ", &[]),
                _ => {}
            },
            Event::Text(t) => append(buffer, &t, &tags),
            Event::Code(t) => {
                let mut with = tags.clone();
                with.push("code");
                append(buffer, &t, &with);
            }
            Event::Html(t) | Event::InlineHtml(t) => append(buffer, &t, &tags),
            // Chat replies mean their line breaks, so single ones are kept.
            Event::SoftBreak | Event::HardBreak => append(buffer, "\n", &tags),
            Event::Rule => {
                newline(buffer);
                append(buffer, "────────\n", &["dim"]);
            }
            _ => {}
        }
    }
    newline(buffer);
}

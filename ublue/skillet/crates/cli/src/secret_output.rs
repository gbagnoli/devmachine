//! Terminal presentation of public secret setup instructions.
use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use skillet_workstation::secrets::CheckReport;
use std::fmt::Write as _;

// Parse shared documentation using CommonMark, preserving destinations in
// plain output and emitting OSC 8 only for supported web links.
fn terminal_guide(guide: &str, hyperlinks: bool) -> String {
    let mut output = String::new();
    let mut link = None;
    for event in Parser::new(guide) {
        match event {
            Event::Start(Tag::Link { dest_url, .. }) => {
                let clickable = hyperlinks
                    && (dest_url.starts_with("https://") || dest_url.starts_with("http://"));
                if clickable {
                    let _ = write!(output, "{}", osc8::Hyperlink::new(&dest_url));
                }
                link = Some((dest_url, clickable));
            }
            Event::End(TagEnd::Link) => {
                if let Some((destination, clickable)) = link.take() {
                    if clickable {
                        let _ = write!(output, "{}", osc8::Hyperlink::END);
                    } else {
                        let _ = write!(output, ": {destination}");
                    }
                }
            }
            Event::Text(text) | Event::Code(text) => output.push_str(&text),
            Event::SoftBreak | Event::HardBreak | Event::End(TagEnd::Paragraph) => output.push(' '),
            _ => {}
        }
    }
    output.trim().to_string()
}

fn write_guide(output: &mut String, guide: &str, hyperlinks: bool) {
    let guide = terminal_guide(guide, hyperlinks);
    // Let the terminal wrap hyperlinks without counting invisible escapes or
    // splitting link control sequences. Plain output is explicitly wrapped.
    if hyperlinks {
        let _ = writeln!(output, "      {guide}");
        return;
    }
    let mut column = 6;
    output.push_str("      ");
    for word in guide.split_whitespace() {
        let length = word.chars().count();
        if column > 6 {
            if column + 1 + length > 88 {
                output.push_str("\n      ");
                column = 6;
            } else {
                output.push(' ');
                column += 1;
            }
        }
        output.push_str(word);
        column += length;
    }
    output.push('\n');
}

pub(super) fn render(report: &CheckReport, unused: &[String], hyperlinks: bool) -> String {
    let mut output = String::new();
    if !report.missing.is_empty() {
        output.push_str("Required entries needing attention\n");
        let mut previous_module = "";
        for entry in &report.missing {
            if previous_module != entry.module {
                let _ = write!(output, "\n{}\n", entry.module);
                previous_module = &entry.module;
            }
            let state = if entry.invalid {
                "INVALID (fields or ambiguous entry)"
            } else {
                "MISSING"
            };
            let _ = writeln!(output, "  {state}: {}", entry.path);
            output.push_str("    Setup:\n");
            write_guide(&mut output, &entry.guide, hyperlinks);
            output.push('\n');
        }
    }
    if !unused.is_empty() {
        output.push_str("\nUnused Skillet entries\n  Scope: all declared hosts, prod and test\n  Review these entries; nothing has been deleted.\n");
        for path in unused {
            let _ = writeln!(output, "  {path}");
        }
    }
    let _ = writeln!(
        output,
        "\nSummary: {} required checked, {} missing/invalid, {} unused.",
        report.checked,
        report.missing.len(),
        unused.len()
    );
    output
}

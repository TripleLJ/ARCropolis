use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct NotesEntry {
    pub section_title: String,
    pub contents: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Contributor {
    pub login: String,
    pub name: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct MainEntry {
    pub title: String,
    pub date: String,
    pub description: String,
    pub entries: Vec<NotesEntry>,
    #[serde(skip)]
    pub contributors: Vec<Contributor>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    Blank,
    Text(String),
    Header(String),
    Bullet(String),

    Continued(String),
    Contributor(Contributor),
}

pub const CREDIT_LINES: usize = 4;
const CREDITS: [&str; CREDIT_LINES] = [
    "ARCropolis, a modding framework for Super Smash Bros. Ultimate",
    "Maintainer(s): Raytwo, Blujay, Coolsonickirby, jozz",
    "Special thanks: Shadów, jam1garner, Genwald, Styley",
    "URL: http://www.arcropolis.com",
];

pub fn lines(entry: &MainEntry, columns: usize) -> Vec<Line> {
    let mut out = Vec::new();

    for paragraph in strip_html(&entry.description).lines() {
        for line in wrap(paragraph.trim(), columns) {
            out.push(Line::Text(line));
        }
    }
    out.push(Line::Blank);

    for section in &entry.entries {
        out.push(Line::Header(strip_html(&section.section_title).trim().to_string()));
        for item in list_items(&section.contents) {
            let mut wrapped = wrap(&item, columns.saturating_sub(2)).into_iter();
            if let Some(first) = wrapped.next() {
                out.push(Line::Bullet(first));
            }
            out.extend(wrapped.map(Line::Continued));
        }
        out.push(Line::Blank);
    }

    if !entry.contributors.is_empty() {
        out.push(Line::Header("Contributors".to_string()));
        out.extend(entry.contributors.iter().cloned().map(Line::Contributor));
        out.push(Line::Blank);
    }

    out.push(Line::Blank);
    out.extend(CREDITS.iter().map(|line| Line::Text(line.to_string())));
    out
}

fn list_items(contents: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut rest = contents;
    while let Some(start) = rest.find("<li>") {
        let after = &rest[start + 4..];
        let end = after.find("</li>").unwrap_or(after.len());
        let item = strip_html(&after[..end]).trim().to_string();
        if !item.is_empty() {
            items.push(item);
        }
        rest = &after[end..];
    }

    if items.is_empty() {
        let plain = strip_html(contents).trim().to_string();
        if !plain.is_empty() {
            items.push(plain);
        }
    }
    items
}

pub fn strip_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let after = &rest[open..];
        let Some(close) = after.find('>') else {
            out.push_str(after);
            rest = "";
            break;
        };
        let tag = after[1..close].trim().to_ascii_lowercase();
        if tag.starts_with("br") {
            out.push('\n');
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);

    out.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&amp;", "&")
}

pub fn wrap(text: &str, columns: usize) -> Vec<String> {
    let columns = columns.max(1);
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.chars().count() + 1 + word.chars().count() <= columns {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

pub fn from_release_markdown(text: &str) -> (Vec<Contributor>, Vec<NotesEntry>) {
    let mut entries = Vec::new();
    let mut logins: Vec<String> = Vec::new();
    let data: Vec<&str> = text.lines().collect();
    let mut i = 0;

    while i < data.len() {
        let Some(heading) = data[i].strip_prefix("### ") else {
            i += 1;
            continue;
        };

        let mut items = Vec::new();
        let mut y = i + 1;
        while y < data.len() && !data[y].is_empty() {
            let Some(mut line) = data[y].strip_prefix("* ") else {
                break;
            };
            if let Some(cut) = line.find("(@") {
                line = line[..cut].trim();
            }
            items.push(format!("<li>{}</li>", escape_html(line)));

            for part in data[y].split('@').skip(1) {
                let end = part.find([' ', '/', ')', '\\']).unwrap_or(part.len());
                let login = &part[..end];
                if !login.is_empty() && !logins.iter().any(|known| known == login) {
                    logins.push(login.to_string());
                }
            }
            y += 1;
        }

        entries.push(NotesEntry {
            section_title: escape_html(heading.trim()),
            contents: format!("<ul>{}</ul>", items.concat()),
        });
        i = y;
    }

    let contributors = logins.into_iter().map(|login| Contributor { login, name: None }).collect();
    (contributors, entries)
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_tags_and_breaks() {
        assert_eq!(strip_html("a<br/>b &amp; <b>c</b>"), "a\nb & c");
    }

    #[test]
    fn wraps_on_words() {
        assert_eq!(wrap("one two three", 7), vec!["one two", "three"]);
    }

    #[test]
    fn markdown_sections_and_mentions() {
        let (people, entries) = from_release_markdown("### Fixes\n* thing (@ray)\n* other by @jozz/\n\n### Notes\n* note\n");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].contents, "<ul><li>thing</li><li>other by @jozz/</li></ul>");
        assert_eq!(people.iter().map(|c| c.login.as_str()).collect::<Vec<_>>(), ["ray", "jozz"]);
    }
}

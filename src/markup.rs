//! Lenient reader for ripwire's XML-like answers (`<ctx>`, `<impact>`, `<affected>`...).
//! Not a general XML parser: it only needs elements, quoted attributes, CDATA and comments.

#[derive(Debug, Default, Clone)]
pub struct Node {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub text: String,
    pub children: Vec<Node>,
}

impl Node {
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    pub fn attr_u64(&self, key: &str) -> Option<u64> {
        self.attr(key).and_then(|v| v.parse().ok())
    }

    pub fn flag(&self, key: &str) -> bool {
        matches!(self.attr(key), Some("1") | Some("true"))
    }

    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }

    pub fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }
}

/// Parses the first top-level element of `input`, skipping any leading comments/text.
pub fn parse(input: &str) -> Option<Node> {
    let mut root = Node::default();
    let mut p = Parser { s: input, i: 0 };
    p.content(&mut root);
    root.children.into_iter().next()
}

struct Parser<'a> {
    s: &'a str,
    i: usize,
}

impl<'a> Parser<'a> {
    fn rest(&self) -> &'a str {
        &self.s[self.i..]
    }

    fn skip_past(&mut self, end: &str) -> &'a str {
        let rest = self.rest();
        match rest.find(end) {
            Some(n) => {
                self.i += n + end.len();
                &rest[..n]
            }
            None => {
                self.i = self.s.len();
                rest
            }
        }
    }

    /// An attribute value written without quotes: up to the next whitespace, `/` or `>`.
    fn unquoted(&mut self) -> &'a str {
        let rest = self.rest();
        let n = rest
            .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
            .unwrap_or(rest.len());
        self.i += n;
        &rest[..n]
    }

    /// Reads children and text into `node` until its closing tag (or EOF).
    fn content(&mut self, node: &mut Node) {
        while self.i < self.s.len() {
            let rest = self.rest();
            if rest.starts_with("<!--") {
                self.skip_past("-->");
            } else if rest.starts_with("<![CDATA[") {
                self.i += "<![CDATA[".len();
                let t = self.skip_past("]]>");
                node.text.push_str(t);
            } else if rest.starts_with("</") {
                self.skip_past(">");
                return;
            } else if rest.starts_with('<') {
                let child = self.element();
                node.children.push(child);
            } else {
                let n = rest.find('<').unwrap_or(rest.len());
                node.text.push_str(&unescape(&rest[..n]));
                self.i += n;
            }
        }
    }

    fn element(&mut self) -> Node {
        self.i += 1; // '<'
        let mut node = Node::default();
        let rest = self.rest();
        let n = rest
            .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
            .unwrap_or(rest.len());
        node.name = rest[..n].to_string();
        self.i += n;
        loop {
            let rest = self.rest().trim_start();
            self.i = self.s.len() - rest.len();
            if rest.starts_with("/>") {
                self.i += 2;
                return node;
            }
            if rest.starts_with('>') || rest.is_empty() {
                self.i += rest.len().min(1);
                break;
            }
            let Some(eq) = rest.find('=') else {
                self.i = self.s.len();
                return node;
            };
            let key = rest[..eq].trim().to_string();
            self.i += eq + 1;
            // Nothing after '=': the input was cut off mid-attribute.
            let Some(quote) = self.rest().chars().next() else {
                self.i = self.s.len();
                return node;
            };
            let value = match quote {
                '"' | '\'' => {
                    self.i += quote.len_utf8();
                    self.skip_past(quote.encode_utf8(&mut [0u8; 4]))
                }
                // Unquoted: up to the next whitespace or tag end, so the rest of the
                // document is still read instead of being swallowed as one value.
                _ => self.unquoted(),
            };
            node.attrs.push((key, unescape(value)));
        }
        self.content(&mut node);
        node
    }
}

fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

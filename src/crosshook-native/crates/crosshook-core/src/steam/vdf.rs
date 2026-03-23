use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VdfNode {
    pub value: Option<String>,
    pub children: BTreeMap<String, VdfNode>,
}

impl VdfNode {
    pub fn with_value(value: impl Into<String>) -> Self {
        Self {
            value: Some(value.into()),
            children: BTreeMap::new(),
        }
    }

    pub fn get_child(&self, key: &str) -> Option<&VdfNode> {
        let normalized = normalize_key(key);
        if normalized.is_empty() {
            return None;
        }

        self.children.get(&normalized)
    }

    pub fn find_descendant(&self, key: &str) -> Option<&VdfNode> {
        let normalized = normalize_key(key);
        if normalized.is_empty() {
            return None;
        }

        self.find_descendant_inner(&normalized)
    }

    fn find_descendant_inner(&self, normalized_key: &str) -> Option<&VdfNode> {
        for (key, child) in &self.children {
            if key == normalized_key {
                return Some(child);
            }

            if let Some(match_node) = child.find_descendant_inner(normalized_key) {
                return Some(match_node);
            }
        }

        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VdfParseError {
    pub position: usize,
    pub message: String,
}

impl VdfParseError {
    fn new(position: usize, message: impl Into<String>) -> Self {
        Self {
            position,
            message: message.into(),
        }
    }
}

impl fmt::Display for VdfParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VDF parse error at {}: {}", self.position, self.message)
    }
}

impl Error for VdfParseError {}

pub fn parse_vdf(content: &str) -> Result<VdfNode, VdfParseError> {
    let mut parser = Parser::new(content);
    let node = parser.parse_object(false)?;
    parser.skip_whitespace_and_comments();

    if let Some(character) = parser.peek() {
        return Err(VdfParseError::new(
            parser.index,
            format!("unexpected trailing character '{character}'"),
        ));
    }

    Ok(node)
}

fn normalize_key(key: &str) -> String {
    key.trim().to_ascii_lowercase()
}

struct Parser {
    chars: Vec<char>,
    index: usize,
}

impl Parser {
    fn new(content: &str) -> Self {
        Self {
            chars: content.chars().collect(),
            index: 0,
        }
    }

    fn parse_object(&mut self, stop_on_closing_brace: bool) -> Result<VdfNode, VdfParseError> {
        let mut node = VdfNode::default();

        loop {
            self.skip_whitespace_and_comments();

            if self.index >= self.chars.len() {
                if stop_on_closing_brace {
                    return Err(VdfParseError::new(
                        self.index,
                        "unexpected end of input while parsing object",
                    ));
                }

                return Ok(node);
            }

            if stop_on_closing_brace && self.peek() == Some('}') {
                self.index += 1;
                return Ok(node);
            }

            let Some(key) = self.read_token()? else {
                return Err(VdfParseError::new(self.index, "expected key token"));
            };

            if key.is_empty() {
                return Err(VdfParseError::new(self.index, "expected key token"));
            }

            self.skip_whitespace_and_comments();

            if self.peek() == Some('{') {
                self.index += 1;
                let child = self.parse_object(true)?;
                node.children.insert(normalize_key(&key), child);
                continue;
            }

            let Some(value) = self.read_token()? else {
                return Err(VdfParseError::new(
                    self.index,
                    format!("expected value for key '{key}'"),
                ));
            };

            node.children
                .insert(normalize_key(&key), VdfNode::with_value(value));
        }
    }

    fn read_token(&mut self) -> Result<Option<String>, VdfParseError> {
        self.skip_whitespace_and_comments();

        match self.peek() {
            None => Ok(None),
            Some('"') => Ok(Some(self.read_quoted_token()?)),
            Some('{') | Some('}') => Ok(None),
            Some(_) => Ok(Some(self.read_unquoted_token())),
        }
    }

    fn read_quoted_token(&mut self) -> Result<String, VdfParseError> {
        let mut token = String::new();
        self.index += 1;

        while let Some(character) = self.bump() {
            if character == '"' {
                return Ok(token);
            }

            if character == '\\' {
                let escaped = self.bump().ok_or_else(|| {
                    VdfParseError::new(self.index, "unterminated escape sequence")
                })?;

                token.push(match escaped {
                    '\\' => '\\',
                    '"' => '"',
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    other => other,
                });
                continue;
            }

            token.push(character);
        }

        Err(VdfParseError::new(self.index, "unterminated quoted string"))
    }

    fn read_unquoted_token(&mut self) -> String {
        let mut token = String::new();

        while let Some(character) = self.peek() {
            if character.is_whitespace() || matches!(character, '{' | '}') {
                break;
            }

            token.push(character);
            self.index += 1;
        }

        token
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            while matches!(self.peek(), Some(character) if character.is_whitespace()) {
                self.index += 1;
            }

            if self.peek() == Some('/') && self.peek_next() == Some('/') {
                self.index += 2;

                while let Some(character) = self.peek() {
                    self.index += 1;
                    if character == '\n' {
                        break;
                    }
                }

                continue;
            }

            break;
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.index).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.index + 1).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.index += 1;
        Some(character)
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_vdf, VdfNode};

    #[test]
    fn parses_nested_objects_and_case_insensitive_lookup() {
        let root = parse_vdf(
            r#"
            "libraryfolders"
            {
              "0"
              {
                "path" "/home/user/SteamLibrary"
              }
            }
            "#,
        )
        .expect("parse");

        let libraryfolders = root.get_child("LibraryFolders").expect("libraryfolders");
        let zero = libraryfolders.get_child("0").expect("0");
        let path = zero.get_child("PATH").expect("path");

        assert_eq!(path.value.as_deref(), Some("/home/user/SteamLibrary"));
    }

    #[test]
    fn parses_comments_and_unquoted_tokens() {
        let root = parse_vdf(
            r#"
            // header comment
            libraryfolders
            {
              1
              {
                path /mnt/games
              }
            }
            "#,
        )
        .expect("parse");

        assert_eq!(
            root.find_descendant("path")
                .and_then(|node| node.value.as_deref()),
            Some("/mnt/games"),
        );
    }

    #[test]
    fn parses_escape_sequences_in_quoted_strings() {
        let root =
            parse_vdf("\"message\" \"line 1\\nline 2\\t\\\"quoted\\\"\\\\done\"").expect("parse");

        assert_eq!(
            root.get_child("message")
                .and_then(|node| node.value.as_deref()),
            Some("line 1\nline 2\t\"quoted\"\\done"),
        );
    }

    #[test]
    fn finds_descendants_recursively() {
        let root = parse_vdf(
            r#"
            "outer"
            {
              "middle"
              {
                "CompatToolMapping"
                {
                  "1245620"
                  {
                    "name" "Proton 9.0-4"
                  }
                }
              }
            }
            "#,
        )
        .expect("parse");

        let mapping = root.find_descendant("compattoolmapping").expect("mapping");
        let app = mapping.get_child("1245620").expect("appid");
        let name = app.get_child("name").expect("name");

        assert_eq!(name.value.as_deref(), Some("Proton 9.0-4"));
    }

    #[test]
    fn rejects_unterminated_quoted_strings() {
        let error = parse_vdf("\"broken\" \"value").expect_err("expected error");
        assert!(error.message.contains("unterminated quoted string"));
    }

    #[test]
    fn leaf_nodes_do_not_return_descendants() {
        let leaf = VdfNode::with_value("value");
        assert!(leaf.find_descendant("child").is_none());
    }
}

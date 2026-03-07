//! Templating engine.
//!
//! Simple syntax: {{palette.name (| filter)?}}
//! Supports simple replace and optional filter application.

use std::collections::BTreeSet;
use std::ops::Range;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct TemplateEngine {
    sources: Vec<TemplateSource>,
    templates: Vec<Template>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceIndex(usize);

impl SourceIndex {
    pub fn as_usize(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct TemplateSource {
    path: PathBuf,
    content: String,
    tokens: Vec<Token>,
}

impl TemplateSource {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    pub fn slice(&self, span: &Range<usize>) -> &str {
        &self.content[span.start..span.end]
    }
}

#[derive(Debug, Clone)]
pub enum Token {
    Text(Range<usize>),
    Slot(TemplateIndex),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TemplateIndex(usize);

impl TemplateIndex {
    pub fn as_usize(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct Template {
    pub source: SourceIndex,
    pub span: Range<usize>,
    pub palette: Range<usize>,
    pub member: Range<usize>,
    pub filter: Option<Range<usize>>,
}

impl Template {
    pub fn raw<'a>(&self, source: &'a TemplateSource) -> &'a str {
        source.slice(&self.span)
    }

    pub fn palette_name<'a>(&self, source: &'a TemplateSource) -> &'a str {
        source.slice(&self.palette)
    }

    pub fn member_name<'a>(&self, source: &'a TemplateSource) -> &'a str {
        source.slice(&self.member)
    }

    pub fn filter_name<'a>(&self, source: &'a TemplateSource) -> Option<&'a str> {
        self.filter.as_ref().map(|span| source.slice(span))
    }
}

impl TemplateEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn parse_file(&mut self, path: impl Into<PathBuf>) -> Result<SourceIndex, Error> {
        let path = path.into();
        let content = std::fs::read_to_string(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;
        self.parse(path, content)
    }

    pub fn parse(
        &mut self,
        path: impl Into<PathBuf>,
        content: String,
    ) -> Result<SourceIndex, Error> {
        let parsed = parse_source(path.into(), content)?;

        let source_index = SourceIndex(self.sources.len());
        let template_base = self.templates.len();

        for template in parsed.templates {
            self.templates.push(Template {
                source: source_index,
                span: template.span,
                palette: template.palette,
                member: template.member,
                filter: template.filter,
            });
        }

        let tokens = parsed
            .tokens
            .into_iter()
            .map(|token| match token {
                ParsedToken::Text(span) => Token::Text(span),
                ParsedToken::Slot(local_index) => {
                    Token::Slot(TemplateIndex(template_base + local_index))
                }
            })
            .collect();

        self.sources.push(TemplateSource {
            path: parsed.path,
            content: parsed.content,
            tokens,
        });

        Ok(source_index)
    }

    pub fn source(&self, index: SourceIndex) -> Option<&TemplateSource> {
        self.sources.get(index.0)
    }

    pub fn sources(&self) -> &[TemplateSource] {
        &self.sources
    }

    pub fn template(&self, index: TemplateIndex) -> Option<&Template> {
        self.templates.get(index.0)
    }

    pub fn template_source_index(&self, index: TemplateIndex) -> Option<SourceIndex> {
        self.templates.get(index.0).map(|template| template.source)
    }

    pub fn template_source(&self, index: TemplateIndex) -> Option<&TemplateSource> {
        let source_index = self.template_source_index(index)?;
        self.source(source_index)
    }

    pub fn iter_templates(&self) -> impl Iterator<Item = (TemplateIndex, SourceIndex, &Template)> {
        self.templates
            .iter()
            .enumerate()
            .map(|(idx, template)| (TemplateIndex(idx), template.source, template))
    }

    pub fn required_palettes(&self) -> Vec<String> {
        let mut names = BTreeSet::new();

        for (_, source_index, template) in self.iter_templates() {
            let source = &self.sources[source_index.0];
            names.insert(template.palette_name(source).to_string());
        }

        names.into_iter().collect()
    }
}

#[derive(Debug, Clone)]
struct ParsedSource {
    path: PathBuf,
    content: String,
    tokens: Vec<ParsedToken>,
    templates: Vec<ParsedTemplate>,
}

#[derive(Debug, Clone)]
struct ParsedTemplate {
    span: Range<usize>,
    palette: Range<usize>,
    member: Range<usize>,
    filter: Option<Range<usize>>,
}

#[derive(Debug, Clone)]
enum ParsedToken {
    Text(Range<usize>),
    Slot(usize),
}

fn parse_source(path: PathBuf, content: String) -> Result<ParsedSource, Error> {
    let mut cursor = 0;
    let mut tokens = Vec::new();
    let mut templates = Vec::new();

    while let Some(open_rel) = content[cursor..].find("{{") {
        let open = cursor + open_rel;
        if open > cursor {
            tokens.push(ParsedToken::Text(cursor..open));
        }

        let expr_start = open + 2;
        let Some(close_rel) = content[expr_start..].find("}}") else {
            return Err(Error::UnclosedTemplate { path, offset: open });
        };
        let expr_end = expr_start + close_rel;

        let template = parse_template(&content, expr_start..expr_end, &path)?;
        let local_index = templates.len();
        templates.push(template);
        tokens.push(ParsedToken::Slot(local_index));

        cursor = expr_end + 2;
    }

    if cursor < content.len() {
        tokens.push(ParsedToken::Text(cursor..content.len()));
    }

    Ok(ParsedSource {
        path,
        content,
        tokens,
        templates,
    })
}

fn parse_template(content: &str, span: Range<usize>, path: &Path) -> Result<ParsedTemplate, Error> {
    let span = trim_ascii_range(content, span);
    let inner = &content[span.start..span.end];

    let Some(dot_rel) = inner.find('.') else {
        return Err(Error::ExpectedColorName {
            path: path.to_path_buf(),
            offset: span.start,
        });
    };

    let palette = trim_ascii_range(content, span.start..(span.start + dot_rel));
    if palette.is_empty() {
        return Err(Error::ExpectedColorName {
            path: path.to_path_buf(),
            offset: span.start,
        });
    }

    let rhs_start = span.start + dot_rel + 1;
    let rhs = &content[rhs_start..span.end];

    let (member, filter) = if let Some(pipe_rel) = rhs.find('|') {
        let member = trim_ascii_range(content, rhs_start..(rhs_start + pipe_rel));
        if member.is_empty() {
            return Err(Error::ExpectedColorName {
                path: path.to_path_buf(),
                offset: rhs_start,
            });
        }

        let filter_start = rhs_start + pipe_rel + 1;
        let filter = trim_ascii_range(content, filter_start..span.end);
        if filter.is_empty() {
            return Err(Error::ExpectedFilterName {
                path: path.to_path_buf(),
                offset: filter_start,
            });
        }

        (member, Some(filter))
    } else {
        let member = trim_ascii_range(content, rhs_start..span.end);
        if member.is_empty() {
            return Err(Error::ExpectedColorName {
                path: path.to_path_buf(),
                offset: rhs_start,
            });
        }

        (member, None)
    };

    Ok(ParsedTemplate {
        span,
        palette,
        member,
        filter,
    })
}

fn trim_ascii_range(content: &str, mut span: Range<usize>) -> Range<usize> {
    let bytes = content.as_bytes();
    while span.start < span.end && bytes[span.start].is_ascii_whitespace() {
        span.start += 1;
    }
    while span.start < span.end && bytes[span.end - 1].is_ascii_whitespace() {
        span.end -= 1;
    }
    span
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read template file `{path}`")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("unclosed template expression in `{path}` at byte {offset}")]
    UnclosedTemplate { path: PathBuf, offset: usize },
    #[error("expected `palette.member` in `{path}` at byte {offset}")]
    ExpectedColorName { path: PathBuf, offset: usize },
    #[error("expected filter name after `|` in `{path}` at byte {offset}")]
    ExpectedFilterName { path: PathBuf, offset: usize },
}

#[cfg(test)]
mod tests {
    use super::{TemplateEngine, Token};

    #[test]
    fn parse_tokens_and_ranges() {
        let mut engine = TemplateEngine::new();
        let source_index = engine
            .parse(
                "sample.tmpl",
                "a={{ base16.base00 | hex }};b={{ansi.red}}".to_string(),
            )
            .expect("template should parse");

        let source = engine.source(source_index).expect("source should exist");
        assert_eq!(source.tokens().len(), 4);

        let slot_indices: Vec<_> = source
            .tokens()
            .iter()
            .filter_map(|token| match token {
                Token::Slot(index) => Some(*index),
                Token::Text(_) => None,
            })
            .collect();
        assert_eq!(slot_indices.len(), 2);

        let first = engine
            .template(slot_indices[0])
            .expect("first template should exist");
        assert_eq!(first.raw(source), "base16.base00 | hex");
        assert_eq!(first.palette_name(source), "base16");
        assert_eq!(first.member_name(source), "base00");
        assert_eq!(first.filter_name(source), Some("hex"));
        assert_eq!(first.source.as_usize(), source_index.as_usize());

        let second = engine
            .template(slot_indices[1])
            .expect("second template should exist");
        assert_eq!(second.raw(source), "ansi.red");
        assert_eq!(second.palette_name(source), "ansi");
        assert_eq!(second.member_name(source), "red");
        assert_eq!(second.filter_name(source), None);

        let Token::Text(span) = &source.tokens()[0] else {
            panic!("expected text token");
        };
        assert_eq!(source.slice(span), "a=");
    }

    #[test]
    fn required_palettes_are_deduped_and_sorted() {
        let mut engine = TemplateEngine::new();
        engine
            .parse("a.tmpl", "{{base16.base00}} {{ansi.red}}".to_string())
            .expect("first template should parse");
        engine
            .parse(
                "b.tmpl",
                "{{ansi.green}} {{base16.base01 | hex}}".to_string(),
            )
            .expect("second template should parse");

        assert_eq!(engine.required_palettes(), vec!["ansi", "base16"]);
    }
}

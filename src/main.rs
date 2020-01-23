/// Extracts components from static html files
use html5ever::rcdom::{Node, NodeData, RcDom};
use html5ever::serialize::{serialize, SerializeOpts, TraversalScope};
use html5ever::tendril::TendrilSink;
use html5ever::{parse_document, LocalName, Namespace, ParseOpts, QualName};
use lazy_static::lazy_static;
use regex::Regex;
use serde::Deserialize;
use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::fs;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use structopt::StructOpt;

type WhateverResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Deserialize)]
struct Replacements {
    pre: String,
    post: String,
    replacement: String,
    content: String,
    attributes: String,
    file_extention: String,
}

struct Extractor {
    output: PathBuf,
    replacements: Replacements,
}

#[derive(Debug, StructOpt)]
#[structopt(name = "example", about = "An example of StructOpt usage.")]
struct Opt {
    #[structopt(parse(from_os_str))]
    input: PathBuf,

    #[structopt(parse(from_os_str))]
    output: PathBuf,

    #[structopt(parse(from_os_str))]
    settingsfile: PathBuf,
}

fn main() -> WhateverResult {
    let opt = Opt::from_args();
    let file = fs::read_to_string(opt.settingsfile)?;
    let replacements = toml::from_str(&file)?;

    let extractor = Extractor {
        output: opt.output,
        replacements,
    };
    extractor.index_stuff(&opt.input)?;
    Ok(())
}

impl Extractor {
    fn index_stuff(&self, entry_path: &Path) -> WhateverResult {
        // Put aside everything inside "everything" in the html file for laiter use in yew.
        let mut index = fs::File::open(entry_path)?;
        let mut buf = String::new();
        index.read_to_string(&mut buf)?;
        let opts = ParseOpts::default();
        let mut dom = parse_document(RcDom::default(), opts)
            .from_utf8()
            .read_from(&mut buf.as_bytes())?;
        {
            let mut document = dom.document.borrow_mut().children.borrow_mut();
            // Remove webflow timestamp and watermark
            document.remove(1);
            document.remove(1);
            let html = &mut document[1].children.borrow_mut();
            self.walk(&html[2])?;
        }
        Ok(())
    }

    /// Recursive function to walk all nodes
    fn walk(&self, node: &Rc<Node>) -> WhateverResult {
        let mut templates = vec![];
        let mut content = vec![];
        for (i, child) in node.children.borrow_mut().iter().enumerate() {
            self.walk(&child)?;
            if let Some(name) = find_attribute(&child, "data-extract") {
                templates.push((name, i));
            }
            if let Some(_) = find_attribute(&child, "data-content") {
                content.push(i);
            }
        }
        let mut children = node.children.borrow_mut();
        for i in content.iter().rev() {
            let mut children = children[*i].children.borrow_mut();
            children.clear();
            children.push(create_node("replace_content".to_owned()));
        }
        for (name, i) in templates.iter().rev() {
            if name == "not" {
                children.remove(*i);
            } else {
                let new_node = create_node(format!("replace_replacement-{}", name));
                let node = std::mem::replace(&mut children[*i], new_node);
                self.write_template(name, node)?;
            }
        }
        Ok(())
    }

    fn text_replacements(&self, text: &str) -> String {
        let string = convert_to_jsx(text);
        // The elements we removed for templating need to be replaced with the code that slides them in again.
        let string = REPLACEMENTS.replace_all(&string, |captures: &regex::Captures| {
            let name = captures.name("a").unwrap().as_str();
            self.replacement(name)
        });
        let string = string.replace("<replace_content />", &self.replacements.content);
        ATTRIBUTES
            .replace_all(&string, |captures: &regex::Captures| {
                let name = captures.name("a").unwrap().as_str();
                self.attribute(name)
            })
            .into()
    }

    fn replacement(&self, name: &str) -> String {
        self.replacements.replacement.replace("%", name)
    }

    fn attribute(&self, name: &str) -> String {
        self.replacements.attributes.replace("%", name)
    }

    fn write_template(&self, name: &str, node: Rc<Node>) -> WhateverResult {
        let string = serialize_html(node, true)?;
        let string = self.text_replacements(&string);
        let pre = self.replacements.pre.replace("%", name);
        let post = self.replacements.post.replace("%", name);
        self.extract(name, format!("{} {} {}", pre, string, post))?;
        Ok(())
    }

    fn extract(&self, name: &str, raw: String) -> WhateverResult {
        let name = format!("{}.{}", name, self.replacements.file_extention);
        let mut new_index = fs::File::create(&self.output.join(&name))?;
        new_index.write(&raw.as_bytes())?;
        println!("Extracted {}", name);
        Ok(())
    }
}

fn find_attribute(node: &Rc<Node>, attribute: &str) -> Option<String> {
    match &node.data {
        NodeData::Element { attrs, .. } => attrs
            .borrow()
            .iter()
            .find(|a| a.name.local.as_ref() == attribute)
            .map(|a| a.value.to_string()),
        _ => None,
    }
}

fn create_node(name: String) -> Rc<Node> {
    Node::new(NodeData::Element {
        name: QualName::new(None, Namespace::from(""), LocalName::from(name)),
        template_contents: None,
        attrs: RefCell::new(vec![]),
        mathml_annotation_xml_integration_point: false,
    })
}

fn serialize_html(node: Rc<Node>, root: bool) -> Result<String, Box<dyn std::error::Error>> {
    let mut bytes = vec![];
    let opts = if root {
        SerializeOpts {
            traversal_scope: TraversalScope::IncludeNode,
            ..Default::default()
        }
    } else {
        SerializeOpts::default()
    };
    serialize(&mut bytes, &node, opts)?;
    Ok(String::from_utf8(bytes)?)
}

fn convert_to_jsx(source: &str) -> String {
    let mut file = fs::File::create("/tmp/jsxconvert.html").unwrap();
    file.write_all(source.as_bytes()).unwrap();
    let output = Command::new("htmltojsx")
        .arg("/tmp/jsxconvert.html")
        .output()
        .unwrap();
    let output = std::str::from_utf8(&output.stdout).unwrap();
    output.replace("defaultValue={\"\"}", "")
}

lazy_static! {
    static ref REPLACEMENTS: Regex = Regex::new(r"<replace_replacement-(?P<a>[\w\W]+?)/>").unwrap();
    static ref ATTRIBUTES: Regex = Regex::new(r#"data-attribute="(?P<a>[\w\W]+?)""#).unwrap();
}

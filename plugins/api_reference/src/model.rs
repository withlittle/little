//! What a page documents, read from the code, in no language in particular:
//! a plugin reads it from its language's code, and the Markdown module
//! writes it.

/// What an object is, which a page labels it with, and groups it by: a
/// kind of Python's, of Rust's, or of both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Package,
    Module,
    Class,
    Function,
    Method,
    Attribute,
    Property,
    Crate,
    Struct,
    Enum,
    Union,
    Trait,
    TypeAlias,
    AssociatedType,
    Constant,
    AssociatedConstant,
    Static,
    Macro,
}

impl Kind {
    /// Its label, like `class`.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Package => "package",
            Kind::Module => "module",
            Kind::Class => "class",
            Kind::Function => "function",
            Kind::Method => "method",
            Kind::Attribute => "attribute",
            Kind::Property => "property",
            Kind::Crate => "crate",
            Kind::Struct => "struct",
            Kind::Enum => "enum",
            Kind::Union => "union",
            Kind::Trait => "trait",
            Kind::TypeAlias => "type alias",
            Kind::AssociatedType => "associated type",
            Kind::Constant => "constant",
            Kind::AssociatedConstant => "associated constant",
            Kind::Static => "static",
            Kind::Macro => "macro",
        }
    }
}

/// A package, module, class, function, or attribute, or a crate, struct,
/// trait, and so on.
#[derive(Clone, Debug, PartialEq)]
pub struct Object {
    /// Its name, like `Solver`.
    pub name: String,
    pub kind: Kind,
    /// Its labels after its kind's, like `dataclass` or `abstractmethod`.
    pub labels: Vec<String>,
    /// How it's called, or, for an attribute, its name, type, and value, as
    /// code, like `solve(problem: Problem) -> Solution`.
    pub signature: Option<String>,
    /// The parameters it's called with, for its docstring's.
    pub parameters: Vec<Parameter>,
    /// The type it returns, or an attribute's type.
    pub returns: Option<String>,
    /// Its docstring's sections, and, for a class, its constructor's after
    /// them.
    pub docs: Vec<Section>,
    /// The code it's defined by, for a class its constructor's, from the
    /// start of its first line.
    pub source: Option<String>,
    /// The file that code is in, if it isn't its page's.
    pub file: Option<String>,
    /// The types of the attributes it defines, documented or not, by name,
    /// for its docstring's.
    pub types: Vec<(String, String)>,
    /// What it defines that's documented.
    pub members: Vec<Object>,
}

impl Object {
    /// An object of `kind`, called `name`, with nothing else known of it.
    pub fn new(name: &str, kind: Kind) -> Object {
        Object {
            name: name.to_string(),
            kind,
            labels: Vec::new(),
            signature: None,
            parameters: Vec::new(),
            returns: None,
            docs: Vec::new(),
            source: None,
            file: None,
            types: Vec::new(),
            members: Vec::new(),
        }
    }

    /// Whether it has a docstring.
    pub fn is_documented(&self) -> bool {
        !self.docs.is_empty()
    }
}

/// A parameter in a signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parameter {
    /// Its name, without `*` or `**`.
    pub name: String,
    /// Its type, as code.
    pub annotation: Option<String>,
    /// Its default value, as code: an empty tuple or dict for one that
    /// takes the rest of the arguments.
    pub default: Option<String>,
}

/// A section of a docstring.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Section {
    /// Text, as Markdown, under its title, if it has one, like `Notes`.
    Text { title: Option<String>, text: String },
    /// A list of parameters, values returned, exceptions raised, and so on.
    Items { kind: Items, items: Vec<Item> },
}

/// What a docstring's section lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Items {
    Parameters,
    OtherParameters,
    Attributes,
    Returns,
    Yields,
    Receives,
    Raises,
    Warns,
    Fields,
    Variants,
}

impl Items {
    /// Its title, like `Other Parameters`.
    pub fn title(self) -> &'static str {
        match self {
            Items::Parameters => "Parameters",
            Items::OtherParameters => "Other Parameters",
            Items::Attributes => "Attributes",
            Items::Returns => "Returns",
            Items::Yields => "Yields",
            Items::Receives => "Receives",
            Items::Raises => "Raises",
            Items::Warns => "Warns",
            Items::Fields => "Fields",
            Items::Variants => "Variants",
        }
    }
}

/// A parameter, value returned, exception raised, and so on, in a
/// docstring.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Item {
    pub name: Option<String>,
    /// Its type, as the docstring writes it.
    pub ty: Option<String>,
    /// Its default value, as the docstring writes it.
    pub default: Option<String>,
    /// What it is, as Markdown.
    pub description: String,
}

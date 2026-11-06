//! Writing a document's page: its title, what `info` says, its servers,
//! the security it needs, and its description; then its security schemes,
//! each tag's operations, its webhooks, and its schemas.

use std::collections::HashMap;

use little_api_reference::pages::last;
use serde_json::Value;

use crate::markdown::{
    self, both, code, collapse, either, escape, fence_for, inline, label, link, shift, table, web,
};
use crate::refs::{Files, Followed, Location, Node};
use crate::schema::{self, FIELDS};

/// What a link on the page can lead to, by its name: a schema's heading,
/// under Schemas, or a security scheme's, under Authentication.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    Schema(String),
    Scheme(String),
}

/// A page, as it's written.
pub struct Writer<'a> {
    /// The document's files.
    pub files: &'a Files,
    /// The document.
    pub root: Node<'a>,
    /// The name of each schema in `components.schemas`, by where it is, and
    /// of where each `$ref` there leads.
    pub names: HashMap<Location, &'a str>,
    /// The id of each heading a link leads to, once the page has been
    /// written without links, to find them.
    ids: Option<HashMap<Target, String>>,
    /// Where each heading a link leads to starts, in `out`.
    headings: Vec<(usize, Target)>,
    /// The page's Markdown, so far.
    pub out: String,
    /// What to warn of.
    pub warnings: Vec<String>,
}

/// An operation, on a path, or a webhook.
struct Operation<'a> {
    /// Its method, like `GET`.
    method: String,
    /// Its path, like `/pets/{petId}`, or its webhook's name.
    path: &'a str,
    node: Node<'a>,
    /// The path item it's in.
    item: Node<'a>,
    /// Its tags, each once.
    tags: Vec<&'a str>,
}

/// The methods a path item has an operation for, in OpenAPI 3.2.
const METHODS: [&str; 9] = [
    "get", "put", "post", "delete", "options", "head", "patch", "trace", "query",
];

/// The Markdown of the page of the document in `file`, and what to warn
/// of.
pub fn write(files: &Files, file: &str) -> (String, Vec<String>) {
    let root = files.root(file);
    let names = schema::names(files, root);
    // A link to a heading needs the id comrak gives it, which depends on
    // every heading before it, so the page is written once to find them,
    // and again with the links.
    let mut first = Writer::new(files, root, names.clone(), None);
    first.page(file);
    let starts: HashMap<usize, String> = markdown::ids(&first.out).into_iter().collect();
    let mut ids = HashMap::new();
    for (at, target) in first.headings {
        if let Some(id) = starts.get(&at) {
            ids.entry(target).or_insert_with(|| id.clone());
        }
    }
    let mut writer = Writer::new(files, root, names, Some(ids));
    writer.page(file);
    let mut page = format!("{}\n", writer.out.trim_end());
    if let Some(summary) = root.get("info").and_then(|info| info.text("summary")) {
        // A string in JSON is one in YAML too, so it reads back as it is.
        let description = Value::from(collapse(summary));
        page = format!("---\ndescription: {description}\n---\n\n{page}");
    }
    (page, writer.warnings)
}

impl<'a> Writer<'a> {
    fn new(
        files: &'a Files,
        root: Node<'a>,
        names: HashMap<Location, &'a str>,
        ids: Option<HashMap<Target, String>>,
    ) -> Writer<'a> {
        Writer {
            files,
            root,
            names,
            ids,
            headings: Vec::new(),
            out: String::new(),
            warnings: Vec::new(),
        }
    }

    /// What `node`'s `$ref` leads to, if it has one, else `node`; or none,
    /// warning why, if it can't be followed.
    pub fn follow(&mut self, node: Node<'a>) -> Option<Followed<'a>> {
        self.files.follow(node, &mut self.warnings)
    }

    /// What `node` is, once its `$ref`s are followed.
    pub fn resolve(&mut self, node: Node<'a>) -> Option<Node<'a>> {
        self.follow(node).map(|followed| followed.node)
    }

    /// `text`, linked to `target`'s heading, if it has one.
    pub fn target_link(&self, text: String, target: Target) -> String {
        match self.ids.as_ref().and_then(|ids| ids.get(&target)) {
            Some(id) => link(&text, &format!("#{id}")),
            None => text,
        }
    }

    /// A heading at `level`, of `text`, which links to `target` lead to.
    pub fn heading(&mut self, level: usize, text: &str, target: Option<Target>) {
        if let Some(target) = target {
            self.headings.push((self.out.len(), target));
        }
        self.out += &format!("{} {text}\n\n", "#".repeat(level));
    }

    /// A block of `text`, like a paragraph.
    pub fn line(&mut self, text: &str) {
        self.out += text;
        self.out += "\n\n";
    }

    /// `text`, a description, in Markdown, under a heading at `level`, if
    /// it isn't blank.
    pub fn description(&mut self, text: Option<&str>, level: usize) {
        if let Some(text) = text.filter(|text| !text.trim().is_empty()) {
            let text = shift(text, level);
            self.line(text.trim_end());
        }
    }

    /// The page of the document in `file`.
    fn page(&mut self, file: &str) {
        let root = self.root;
        let info = root.get("info");
        let title = match info.and_then(|info| info.text("title")) {
            Some(title) => inline(title),
            None => {
                let name = last(file);
                escape(name.rsplit_once('.').map_or(name, |(stem, _)| stem))
            }
        };
        self.heading(1, &title, None);
        self.info(info);
        self.servers(root);
        let security = root.get("security").filter(|security| {
            (security.value.as_array()).is_some_and(|requirements| !requirements.is_empty())
        });
        if let Some(security) = security {
            let text = self.security(security);
            self.line(&label("Security", "", &text));
        }
        self.description(info.and_then(|info| info.str("description")), 1);
        self.authentication();
        self.tagged();
        self.webhooks();
        self.schemas();
    }

    /// A list of what `info` says of the API, and its external docs.
    fn info(&mut self, info: Option<Node<'a>>) {
        let mut items = Vec::new();
        if let Some(info) = info {
            if let Some(version) = info.value.get("version").filter(|version| !blank(version)) {
                items.push(label("Version", "", &markdown::value(version)));
            }
            if let Some(terms) = info.text("termsOfService") {
                items.push(label("Terms of service", "", &markdown::url(terms.trim())));
            }
            if let Some(contact) = info.get("contact") {
                let mut parts: Vec<String> = titled_link(contact.text("name"), contact.text("url"))
                    .into_iter()
                    .collect();
                if let Some(email) = contact.text("email").map(str::trim) {
                    parts.push(link(&escape(email), &format!("mailto:{email}")));
                }
                if !parts.is_empty() {
                    items.push(label("Contact", "", &parts.join(", ")));
                }
            }
            if let Some(licence) = info.get("license") {
                let name = titled_link(licence.text("name"), licence.text("url"));
                let identifier = licence.text("identifier").map(|id| code(id.trim()));
                let text = match (name, identifier) {
                    (Some(name), Some(identifier)) => Some(format!("{name} ({identifier})")),
                    (name, identifier) => name.or(identifier),
                };
                if let Some(text) = text {
                    items.push(label("Licence", "", &text));
                }
            }
        }
        if let Some(docs) = self.root.get("externalDocs").and_then(external) {
            items.push(label("External docs", "", &docs));
        }
        if !items.is_empty() {
            let items: Vec<String> = items.iter().map(|item| format!("- {item}")).collect();
            self.line(&items.join("\n"));
        }
    }

    /// The servers `node`, the document, a path item, or an operation, has,
    /// if it has any, with their variables.
    fn servers(&mut self, node: Node<'a>) {
        let mut list = Vec::new();
        for server in node.items_at("servers") {
            let Some(url) = server.str("url") else {
                continue;
            };
            let mut item = format!("- {}", code(url.trim()));
            if let Some(description) = server.text("description") {
                item += &format!(": {}", collapse(description));
            }
            list.push(item);
            for (name, variable) in server.entries_at("variables") {
                let mut notes = Vec::new();
                if let Some(description) = variable.text("description") {
                    notes.push(collapse(description));
                }
                let values: Vec<String> = variable
                    .items_at("enum")
                    .map(|value| markdown::value(value.value))
                    .collect();
                match values.as_slice() {
                    [] => {}
                    [value] => notes.push(format!("Always {value}.")),
                    values => notes.push(format!("One of {}.", either(values))),
                }
                let default = variable.value.get("default").filter(|_| values.len() != 1);
                if let Some(default) = default {
                    notes.push(format!("Defaults to {}.", markdown::value(default)));
                }
                let notes = match notes.is_empty() {
                    true => String::new(),
                    false => format!(": {}", markdown::sentences(&notes)),
                };
                list.push(format!("  - {}{notes}", code(name)));
            }
        }
        if !list.is_empty() {
            self.line(&label("Servers", "", ""));
            self.line(&list.join("\n"));
        }
    }

    /// The security `security`, a list of requirements, any of which will
    /// do, needs, as a sentence: each requirement's schemes, linked to their
    /// headings, with the scopes it needs.
    fn security(&mut self, security: Node<'a>) -> String {
        let mut requirements = Vec::new();
        let mut several = false;
        for requirement in security.items() {
            let mut schemes = Vec::new();
            for (name, scopes) in requirement.entries() {
                let scheme = self.target_link(code(name), Target::Scheme(name.to_string()));
                let scopes: Vec<String> = scopes
                    .items()
                    .filter_map(|scope| scope.value.as_str())
                    .map(code)
                    .collect();
                schemes.push(match scopes.is_empty() {
                    true => scheme,
                    false => format!("{scheme} ({})", both(&scopes)),
                });
            }
            several |= schemes.len() > 1;
            requirements.push(match schemes.is_empty() {
                true => "none".to_string(),
                false => both(&schemes),
            });
        }
        match requirements.len() {
            0 => "none".into(),
            // `a` and `b`, or `c`, not `a` and `b` or `c`.
            2.. if several => format!("either {}", requirements.join(", or ")),
            _ => either(&requirements),
        }
    }

    /// `## Authentication`, with each of the document's security schemes.
    fn authentication(&mut self) {
        let components = self.root.get("components");
        let schemes: Vec<(&str, Node)> = components
            .into_iter()
            .flat_map(|components| components.entries_at("securitySchemes"))
            .collect();
        if schemes.is_empty() {
            return;
        }
        self.heading(2, "Authentication", None);
        for (name, scheme) in schemes {
            self.heading(3, &code(name), Some(Target::Scheme(name.to_string())));
            let Some(followed) = self.follow(scheme) else {
                continue;
            };
            let node = followed.node;
            if node.bool("deprecated") || scheme.bool("deprecated") {
                self.line("**Deprecated.**");
            }
            self.scheme(node);
            self.description(followed.description.or(node.text("description")), 3);
        }
    }

    /// What a client sends for the security scheme `scheme`.
    fn scheme(&mut self, scheme: Node<'a>) {
        let Some(kind) = scheme.text("type") else {
            return;
        };
        match kind.trim() {
            "apiKey" => {
                let name = code(scheme.str("name").unwrap_or_default().trim());
                let place = match scheme.str("in") {
                    Some("query") => format!("in the {name} query parameter"),
                    Some("cookie") => format!("in the {name} cookie"),
                    _ => format!("in the {name} header"),
                };
                self.line(&format!("An API key, {place}."));
            }
            "http" => {
                let name = scheme.str("scheme").unwrap_or_default().trim();
                let text = match name.to_ascii_lowercase().as_str() {
                    "basic" => "A username and password, in the `Authorization` header, with \
                                HTTP's `Basic` scheme."
                        .to_string(),
                    "bearer" => {
                        let format = (scheme.text("bearerFormat"))
                            .map(|format| format!(", formatted as {}", code(format.trim())));
                        let format = format.unwrap_or_default();
                        format!("A bearer token, in the `Authorization` header{format}.")
                    }
                    _ => format!(
                        "Credentials, in the `Authorization` header, with HTTP's {} scheme.",
                        code(name)
                    ),
                };
                self.line(&text);
            }
            "oauth2" => self.oauth(scheme),
            "openIdConnect" => {
                let at = (scheme.text("openIdConnectUrl"))
                    .map(|url| format!(", with its configuration at {}", code(url.trim())));
                self.line(&format!("OpenID Connect{}.", at.unwrap_or_default()));
            }
            "mutualTLS" => self.line("Mutual TLS, with a client certificate."),
            kind => self.line(&format!("Of type {}.", code(kind))),
        }
    }

    /// OAuth 2.0's flows in `scheme`, with their URLs, and their scopes.
    fn oauth(&mut self, scheme: Node<'a>) {
        const FLOWS: [(&str, &str); 5] = [
            ("implicit", "Implicit"),
            ("password", "Password"),
            ("clientCredentials", "Client credentials"),
            ("authorizationCode", "Authorization code"),
            ("deviceAuthorization", "Device authorization"),
        ];
        const URLS: [(&str, &str); 4] = [
            ("authorizationUrl", "the authorization URL"),
            ("deviceAuthorizationUrl", "the device authorization URL"),
            ("tokenUrl", "the token URL"),
            ("refreshUrl", "the refresh URL"),
        ];
        let mut list = Vec::new();
        let mut scopes: Vec<(&str, Node)> = Vec::new();
        for (key, flow) in scheme.entries_at("flows") {
            let Some((_, name)) = FLOWS.iter().find(|(flow, _)| *flow == key) else {
                continue;
            };
            let urls: Vec<String> = (URLS.iter())
                .filter_map(|(key, what)| Some(format!("{what} {}", code(flow.text(key)?.trim()))))
                .collect();
            list.push(match urls.is_empty() {
                true => format!("- {name}."),
                false => format!("- {name}, with {}.", both(&urls)),
            });
            for (scope, description) in flow.entries_at("scopes") {
                if !scopes.iter().any(|(other, _)| *other == scope) {
                    scopes.push((scope, description));
                }
            }
        }
        if list.is_empty() {
            self.line("OAuth 2.0.");
        } else {
            self.line("OAuth 2.0, with these flows:");
            self.line(&list.join("\n"));
        }
        if !scopes.is_empty() {
            let rows: Vec<Vec<String>> = (scopes.iter())
                .map(|(scope, description)| {
                    let description = description.value.as_str().map(inline);
                    vec![code(scope), description.unwrap_or_default()]
                })
                .collect();
            table(&mut self.out, &["Scope", "Description"], &rows);
        }
        if let Some(metadata) = scheme.text("oauth2MetadataUrl") {
            self.line(&format!("Its metadata is at {}.", code(metadata.trim())));
        }
    }

    /// The operations in the document's `paths`, or `webhooks`, in order.
    fn operations_in(&mut self, key: &str) -> Vec<Operation<'a>> {
        let mut operations = Vec::new();
        for (path, item) in self.root.entries_at(key) {
            if path.starts_with("x-") {
                continue;
            }
            let Some(item) = self.resolve(item) else {
                continue;
            };
            let methods = (item.entries())
                .filter(|(method, _)| METHODS.contains(method))
                .map(|(method, node)| (method.to_ascii_uppercase(), node));
            let additional = (item.entries_at("additionalOperations"))
                .map(|(method, node)| (method.to_string(), node));
            for (method, node) in methods.chain(additional) {
                if !node.value.is_object() {
                    continue;
                }
                let mut tags = Vec::new();
                for tag in node.items_at("tags").filter_map(|tag| tag.value.as_str()) {
                    if !tag.trim().is_empty() && !tags.contains(&tag) {
                        tags.push(tag);
                    }
                }
                operations.push(Operation {
                    method,
                    path,
                    node,
                    item,
                    tags,
                });
            }
        }
        operations
    }

    /// A section for each tag, with its operations, in the order `tags`
    /// lists them, then those it doesn't, and one for operations with none.
    fn tagged(&mut self) {
        let operations = self.operations_in("paths");
        let declared: Vec<(&str, Node)> = (self.root.items_at("tags"))
            .filter_map(|tag| Some((tag.text("name")?, tag)))
            .collect();
        let mut order: Vec<&str> = Vec::new();
        let used = operations
            .iter()
            .flat_map(|operation| operation.tags.iter().copied());
        for tag in declared.iter().map(|(name, _)| *name).chain(used) {
            if !order.contains(&tag) {
                order.push(tag);
            }
        }
        for tag in order {
            let tagged: Vec<&Operation> = (operations.iter())
                .filter(|operation| operation.tags.contains(&tag))
                .collect();
            if tagged.is_empty() {
                continue;
            }
            let node = declared
                .iter()
                .find(|(name, _)| *name == tag)
                .map(|(_, node)| *node);
            let title = node.and_then(|node| node.text("summary")).unwrap_or(tag);
            self.heading(2, &inline(title), None);
            if let Some(node) = node {
                self.description(node.str("description"), 2);
                if let Some(docs) = node.get("externalDocs").and_then(external) {
                    self.line(&label("External docs", "", &docs));
                }
            }
            for operation in tagged {
                self.operation(operation, 3, false);
            }
        }
        let untagged: Vec<&Operation> = (operations.iter())
            .filter(|operation| operation.tags.is_empty())
            .collect();
        if !untagged.is_empty() {
            let title = match untagged.len() == operations.len() {
                true => "Endpoints",
                false => "Other endpoints",
            };
            self.heading(2, title, None);
            for operation in untagged {
                self.operation(operation, 3, false);
            }
        }
    }

    /// `## Webhooks`, with each of the document's webhooks.
    fn webhooks(&mut self) {
        let operations = self.operations_in("webhooks");
        if operations.is_empty() {
            return;
        }
        self.heading(2, "Webhooks", None);
        for operation in &operations {
            self.operation(operation, 3, true);
        }
    }

    /// `operation`, under a heading at `level`, and a `webhook`, if it's
    /// one.
    fn operation(&mut self, operation: &Operation<'a>, level: usize, webhook: bool) {
        let node = operation.node;
        let summary = node.text("summary");
        let request = format!("{} {}", operation.method, operation.path);
        let heading = match (summary, webhook) {
            (Some(summary), _) => inline(summary),
            (None, true) => code(operation.path),
            (None, false) => code(&request),
        };
        self.heading(level, &heading, None);
        if node.bool("deprecated") {
            self.line("**Deprecated.**");
        }
        let method = code(&operation.method);
        match (summary, webhook) {
            (Some(_), true) => {
                let name = code(operation.path);
                self.line(&format!("The API sends {name} as a {method} request."));
            }
            (None, true) => self.line(&format!("The API sends a {method} request.")),
            (Some(_), false) => {
                let fence = fence_for(&request);
                self.line(&format!("{fence}http\n{request}\n{fence}"));
            }
            (None, false) => {}
        }
        let description = node
            .text("description")
            .or(operation.item.text("description"));
        self.description(description, level);
        if let Some(docs) = node.get("externalDocs").and_then(external) {
            self.line(&label("External docs", "", &docs));
        }
        if let Some(security) = node.get("security") {
            let text = self.security(security);
            self.line(&label("Security", "", &text));
        }
        match node.items_at("servers").next() {
            Some(_) => self.servers(node),
            None => self.servers(operation.item),
        }
        self.parameters(operation);
        self.request_body(node, level);
        self.responses(node, level);
    }

    /// A table of `operation`'s parameters, and its path item's, for each
    /// place they go.
    fn parameters(&mut self, operation: &Operation<'a>) {
        const PLACES: [(&str, &str); 5] = [
            ("path", "Path parameters"),
            ("query", "Query parameters"),
            ("querystring", "Query string"),
            ("header", "Headers"),
            ("cookie", "Cookies"),
        ];
        // Each with the description beside its `$ref`, which goes over its
        // own.
        let mut parameters: Vec<(Node, Option<&str>)> = Vec::new();
        let all = operation
            .item
            .items_at("parameters")
            .chain(operation.node.items_at("parameters"));
        for node in all {
            let Some(followed) = self.follow(node) else {
                continue;
            };
            let parameter = followed.node;
            let (Some(name), Some(place)) = (parameter.str("name"), parameter.str("in")) else {
                continue;
            };
            // OpenAPI describes these headers elsewhere.
            let described = ["accept", "content-type", "authorization"];
            if place == "header" && described.contains(&name.to_ascii_lowercase().as_str()) {
                continue;
            }
            // An operation's parameter replaces its path item's.
            let same = |(other, _): &(Node, Option<&str>)| {
                other.str("name") == Some(name) && other.str("in") == Some(place)
            };
            match parameters.iter().position(same) {
                Some(at) => parameters[at] = (parameter, followed.description),
                None => parameters.push((parameter, followed.description)),
            }
        }
        for (place, title) in PLACES {
            let mut rows = Vec::new();
            for (parameter, over) in &parameters {
                if parameter.str("in") != Some(place) {
                    continue;
                }
                let schema = self.parameter_schema(*parameter);
                let kind = match schema {
                    Some(schema) => self.kind(schema),
                    None => "any".into(),
                };
                let required = place == "path" || parameter.bool("required");
                let notes = self.notes(schema, Some(*parameter), *over);
                let name = code(parameter.str("name").unwrap_or_default());
                rows.push(vec![name, kind, schema::yes(required), notes]);
            }
            if !rows.is_empty() {
                self.line(&label(title, "", ""));
                table(
                    &mut self.out,
                    &["Name", "Type", "Required", "Description"],
                    &rows,
                );
            }
        }
    }

    /// The schema of `parameter`, or a header: its own, or its first media
    /// type's.
    fn parameter_schema(&mut self, parameter: Node<'a>) -> Option<Node<'a>> {
        if let Some(schema) = parameter.get("schema") {
            return Some(schema);
        }
        let (_, media) = self.media(parameter).into_iter().next()?;
        media.get("schema")
    }

    /// The media types in `node`'s `content`, each with what it says of
    /// them, in order.
    fn media(&mut self, node: Node<'a>) -> Vec<(&'a str, Node<'a>)> {
        let mut media = Vec::new();
        for (name, entry) in node.entries_at("content") {
            if let Some(entry) = self.resolve(entry) {
                media.push((name, entry));
            }
        }
        media
    }

    /// What each schema of `media` is, with the media types it's in, like
    /// [`Pet`], as `application/json` or `application/xml`.
    fn media_kinds(&mut self, media: &[(&'a str, Node<'a>)]) -> String {
        let mut groups: Vec<(Option<String>, Vec<String>)> = Vec::new();
        for (name, node) in media {
            let kind = node.get("schema").map(|schema| self.kind(schema));
            let name = code(name);
            match groups.iter_mut().find(|(other, _)| *other == kind) {
                Some((_, names)) => names.push(name),
                None => groups.push((kind, vec![name])),
            }
        }
        let groups: Vec<String> = (groups.into_iter())
            .map(|(kind, names)| match kind {
                Some(kind) => format!("{kind}, as {}", either(&names)),
                None => either(&names),
            })
            .collect();
        groups.join("; ")
    }

    /// The request body `node`, an operation at `level`, takes, if it takes
    /// one.
    fn request_body(&mut self, node: Node<'a>, level: usize) {
        let Some(body) = node.get("requestBody") else {
            return;
        };
        let Some(followed) = self.follow(body) else {
            let reference = body.reference().unwrap_or_default();
            self.line(&label("Request body", "", &code(reference)));
            return;
        };
        let body = followed.node;
        let media = self.media(body);
        let required = if body.bool("required") {
            " (required)"
        } else {
            ""
        };
        let kinds = self.media_kinds(&media);
        self.line(&label("Request body", required, &kinds));
        self.description(followed.description.or(body.text("description")), level);
        self.fields("Request body", &media);
        self.examples("Request body", &media, level);
    }

    /// A table of the responses `node`, an operation at `level`, gives,
    /// then each one's headers, fields, and examples.
    fn responses(&mut self, node: Node<'a>, level: usize) {
        let mut rows = Vec::new();
        let mut responses = Vec::new();
        let mut bodies = false;
        for (status, response) in node.entries_at("responses") {
            if status.starts_with("x-") {
                continue;
            }
            let Some(followed) = self.follow(response) else {
                let reference = code(response.reference().unwrap_or_default());
                rows.push(vec![code(status), reference, String::new()]);
                continue;
            };
            let response = followed.node;
            let media = self.media(response);
            bodies |= !media.is_empty();
            let description = followed.description.or(response.text("description"));
            let kinds = self.media_kinds(&media);
            rows.push(vec![
                code(status),
                collapse(description.unwrap_or_default()),
                kinds,
            ]);
            responses.push((status, response, media));
        }
        if rows.is_empty() {
            return;
        }
        self.line(&label("Responses", "", ""));
        if bodies {
            table(&mut self.out, &["Status", "Description", "Body"], &rows);
        } else {
            rows.iter_mut().for_each(|row| row.truncate(2));
            table(&mut self.out, &["Status", "Description"], &rows);
        }
        for (status, response, media) in responses {
            let prefix = format!("Response {}", code(status));
            self.headers(&prefix, response);
            self.fields(&prefix, &media);
            self.examples(&prefix, &media, level);
        }
    }

    /// A table of the headers of `response`, after `prefix`, like
    /// Response `200`, if it has any.
    fn headers(&mut self, prefix: &str, response: Node<'a>) {
        let mut rows = Vec::new();
        for (name, header) in response.entries_at("headers") {
            // OpenAPI describes it with the media types.
            if name.eq_ignore_ascii_case("content-type") {
                continue;
            }
            let Some(followed) = self.follow(header) else {
                continue;
            };
            let header = followed.node;
            let schema = self.parameter_schema(header);
            let kind = match schema {
                Some(schema) => self.kind(schema),
                None => "any".into(),
            };
            let notes = self.notes(schema, Some(header), followed.description);
            rows.push(vec![code(name), kind, notes]);
        }
        if !rows.is_empty() {
            self.line(&label(&format!("{prefix} headers"), "", ""));
            table(&mut self.out, &["Name", "Type", "Description"], &rows);
        }
    }

    /// A table of the fields of each of `media`'s schemas, after `prefix`,
    /// like Request body, or one, if they're all the same.
    fn fields(&mut self, prefix: &str, media: &[(&'a str, Node<'a>)]) {
        let mut tables = Vec::new();
        for (name, node) in media {
            let rows = match node.get("schema") {
                Some(schema) => self.field_rows(schema),
                None => Vec::new(),
            };
            tables.push((*name, rows));
        }
        let label_of = |after: &str| label(&format!("{prefix} fields"), after, "");
        if let Some((_, first)) = tables.first()
            && !first.is_empty()
            && tables.iter().all(|(_, rows)| rows == first)
        {
            let first = first.clone();
            self.line(&label_of(""));
            table(&mut self.out, &FIELDS, &first);
            return;
        }
        for (name, rows) in tables {
            if !rows.is_empty() {
                self.line(&label_of(&format!(", as {}", code(name))));
                table(&mut self.out, &FIELDS, &rows);
            }
        }
    }

    /// The examples of each of `media`, under a heading at `level`, after
    /// `prefix`, like Request body: its own, or else its schema's, if it's
    /// written where it's used.
    fn examples(&mut self, prefix: &str, media: &[(&'a str, Node<'a>)], level: usize) {
        for (name, node) in media {
            let after = match media.len() {
                1 => String::new(),
                _ => format!(", as {}", code(name)),
            };
            let one = format!("{prefix} example");
            let mut written = false;
            if let Some(example) = node.value.get("example") {
                self.line(&label(&one, &after, ""));
                self.example_block(example, Some(name));
                written = true;
            }
            for (key, example) in node.entries_at("examples") {
                let Some(followed) = self.follow(example) else {
                    continue;
                };
                let example = followed.node;
                let title = followed.summary.or(example.text("summary"));
                let title = title.map(inline).unwrap_or_else(|| code(key));
                self.line(&label(&one, &after, &title));
                self.description(followed.description.or(example.text("description")), level);
                let value = example
                    .value
                    .get("value")
                    .or(example.value.get("dataValue"));
                if let Some(value) = value {
                    self.example_block(value, Some(name));
                } else if let Some(serialized) = example.str("serializedValue") {
                    self.example_block(&Value::from(serialized), Some(name));
                } else if let Some(external) = example.text("externalValue") {
                    self.line(&markdown::url(external.trim()));
                }
                written = true;
            }
            if written {
                continue;
            }
            let Some((schema, _)) = node
                .get("schema")
                .and_then(|schema| self.expand(schema, &[], false))
            else {
                continue;
            };
            let examples: Vec<&Value> =
                match (schema.value.get("example"), schema.value.get("examples")) {
                    (Some(example), _) => vec![example],
                    (None, Some(Value::Array(examples))) => examples.iter().collect(),
                    _ => Vec::new(),
                };
            let bold = match examples.len() {
                0 => continue,
                1 => one,
                _ => format!("{prefix} examples"),
            };
            self.line(&label(&bold, &after, ""));
            for example in examples {
                self.example_block(example, Some(name));
            }
        }
    }

    /// A code block of `value`, an example of `media`, if it's for one: a
    /// string as it is, in the media type's language, and anything else as
    /// JSON.
    pub fn example_block(&mut self, value: &Value, media: Option<&str>) {
        let (text, language) = match (value, media) {
            (Value::String(text), Some(media)) => {
                let text = text.strip_suffix('\n').unwrap_or(text);
                (text.to_string(), language(media))
            }
            (value, _) => (
                serde_json::to_string_pretty(value).unwrap_or_default(),
                "json",
            ),
        };
        let fence = fence_for(&text);
        self.line(&format!("{fence}{language}\n{text}\n{fence}"));
    }
}

/// Whether `value` is nothing, or blank text.
fn blank(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(text) => text.trim().is_empty(),
        _ => false,
    }
}

/// `text`, linked to `url`, if it's to another site, or else with `url`
/// after it, in code; or either alone.
fn titled_link(text: Option<&str>, url: Option<&str>) -> Option<String> {
    match (text, url.map(str::trim)) {
        (Some(text), Some(url)) if web(url) => Some(link(&escape(text), url)),
        (Some(text), Some(url)) => Some(format!("{} ({})", inline(text), code(url))),
        (Some(text), None) => Some(inline(text)),
        (None, Some(url)) => Some(markdown::url(url)),
        (None, None) => None,
    }
}

/// A link to the external docs `docs` describes.
fn external(docs: Node) -> Option<String> {
    titled_link(docs.text("description"), docs.text("url"))
}

/// The language of a code block of `media`, like `json` for
/// `application/problem+json`, or none.
fn language(media: &str) -> &'static str {
    let media = media
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let subtype = media
        .split_once('/')
        .map_or(media.as_str(), |(_, subtype)| subtype);
    match subtype {
        "json" => "json",
        "xml" => "xml",
        "yaml" | "x-yaml" | "yml" => "yaml",
        "html" => "html",
        subtype if subtype.ends_with("+json") => "json",
        subtype if subtype.ends_with("+xml") => "xml",
        subtype if subtype.ends_with("+yaml") => "yaml",
        _ => "",
    }
}

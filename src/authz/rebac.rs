//! Relationship-based access control (Zanzibar style): tuples, a schema,
//! a store and the check that walks them.
//!
//! A **tuple** says a subject has a relation to an object:
//! `doc:42#editor@user:alice`, or with a set of subjects
//! `doc:42#viewer@group:eng#member` (every member of `group:eng`).
//!
//! The **schema** says, per object type, how each relation is made up — a
//! union (`|`) of terms:
//!
//! | Term | Means |
//! |---|---|
//! | `user` | tuples naming a `user:…` directly |
//! | `group#member` | tuples naming `group:…#member` — those members |
//! | `owner` | whoever has `owner` on the same object |
//! | `parent->viewer` | the viewers of whatever this object's `parent` is |
//!
//! ```rust
//! use rust_web_server::authz::{Authorizer, MemoryStore, Schema};
//!
//! let schema = Schema::new()
//!     .relation("group", "member", "user").unwrap()
//!     .relation("folder", "viewer", "user | group#member").unwrap()
//!     .relation("doc", "parent", "folder").unwrap()
//!     .relation("doc", "owner", "user").unwrap()
//!     .relation("doc", "viewer", "user | owner | parent->viewer").unwrap();
//! let authz = Authorizer::new(schema, MemoryStore::new()).unwrap();
//! for tuple in ["group:eng#member@user:bob", "folder:specs#viewer@group:eng#member", "doc:42#parent@folder:specs"] {
//!     authz.write_str(tuple).unwrap();
//! }
//! assert!(authz.check("user:bob", "viewer", "doc:42").unwrap());
//! assert!(!authz.check("user:eve", "viewer", "doc:42").unwrap());
//! ```

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, RwLock};

/// The longest chain of relations a check follows.
pub const MAX_DEPTH: usize = 25;
/// The most store reads one check makes.
pub const MAX_READS: usize = 1000;
/// Longest type name, id or relation name accepted.
pub const MAX_NAME: usize = 256;

/// What can go wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthzError {
    /// Text that isn't a tuple, object, subject or relation expression.
    Parse(String),
    /// Not allowed by the schema (unknown type or relation, a subject the
    /// relation doesn't take).
    Schema(String),
    /// The check hit [`MAX_DEPTH`] or [`MAX_READS`] before an answer.
    Limit(String),
    /// The tuple store failed.
    Store(String),
}

impl fmt::Display for AuthzError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthzError::Parse(e) => write!(f, "parse error: {e}"),
            AuthzError::Schema(e) => write!(f, "schema: {e}"),
            AuthzError::Limit(e) => write!(f, "limit: {e}"),
            AuthzError::Store(e) => write!(f, "store: {e}"),
        }
    }
}

impl std::error::Error for AuthzError {}

fn name(text: &str, what: &str) -> Result<String, AuthzError> {
    let ok = !text.is_empty()
        && text.len() <= MAX_NAME
        && text.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'));
    if ok {
        Ok(text.to_string())
    } else {
        Err(AuthzError::Parse(format!("{what} {text:?}: letters, digits, '_', '-' and '.' only, 1–{MAX_NAME} characters")))
    }
}

/// An id: anything but `#` (it ends an object), spaces and control
/// characters — `@` is fine (`user:alice@example.com`): a tuple's object
/// ends at its `#` and the relation, a name, at the next `@`.
fn id(text: &str) -> Result<String, AuthzError> {
    let ok = !text.is_empty() && text.len() <= MAX_NAME && !text.bytes().any(|b| b == b'#' || b == b' ' || b.is_ascii_control());
    if ok {
        Ok(text.to_string())
    } else {
        Err(AuthzError::Parse(format!("id {text:?}: 1–{MAX_NAME} characters, no '#', spaces or control characters")))
    }
}

/// An object: `type:id`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectRef {
    pub kind: String,
    pub id: String,
}

impl ObjectRef {
    pub fn parse(text: &str) -> Result<ObjectRef, AuthzError> {
        let (kind, rest) = text.split_once(':').ok_or_else(|| AuthzError::Parse(format!("object {text:?}: expected type:id")))?;
        Ok(ObjectRef { kind: name(kind, "type")?, id: id(rest)? })
    }
}

impl fmt::Display for ObjectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind, self.id)
    }
}

/// Who a tuple names: an object (`user:alice`) or a set (`group:eng#member`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Subject {
    pub object: ObjectRef,
    pub relation: Option<String>,
}

impl Subject {
    pub fn parse(text: &str) -> Result<Subject, AuthzError> {
        match text.split_once('#') {
            Some((object, relation)) => Ok(Subject { object: ObjectRef::parse(object)?, relation: Some(name(relation, "relation")?) }),
            None => Ok(Subject { object: ObjectRef::parse(text)?, relation: None }),
        }
    }
}

impl fmt::Display for Subject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.relation {
            Some(relation) => write!(f, "{}#{relation}", self.object),
            None => write!(f, "{}", self.object),
        }
    }
}

/// `object#relation@subject`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tuple {
    pub object: ObjectRef,
    pub relation: String,
    pub subject: Subject,
}

impl Tuple {
    pub fn parse(text: &str) -> Result<Tuple, AuthzError> {
        let text = text.trim();
        let malformed = || AuthzError::Parse(format!("tuple {text:?}: expected object#relation@subject"));
        let (object, rest) = text.split_once('#').ok_or_else(malformed)?;
        let (relation, subject) = rest.split_once('@').ok_or_else(malformed)?;
        Ok(Tuple { object: ObjectRef::parse(object)?, relation: name(relation, "relation")?, subject: Subject::parse(subject)? })
    }
}

impl fmt::Display for Tuple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}@{}", self.object, self.relation, self.subject)
    }
}

/// One term of a relation's definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Term {
    /// `user`: tuples naming an object of this type.
    Direct(String),
    /// `group#member`: tuples naming this type's objects with that relation.
    DirectSet(String, String),
    /// `owner`: another relation on the same object.
    Computed(String),
    /// `parent->viewer`: `viewer` on the objects this one's `parent` names.
    TupleToUserset { tupleset: String, computed: String },
}

/// Parse a relation definition: terms separated by `|`. A bare name is a
/// type when the schema has such a type, else a relation on the same object
/// (resolved by [`Schema::relation`]); here it comes back as `Direct`.
pub fn parse_expression(text: &str) -> Result<Vec<Term>, AuthzError> {
    if text.len() > 4096 {
        return Err(AuthzError::Parse("relation definition over 4096 characters".to_string()));
    }
    let mut terms = Vec::new();
    for part in text.split('|') {
        let part = part.trim();
        let term = if let Some((tupleset, computed)) = part.split_once("->") {
            Term::TupleToUserset { tupleset: name(tupleset.trim(), "relation")?, computed: name(computed.trim(), "relation")? }
        } else if let Some((kind, relation)) = part.split_once('#') {
            Term::DirectSet(name(kind.trim(), "type")?, name(relation.trim(), "relation")?)
        } else {
            Term::Direct(name(part, "type or relation")?)
        };
        terms.push(term);
    }
    if terms.is_empty() {
        return Err(AuthzError::Parse("empty relation definition".to_string()));
    }
    Ok(terms)
}

/// Object types and their relations.
#[derive(Clone, Debug, Default)]
pub struct Schema {
    types: HashMap<String, HashMap<String, Vec<Term>>>,
    /// Definitions as written, in order, for resolving and reporting.
    written: Vec<(String, String, String)>,
}

impl Schema {
    pub fn new() -> Schema {
        Schema::default()
    }

    /// Define `relation` on `kind` as `definition` (see [`parse_expression`]).
    /// Names are resolved when the schema is used ([`Schema::validate`]), so
    /// definitions may refer to types defined later.
    pub fn relation(mut self, kind: &str, relation: &str, definition: &str) -> Result<Schema, AuthzError> {
        let (kind, relation) = (name(kind, "type")?, name(relation, "relation")?);
        let terms = parse_expression(definition)?;
        self.types.entry(kind.clone()).or_default().insert(relation.clone(), terms);
        self.written.push((kind, relation, definition.to_string()));
        Ok(self)
    }

    /// A type with no relations of its own (e.g. `user`).
    pub fn object_type(mut self, kind: &str) -> Result<Schema, AuthzError> {
        self.types.entry(name(kind, "type")?).or_default();
        Ok(self)
    }

    /// The type names, sorted.
    pub fn types(&self) -> Vec<&str> {
        let mut types: Vec<&str> = self.types.keys().map(String::as_str).collect();
        types.sort_unstable();
        types
    }

    /// The relation names of `kind`, sorted.
    pub fn relations(&self, kind: &str) -> Vec<&str> {
        let mut relations: Vec<&str> = self.types.get(kind).map(|r| r.keys().map(String::as_str).collect()).unwrap_or_default();
        relations.sort_unstable();
        relations
    }

    pub fn has_relation(&self, kind: &str, relation: &str) -> bool {
        self.types.get(kind).is_some_and(|r| r.contains_key(relation))
    }

    /// Resolve bare names (a type used as a subject type, or else a relation
    /// on the same object) and check every reference; returns the resolved
    /// schema. Subject types that appear only in definitions (`user`) become
    /// types.
    pub fn validate(&self) -> Result<Schema, AuthzError> {
        let mut known: HashSet<String> = self.types.keys().cloned().collect();
        let mut resolved = self.clone();
        for (kind, relations) in &self.types {
            for (relation, terms) in relations {
                let mut out = Vec::with_capacity(terms.len());
                for term in terms {
                    out.push(match term {
                        Term::Direct(word) if relations.contains_key(word) && !self.types.contains_key(word) => Term::Computed(word.clone()),
                        Term::Direct(word) => {
                            known.insert(word.clone());
                            Term::Direct(word.clone())
                        }
                        Term::DirectSet(set_kind, set_relation) => {
                            if !self.has_relation(set_kind, set_relation) {
                                return Err(AuthzError::Schema(format!("{kind}#{relation}: {set_kind}#{set_relation} isn't defined")));
                            }
                            term.clone()
                        }
                        Term::TupleToUserset { tupleset, .. } => {
                            let Some(set_terms) = relations.get(tupleset) else {
                                return Err(AuthzError::Schema(format!("{kind}#{relation}: {tupleset} isn't a relation of {kind}")));
                            };
                            if !set_terms.iter().all(|t| matches!(t, Term::Direct(_))) {
                                return Err(AuthzError::Schema(format!("{kind}#{relation}: {tupleset} must name objects directly (only types, no sets or relations)")));
                            }
                            term.clone()
                        }
                        Term::Computed(other) => {
                            if !relations.contains_key(other) {
                                return Err(AuthzError::Schema(format!("{kind}#{relation}: {other} isn't a relation of {kind}")));
                            }
                            term.clone()
                        }
                    });
                }
                resolved.types.get_mut(kind).expect("kind").insert(relation.clone(), out);
            }
        }
        // `parent->viewer`: viewer must exist on every type parent names.
        for (kind, relations) in &resolved.types {
            for (relation, terms) in relations {
                for term in terms {
                    if let Term::TupleToUserset { tupleset, computed } = term {
                        for target in relations[tupleset].iter().filter_map(|t| if let Term::Direct(k) = t { Some(k) } else { None }) {
                            if !resolved.has_relation(target, computed) {
                                return Err(AuthzError::Schema(format!("{kind}#{relation}: {tupleset}->{computed} needs {computed} on {target}")));
                            }
                        }
                    }
                }
            }
        }
        for kind in known {
            resolved.types.entry(kind).or_default();
        }
        Ok(resolved)
    }

    fn terms(&self, kind: &str, relation: &str) -> Option<&[Term]> {
        self.types.get(kind)?.get(relation).map(Vec::as_slice)
    }

    /// Whether the schema lets `tuple` be stored: a known relation whose
    /// definition takes this kind of subject directly.
    pub fn allows(&self, tuple: &Tuple) -> Result<(), AuthzError> {
        let terms = self
            .terms(&tuple.object.kind, &tuple.relation)
            .ok_or_else(|| AuthzError::Schema(format!("{}#{} isn't defined", tuple.object.kind, tuple.relation)))?;
        let fits = terms.iter().any(|term| match (term, &tuple.subject.relation) {
            (Term::Direct(kind), None) => *kind == tuple.subject.object.kind,
            (Term::DirectSet(kind, relation), Some(r)) => *kind == tuple.subject.object.kind && relation == r,
            _ => false,
        });
        if fits {
            Ok(())
        } else {
            Err(AuthzError::Schema(format!("{}#{} doesn't take {} as a subject", tuple.object.kind, tuple.relation, tuple.subject)))
        }
    }
}

/// Which tuples to list: any field left `None` matches everything.
#[derive(Clone, Debug, Default)]
pub struct TupleFilter {
    pub object: Option<ObjectRef>,
    pub object_kind: Option<String>,
    pub relation: Option<String>,
    pub subject: Option<Subject>,
}

impl TupleFilter {
    pub fn matches(&self, tuple: &Tuple) -> bool {
        self.object.as_ref().is_none_or(|o| *o == tuple.object)
            && self.object_kind.as_ref().is_none_or(|k| *k == tuple.object.kind)
            && self.relation.as_ref().is_none_or(|r| *r == tuple.relation)
            && self.subject.as_ref().is_none_or(|s| *s == tuple.subject)
    }
}

/// Where tuples live.
pub trait TupleStore: Send + Sync {
    /// Store `tuple`; `true` if it wasn't there.
    fn write(&self, tuple: &Tuple) -> Result<bool, AuthzError>;
    /// Remove `tuple`; `true` if it was there.
    fn delete(&self, tuple: &Tuple) -> Result<bool, AuthzError>;
    /// The subjects with `relation` on `object`.
    fn read(&self, object: &ObjectRef, relation: &str) -> Result<Vec<Subject>, AuthzError>;
    /// Tuples matching `filter`, at most `limit`.
    fn list(&self, filter: &TupleFilter, limit: usize) -> Result<Vec<Tuple>, AuthzError>;
}

/// Tuples in memory: for tests, small apps, and tuples seeded from the
/// config file. Lost on restart.
#[derive(Default)]
pub struct MemoryStore {
    tuples: RwLock<HashMap<(ObjectRef, String), BTreeSet<Subject>>>,
}

impl MemoryStore {
    pub fn new() -> MemoryStore {
        MemoryStore::default()
    }
}

impl TupleStore for MemoryStore {
    fn write(&self, tuple: &Tuple) -> Result<bool, AuthzError> {
        let mut tuples = self.tuples.write().unwrap_or_else(|e| e.into_inner());
        Ok(tuples.entry((tuple.object.clone(), tuple.relation.clone())).or_default().insert(tuple.subject.clone()))
    }

    fn delete(&self, tuple: &Tuple) -> Result<bool, AuthzError> {
        let mut tuples = self.tuples.write().unwrap_or_else(|e| e.into_inner());
        let key = (tuple.object.clone(), tuple.relation.clone());
        let removed = tuples.get_mut(&key).is_some_and(|subjects| subjects.remove(&tuple.subject));
        if tuples.get(&key).is_some_and(BTreeSet::is_empty) {
            tuples.remove(&key);
        }
        Ok(removed)
    }

    fn read(&self, object: &ObjectRef, relation: &str) -> Result<Vec<Subject>, AuthzError> {
        let tuples = self.tuples.read().unwrap_or_else(|e| e.into_inner());
        Ok(tuples.get(&(object.clone(), relation.to_string())).map(|s| s.iter().cloned().collect()).unwrap_or_default())
    }

    fn list(&self, filter: &TupleFilter, limit: usize) -> Result<Vec<Tuple>, AuthzError> {
        let tuples = self.tuples.read().unwrap_or_else(|e| e.into_inner());
        let mut out: Vec<Tuple> = tuples
            .iter()
            .flat_map(|((object, relation), subjects)| {
                subjects.iter().map(move |subject| Tuple { object: object.clone(), relation: relation.clone(), subject: subject.clone() })
            })
            .filter(|t| filter.matches(t))
            .collect();
        out.sort();
        out.truncate(limit);
        Ok(out)
    }
}

/// A schema and a tuple store: writes checked against the schema, and
/// [`check`](Self::check).
#[derive(Clone)]
pub struct Authorizer {
    schema: Arc<Schema>,
    store: Arc<dyn TupleStore>,
}

impl fmt::Debug for Authorizer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Authorizer").field("types", &self.schema.types()).finish_non_exhaustive()
    }
}

impl Authorizer {
    /// `schema` is validated (see [`Schema::validate`]).
    pub fn new(schema: Schema, store: impl TupleStore + 'static) -> Result<Authorizer, AuthzError> {
        Authorizer::with_store(schema, Arc::new(store))
    }

    pub fn with_store(schema: Schema, store: Arc<dyn TupleStore>) -> Result<Authorizer, AuthzError> {
        Ok(Authorizer { schema: Arc::new(schema.validate()?), store })
    }

    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    pub fn store(&self) -> &Arc<dyn TupleStore> {
        &self.store
    }

    /// Store `tuple` if the schema allows it; `true` if it was new.
    pub fn write(&self, tuple: &Tuple) -> Result<bool, AuthzError> {
        self.schema.allows(tuple)?;
        self.store.write(tuple)
    }

    /// [`write`](Self::write) from text (`doc:42#editor@user:alice`).
    pub fn write_str(&self, tuple: &str) -> Result<bool, AuthzError> {
        self.write(&Tuple::parse(tuple)?)
    }

    pub fn delete(&self, tuple: &Tuple) -> Result<bool, AuthzError> {
        self.store.delete(tuple)
    }

    /// Whether `subject` (`user:alice`) has `relation` on `object`
    /// (`doc:42`).
    pub fn check(&self, subject: &str, relation: &str, object: &str) -> Result<bool, AuthzError> {
        Ok(self.explain(subject, relation, object)?.is_some())
    }

    /// Like [`check`](Self::check), with the path that granted it: each step
    /// `object#relation`, ending with the tuple that named the subject.
    pub fn explain(&self, subject: &str, relation: &str, object: &str) -> Result<Option<Vec<String>>, AuthzError> {
        self.explain_subject(&Subject::parse(subject)?, relation, &ObjectRef::parse(object)?)
    }

    /// [`check`](Self::check) with a subject and object already parsed —
    /// for a caller, so an id is never re-read as text.
    pub fn check_subject(&self, subject: &Subject, relation: &str, object: &ObjectRef) -> Result<bool, AuthzError> {
        Ok(self.explain_subject(subject, relation, object)?.is_some())
    }

    /// [`explain`](Self::explain) with parsed parts.
    pub fn explain_subject(&self, subject: &Subject, relation: &str, object: &ObjectRef) -> Result<Option<Vec<String>>, AuthzError> {
        if !self.schema.has_relation(&object.kind, relation) {
            return Err(AuthzError::Schema(format!("{}#{relation} isn't defined", object.kind)));
        }
        Walk::new(self).run(subject, object.clone(), relation.to_string())
    }
}

/// One check: an explicit stack, a visited set and budgets — no recursion,
/// so a deep or cyclic tuple graph can't overflow the stack or loop.
struct Walk<'a> {
    authz: &'a Authorizer,
    reads: usize,
    /// `(object, relation, parent index)`; the stack holds indexes into it.
    nodes: Vec<(ObjectRef, String, Option<usize>, usize)>,
}

impl<'a> Walk<'a> {
    fn new(authz: &'a Authorizer) -> Walk<'a> {
        Walk { authz, reads: 0, nodes: Vec::new() }
    }

    fn read(&mut self, object: &ObjectRef, relation: &str) -> Result<Vec<Subject>, AuthzError> {
        self.reads += 1;
        if self.reads > MAX_READS {
            return Err(AuthzError::Limit(format!("more than {MAX_READS} tuple reads")));
        }
        self.authz.store.read(object, relation)
    }

    fn path(&self, mut at: usize, last: String) -> Vec<String> {
        let mut steps = vec![last];
        loop {
            let (object, relation, parent, _) = &self.nodes[at];
            steps.push(format!("{object}#{relation}"));
            match parent {
                Some(p) => at = *p,
                None => break,
            }
        }
        steps.reverse();
        steps
    }

    fn run(mut self, subject: &Subject, object: ObjectRef, relation: String) -> Result<Option<Vec<String>>, AuthzError> {
        let mut visited: HashSet<(ObjectRef, String)> = HashSet::new();
        self.nodes.push((object, relation, None, 0));
        let mut stack = vec![0usize];
        while let Some(at) = stack.pop() {
            let (object, relation, _, depth) = self.nodes[at].clone();
            if !visited.insert((object.clone(), relation.clone())) {
                continue;
            }
            // The goal itself: asking about `group:eng#member` for `group:eng#member`.
            if subject.relation.as_deref() == Some(relation.as_str()) && subject.object == object {
                return Ok(Some(self.path(at, format!("{subject}"))));
            }
            if depth >= MAX_DEPTH {
                return Err(AuthzError::Limit(format!("deeper than {MAX_DEPTH} relations")));
            }
            let Some(terms) = self.authz.schema.terms(&object.kind, &relation) else { continue };
            let terms = terms.to_vec();
            let mut direct_read = false;
            for term in &terms {
                match term {
                    Term::Direct(_) | Term::DirectSet(..) => {
                        if direct_read {
                            continue;
                        }
                        direct_read = true;
                        for found in self.read(&object, &relation)? {
                            if found == *subject {
                                return Ok(Some(self.path(at, format!("{object}#{relation}@{found}"))));
                            }
                            if let Some(set_relation) = &found.relation {
                                self.nodes.push((found.object.clone(), set_relation.clone(), Some(at), depth + 1));
                                stack.push(self.nodes.len() - 1);
                            }
                        }
                    }
                    Term::Computed(other) => {
                        self.nodes.push((object.clone(), other.clone(), Some(at), depth + 1));
                        stack.push(self.nodes.len() - 1);
                    }
                    Term::TupleToUserset { tupleset, computed } => {
                        for found in self.read(&object, tupleset)? {
                            if found.relation.is_none() {
                                self.nodes.push((found.object.clone(), computed.clone(), Some(at), depth + 1));
                                stack.push(self.nodes.len() - 1);
                            }
                        }
                    }
                }
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests;

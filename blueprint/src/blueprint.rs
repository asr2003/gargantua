use std::collections::BTreeSet;

use async_graphql_parser::types::ServiceDocument;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use valid::{Valid, ValidationError, Validator};

use crate::{error::Error, index::Index};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Blueprint {
    pub definitions: Vec<Definition>,
    pub schema: SchemaDefinition,
    pub directives: Vec<DirectiveDefinition>,
    pub join_graphs: Vec<JoinGraph>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
pub struct Graph(String);
impl Graph {
    pub fn new<A: AsRef<str>>(name: A) -> Self {
        Graph(name.as_ref().to_string())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
pub struct JoinGraph {
    pub name: Graph,
    pub url: url::Url,
}

impl Blueprint {
    pub fn parse_doc(doc: ServiceDocument) -> Blueprint {
        // TODO: drop the unwrap after parse drops the Valid type
        super::build::parse(doc).to_result().unwrap()
    }

    pub fn parse(schema: String) -> Valid<Blueprint, Error> {
        Valid::from(
            async_graphql_parser::parse_schema(schema)
                .map_err(|e| ValidationError::new(Error::from(e))),
        )
        .map(Blueprint::parse_doc)
    }

    pub fn to_index(&self) -> Index {
        Index::from(self)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Definition {
    Interface(InterfaceTypeDefinition),
    Object(ObjectTypeDefinition),
    InputObject(InputObjectTypeDefinition),
    Scalar(ScalarTypeDefinition),
    Enum(EnumTypeDefinition),
    Union(UnionTypeDefinition),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InterfaceTypeDefinition {
    pub name: String,
    pub fields: Vec<FieldDefinition>,
    pub description: Option<String>,
    pub join_types: Vec<JoinType>,
    pub join_implements: Vec<JoinImplements>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectTypeDefinition {
    pub name: String,
    pub fields: Vec<FieldDefinition>,
    pub description: Option<String>,
    pub implements: BTreeSet<String>,
    pub join_types: Vec<JoinType>,
    pub join_implements: Vec<JoinImplements>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InputObjectTypeDefinition {
    pub name: String,
    pub fields: Vec<InputFieldDefinition>,
    pub description: Option<String>,
    pub join_types: Vec<JoinType>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnumTypeDefinition {
    pub name: String,
    pub directives: Vec<Directive>,
    pub description: Option<String>,
    pub enum_values: Vec<EnumValueDefinition>,
    pub join_types: Vec<JoinType>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnumValueDefinition {
    pub description: Option<String>,
    pub name: String,
    pub directives: Vec<Directive>,
    pub join_enums: Vec<JoinEnum>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SchemaDefinition {
    pub query: Option<String>,
    pub mutation: Option<String>,
    pub subscription: Option<String>,
    pub directives: Vec<Directive>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InputFieldDefinition {
    pub name: String,
    pub of_type: Type,
    pub default_value: Option<Value>,
    pub description: Option<String>,
    pub join_fields: Vec<JoinField>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FieldDefinition {
    pub name: String,
    pub args: Vec<InputFieldDefinition>,
    pub of_type: Type,
    pub directives: Vec<Directive>,
    pub description: Option<String>,
    pub join_fields: Vec<JoinField>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Directive {
    pub name: String,
    pub arguments: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DirectiveDefinition {
    pub name: String,
    pub description: Option<String>,
    pub arguments: Vec<InputFieldDefinition>,
    pub repeatable: bool,
    pub locations: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScalarTypeDefinition {
    pub name: String,
    pub directives: Vec<Directive>,
    pub description: Option<String>,
    pub join_types: Vec<JoinType>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnionTypeDefinition {
    pub name: String,
    pub directives: Vec<Directive>,
    pub description: Option<String>,
    pub types: BTreeSet<String>,
    pub join_types: Vec<JoinType>,
    pub join_unions: Vec<JoinUnion>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JoinType {
    pub graph: Graph,
    pub key: Option<String>,
    #[serde(default = "default_false")]
    pub extension: bool,
    #[serde(default = "default_true")]
    pub resolvable: bool,
    #[serde(default = "default_false")]
    pub is_interface_object: bool,
}

fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JoinEnum {
    pub graph: Graph,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JoinField {
    pub graph: Option<Graph>,
    pub requires: Option<String>,
    pub provides: Option<String>,
    pub r#type: Option<String>,
    pub external: Option<bool>,
    pub r#override: Option<String>,
    pub used_overridden: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JoinImplements {
    pub graph: Graph,
    pub interface: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JoinUnion {
    pub graph: Graph,
    pub member: String,
}

/// Type to represent GraphQL type usage with modifiers
/// [spec](https://spec.graphql.org/October2021/#sec-Wrapping-Types)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Type {
    Named {
        /// Name of the type
        name: String,
        /// Flag to indicate the type is required.
        required: bool,
    },
    List {
        /// Type is a list
        of_type: Box<Type>,
        /// Flag to indicate the type is required.
        non_null: bool,
    },
}

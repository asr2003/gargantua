use std::collections::{BTreeMap, BTreeSet};

use async_graphql_parser::Positioned;
use async_graphql_value::{ConstValue, Name};
use serde::de::DeserializeOwned;

use crate::{
    Blueprint, Definition, Directive, DirectiveDefinition, EnumValueDefinition, FieldDefinition,
    InputFieldDefinition, JoinEnum, JoinField, JoinGraph, JoinImplements, JoinType, JoinUnion,
    SchemaDefinition, Type,
};

// Reading a super-graph configuration is infallible
pub fn parse(doc: async_graphql_parser::types::ServiceDocument) -> Blueprint {
    let mut root_schema = SchemaDefinition {
        query: None,
        mutation: None,
        subscription: None,
        directives: Vec::new(),
    };
    let mut definitions = BTreeMap::<String, Definition>::new();
    let mut directives = BTreeMap::<String, DirectiveDefinition>::new();

    for definition in doc.definitions.into_iter() {
        match definition {
            async_graphql_parser::types::TypeSystemDefinition::Schema(Positioned {
                pos: _,
                node: schema_node,
            }) => {
                let schema = parse_schema(schema_node);

                if let (None, Some(query)) = (&root_schema.query, schema.query) {
                    root_schema.query = Some(query)
                }

                if let (None, Some(mutation)) = (&root_schema.mutation, schema.mutation) {
                    root_schema.mutation = Some(mutation)
                }

                if let (None, Some(subscription)) = (&root_schema.subscription, schema.subscription)
                {
                    root_schema.subscription = Some(subscription)
                }

                // TODO: validate that non-repetitive directive is not defined twice
                let directives = root_schema
                    .directives
                    .clone()
                    .into_iter()
                    .chain(schema.directives.into_iter())
                    .collect::<Vec<_>>();

                root_schema.directives = directives;
            }
            async_graphql_parser::types::TypeSystemDefinition::Type(Positioned {
                pos: _,
                node: type_node,
            }) => {
                let name = type_node.name.clone().into_inner().to_string();

                let definition = parse_type(type_node);
                definitions.insert(name, definition);
            }
            async_graphql_parser::types::TypeSystemDefinition::Directive(Positioned {
                pos: _,
                node: directive_node,
            }) => {
                let name = directive_node.name.clone().into_inner().to_string();
                let directive = parse_directive_definition(directive_node);
                directives.insert(name.clone(), directive);
            }
        }
    }
    let definitions = definitions.clone().into_values().collect::<Vec<_>>();
    let directives = directives.into_values().collect::<Vec<_>>();

    let join_graphs = parse_join_graphs(definitions.clone());

    Blueprint { definitions, schema: root_schema, directives, join_graphs }
}

fn parse_directive_definition(
    directive_node: async_graphql_parser::types::DirectiveDefinition,
) -> DirectiveDefinition {
    let name = directive_node.name.into_inner().to_string();
    let repeatable = directive_node.is_repeatable;
    let description = directive_node.description.map(|d| d.to_string());

    let arguments: Vec<_> = directive_node
        .arguments
        .into_iter()
        .map(|Positioned { pos: _, node: input_field_node }| parse_input_field(input_field_node))
        .collect();

    let locations: Vec<String> = directive_node
        .locations
        .into_iter()
        .map(|loc| match loc.into_inner() {
            async_graphql_parser::types::DirectiveLocation::Query => "Query".into(),
            async_graphql_parser::types::DirectiveLocation::Mutation => "Mutation".into(),
            async_graphql_parser::types::DirectiveLocation::Subscription => "Subscription".into(),
            async_graphql_parser::types::DirectiveLocation::Field => "Field".into(),
            async_graphql_parser::types::DirectiveLocation::FragmentDefinition => {
                "FragmentDefinition".into()
            }
            async_graphql_parser::types::DirectiveLocation::FragmentSpread => {
                "FragmentSpread".into()
            }
            async_graphql_parser::types::DirectiveLocation::InlineFragment => {
                "InlineFragment".into()
            }
            async_graphql_parser::types::DirectiveLocation::Schema => "Schema".into(),
            async_graphql_parser::types::DirectiveLocation::Scalar => "Scalar".into(),
            async_graphql_parser::types::DirectiveLocation::Object => "Object".into(),
            async_graphql_parser::types::DirectiveLocation::FieldDefinition => {
                "FieldDefinition".into()
            }
            async_graphql_parser::types::DirectiveLocation::ArgumentDefinition => {
                "ArgumentDefinition".into()
            }
            async_graphql_parser::types::DirectiveLocation::Interface => "Interface".into(),
            async_graphql_parser::types::DirectiveLocation::Union => "Union".into(),
            async_graphql_parser::types::DirectiveLocation::Enum => "Enum".into(),
            async_graphql_parser::types::DirectiveLocation::EnumValue => "EnumValue".into(),
            async_graphql_parser::types::DirectiveLocation::InputObject => "InputObject".into(),
            async_graphql_parser::types::DirectiveLocation::InputFieldDefinition => {
                "InputFieldDefinition".into()
            }
            async_graphql_parser::types::DirectiveLocation::VariableDefinition => {
                "VariableDefinition".into()
            }
        })
        .collect();

    DirectiveDefinition { name, description, arguments, repeatable, locations }
}

fn parse_type(type_node: async_graphql_parser::types::TypeDefinition) -> Definition {
    let name = type_node.name.to_string();
    let description = type_node.description.map(|d| d.to_string());
    let directives: Vec<_> = type_node
        .directives
        .clone()
        .into_iter()
        .map(|Positioned { pos: _, node: directive_node }| parse_directive(directive_node))
        .collect();

    match type_node.kind {
        async_graphql_parser::types::TypeKind::Scalar => {
            let join_types: Vec<JoinType> = find_directive(&directives, "join__type");
            Definition::Scalar(crate::ScalarTypeDefinition {
                name,
                directives,
                description,
                join_types,
            })
        }
        async_graphql_parser::types::TypeKind::Object(object_type) => {
            let fields = object_type
                .fields
                .into_iter()
                .map(|Positioned { pos: _, node: field_node }| parse_field(field_node))
                .collect();

            let implements = object_type
                .implements
                .into_iter()
                .map(|name| name.to_string())
                .collect::<BTreeSet<String>>();

            let join_types: Vec<JoinType> = find_directive(&directives, "join__type");
            let join_implements: Vec<JoinImplements> =
                find_directive(&directives, "join__implements");

            Definition::Object(crate::ObjectTypeDefinition {
                name,
                fields,
                description,
                implements,
                join_types,
                join_implements,
            })
        }
        async_graphql_parser::types::TypeKind::Interface(interface_type) => {
            let fields = interface_type
                .fields
                .into_iter()
                .map(|Positioned { pos: _, node: field_node }| parse_field(field_node))
                .collect();

            let join_types: Vec<JoinType> = find_directive(&directives, "join__type");
            let join_implements: Vec<JoinImplements> =
                find_directive(&directives, "join__implements");
            Definition::Interface(crate::InterfaceTypeDefinition {
                name,
                fields,
                description,
                join_implements,
                join_types,
            })
        }
        async_graphql_parser::types::TypeKind::Union(union_type) => {
            let types = union_type
                .members
                .into_iter()
                .map(|type_name| type_name.into_inner().to_string())
                .collect();

            let join_types: Vec<JoinType> = find_directive(&directives, "join__type");
            let join_unions: Vec<JoinUnion> = find_directive(&directives, "join__unionMember");

            Definition::Union(crate::UnionTypeDefinition {
                name,
                directives,
                description,
                types,
                join_types,
                join_unions,
            })
        }
        async_graphql_parser::types::TypeKind::Enum(enum_type) => {
            let enum_values = enum_type
                .values
                .into_iter()
                .map(|Positioned { pos: _, node: enum_node }| parse_enum(enum_node))
                .collect();

            let join_types: Vec<JoinType> = find_directive(&directives, "join__type");
            Definition::Enum(crate::EnumTypeDefinition {
                name,
                directives,
                description,
                enum_values,
                join_types,
            })
        }
        async_graphql_parser::types::TypeKind::InputObject(input_object_type) => {
            let fields = input_object_type
                .fields
                .into_iter()
                .map(|Positioned { pos: _, node: input_field_node }| {
                    parse_input_field(input_field_node)
                })
                .collect();

            let join_types: Vec<JoinType> = find_directive(&directives, "join__type");
            Definition::InputObject(crate::InputObjectTypeDefinition {
                name,
                fields,
                description,
                join_types,
            })
        }
    }
}

fn parse_enum(enum_node: async_graphql_parser::types::EnumValueDefinition) -> EnumValueDefinition {
    let name = enum_node.value.to_string();
    let description = enum_node.description.map(|d| d.to_string());
    let directives: Vec<_> = enum_node
        .directives
        .into_iter()
        .map(|Positioned { pos: _, node: directive_node }| parse_directive(directive_node))
        .collect();

    let join_enums: Vec<JoinEnum> = find_directive(&directives, "join__enumValue");
    EnumValueDefinition { description, name, directives, join_enums }
}

fn parse_field(field_node: async_graphql_parser::types::FieldDefinition) -> FieldDefinition {
    let name = field_node.name.to_string();
    let description = field_node.description.map(|d| d.to_string());
    let of_type = map_type(&field_node.ty.into_inner());

    let args = field_node
        .arguments
        .into_iter()
        .map(|Positioned { pos: _, node: arg_node }| parse_input_field(arg_node))
        .collect();

    let directives: Vec<_> = field_node
        .directives
        .into_iter()
        .map(|Positioned { pos: _, node: directive_node }| parse_directive(directive_node))
        .collect();

    let join_fields: Vec<JoinField> = find_directive(&directives, "join__field");

    FieldDefinition { name, args, of_type, directives, description, join_fields }
}

fn parse_input_field(
    input_field_node: async_graphql_parser::types::InputValueDefinition,
) -> InputFieldDefinition {
    let name = input_field_node.name.to_string();
    let description = input_field_node.description.map(|d| d.to_string());
    let of_type = map_type(&input_field_node.ty.into_inner());

    let default_value = input_field_node
        .default_value
        .map(|Positioned { pos: _, node: argument_value }| parse_argument(argument_value));

    let directives: Vec<_> = input_field_node
        .directives
        .into_iter()
        .map(|Positioned { pos: _, node: directive_node }| parse_directive(directive_node))
        .collect();

    let join_fields: Vec<JoinField> = find_directive(&directives, "join__field");

    InputFieldDefinition { name, of_type, default_value, description, join_fields }
}

fn map_type(type_: &async_graphql_parser::types::Type) -> Type {
    match &type_.base {
        async_graphql_parser::types::BaseType::Named(name) => {
            Type::Named { name: name.to_string(), required: !type_.nullable }
        }
        async_graphql_parser::types::BaseType::List(inner_type) => Type::List {
            of_type: Box::new(map_type(inner_type.as_ref())),
            non_null: !type_.nullable,
        },
    }
}

fn parse_schema(schema_node: async_graphql_parser::types::SchemaDefinition) -> SchemaDefinition {
    let query = if let Some(Positioned { pos: _, node: query_node }) = schema_node.query {
        Some(query_node.to_string())
    } else {
        None
    };

    let mutation = if let Some(Positioned { pos: _, node: mutation_node }) = schema_node.mutation {
        Some(mutation_node.to_string())
    } else {
        None
    };

    let subscription =
        if let Some(Positioned { pos: _, node: subscription_node }) = schema_node.subscription {
            Some(subscription_node.to_string())
        } else {
            None
        };

    let directives = schema_node
        .directives
        .into_iter()
        .map(|Positioned { pos: _, node: directive_node }| parse_directive(directive_node))
        .collect();

    SchemaDefinition { query, mutation, subscription, directives }
}

fn parse_directive(directive_node: async_graphql_parser::types::ConstDirective) -> Directive {
    let arguments = parse_arguments(directive_node.arguments);

    Directive { name: directive_node.name.to_string(), arguments }
}

fn parse_arguments(
    arguments: Vec<(Positioned<Name>, Positioned<ConstValue>)>,
) -> serde_json::Value {
    let map: BTreeMap<String, serde_json::Value> = arguments
        .into_iter()
        .map(
            |(
                Positioned { pos: _, node: argument_name },
                Positioned { pos: _, node: argument_node },
            )| { (argument_name.to_string(), parse_argument(argument_node)) },
        )
        .collect();
    serde_json::Value::Object(map.into_iter().collect())
}

fn parse_argument(argument_node: async_graphql_value::ConstValue) -> serde_json::Value {
    argument_node.into_json().unwrap()
}

fn parse_join_graphs(definitions: Vec<Definition>) -> Vec<JoinGraph> {
    definitions
        .into_iter()
        .find_map(|definition| {
            if let Definition::Enum(enumeration) = definition {
                if enumeration.name == "join__Graph" {
                    let data =
                        enumeration
                            .enum_values
                            .into_iter()
                            .fold(Vec::new(), |mut acc, cur| {
                                let mut join_graphs: Vec<JoinGraph> =
                                    find_directive(&cur.directives, "join__graph");

                                acc.append(&mut join_graphs);
                                acc
                            });

                    Some(data)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .expect("Enumeration `join__Graph` is not found")
}

fn find_directive<Value: DeserializeOwned>(directives: &[Directive], name: &str) -> Vec<Value> {
    directives
        .iter()
        .filter(|directive| directive.name == name)
        .map(|directive| serde_json::from_value(directive.arguments.clone()).unwrap())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        let graphql = resource::resource_str!("./src/fixtures/router.graphql");
        let document = async_graphql_parser::parse_schema(graphql).unwrap();
        let blueprint = parse(document);
        let blueprint = serde_json::to_string_pretty(&blueprint).unwrap();
        insta::assert_snapshot!(blueprint);
    }
}

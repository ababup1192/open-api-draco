extern crate yaml_rust;
use yaml_rust::{YamlEmitter, YamlLoader};
extern crate regex;
#[macro_use]
extern crate maplit;

fn main() {
  let s = "
foo:
    - list1
    - list2
bar:
    - 1
    - 2.0
";
  let docs = YamlLoader::load_from_str(s).unwrap();

  // Multi document support, doc is a yaml::Yaml
  let doc = &docs[0];

  // Debug support
  println!("{:?}", doc);

  // Index access for map & array
  assert_eq!(doc["foo"][0].as_str().unwrap(), "list1");
  assert_eq!(doc["bar"][1].as_f64().unwrap(), 2.0);

  // Chained key/array access is checked and won't panic,
  // return BadValue if they are not exist.
  assert!(doc["INVALID_KEY"][100].is_badvalue());

  // Dump the YAML object
  let mut out_str = String::new();
  {
    let mut emitter = YamlEmitter::new(&mut out_str);
    emitter.dump(doc).unwrap(); // dump the YAML object to a String
  }
  println!("{}", out_str);
}

pub mod apis {
  use std::collections::HashMap;

  #[derive(PartialEq, Clone, Debug)]
  pub struct Api {
    pub path: String,
    pub param_map: HashMap<String, ParamType>,
    pub method_map: HashMap<String, Method>,
  }

  #[derive(PartialEq, Clone, Debug)]
  pub enum ParamType {
    Integer,
    String,
  }

  #[derive(PartialEq, Clone, Debug)]
  pub struct Method {
    pub operation_id: String,
    pub summary: String,
    pub response_opt: Option<Content>,
    pub request_body_opt: Option<Content>,
  }

  #[derive(PartialEq, Clone, Debug)]
  pub enum Content {
    Array(Box<Content>),
    Object(Vec<Property>),
    String,
    Integer,
    Number,
    Boolean,
  }

  #[derive(PartialEq, Clone, Debug)]
  pub struct Property {
    pub key: String,
    pub value: Content,
  }

  pub fn from_yaml(yaml: &yaml_rust::Yaml) -> Vec<Api> {
    let paths = &yaml["paths"];

    fn create_param_tuple(param: yaml_rust::Yaml) -> (String, ParamType) {
      let schema_type = param["schema"]["type"].as_str();
      (
        param["name"].as_str().unwrap().to_string(),
        match schema_type {
          Some("integer") => ParamType::Integer,
          Some("string") => ParamType::String,
          _ => panic!("unexpected schema type: {}", schema_type.unwrap_or("None")),
        },
      )
    };

    fn create_method(method: yaml_rust::Yaml) -> Method {
      let request_body_schema =
        method["requestBody"]["content"]["application/json"]["schema"].clone();

      let response_schema =
        method["responses"]["200"]["content"]["application/json"]["schema"].clone();

      fn create_schema(base_doument: yaml_rust::Yaml) -> Option<Content> {
        base_doument["type"].as_str().and_then(|schema_type| {
          if schema_type == "object" {
            let property_keys = base_doument["properties"]
              .as_hash()
              .expect("can not get object properties")
              .keys()
              .map(|key| key.as_str().unwrap())
              .collect::<Vec<_>>();

            let properties = property_keys
              .into_iter()
              .map(|key| Property {
                key: key.to_string(),
                value: {
                  let prop_type_text = base_doument["properties"][key]["type"].as_str();

                  match prop_type_text {
                    Some("string") => Content::String,
                    Some("integer") => Content::Integer,
                    _ => panic!(
                      "unsuppoted property type: ({}: {})",
                      key,
                      prop_type_text.unwrap_or("None")
                    ),
                  }
                },
              })
              .collect::<Vec<_>>();

            Some(Content::Object(properties))
          } else {
            None
          }
        })
      }

      Method {
        operation_id: method["operationId"].as_str().unwrap().to_string(),
        summary: method["summary"]
          .as_str()
          .and_then(|summary| {
            if summary.trim().is_empty() {
              None
            } else {
              Some(summary)
            }
          })
          .expect("summary is empty")
          .to_string(),
        response_opt: create_schema(response_schema),
        request_body_opt: create_schema(request_body_schema),
      }
    }

    paths
      .as_hash()
      .expect("can not parse hash from paths")
      .keys()
      .map(|path| {
        let path = path.as_str().expect("can not get path");
        let path_methods = paths[path]
          .as_hash()
          .into_iter()
          .flat_map(|path_methods| path_methods.keys());

        Api {
          path: path.to_string(),
          param_map: paths[path]["parameters"]
            .clone()
            .into_iter()
            .map(create_param_tuple)
            .collect::<HashMap<_, _>>(),
          method_map: path_methods
            .map(|method| {
              method
                .as_str()
                .expect("can not get path methods(get, post, ...)")
            })
            .filter(|method| method != &"parameters")
            .map(|method| {
              (
                method.to_string(),
                create_method(paths[path][method].clone()),
              )
            })
            .collect::<HashMap<_, _>>(),
        }
      })
      .collect()
  }
  pub fn nomalize_play_variable_path(path: String) -> String {
    use regex::Regex;

    let modify_identifer = (|| {
      let variable_reg = Regex::new(r"^\{.*\}$").unwrap();
      let blace_reg = Regex::new(r"[{}]").unwrap();

      let f = move |variable: String| {
        if variable_reg.is_match(&variable) {
          format!(":{}", blace_reg.replace_all(&variable, ""))
        } else {
          variable
        }
      };
      f
    })();

    path
      .split("/")
      .map(|variable| modify_identifer(variable.to_string()))
      .collect::<Vec<String>>()
      .join("/")
  }

  pub fn to_play_routings(api: Api) -> Vec<String> {
    api
      .method_map
      .clone()
      .keys()
      .into_iter()
      .map(|method_type| {
        format!(
          "{} {} {{Method Name}}({})",
          match &method_type[..] {
            "get" => "GET",
            "put" => "PUT",
            m => panic!("unsupported method type {}", m),
          },
          nomalize_play_variable_path(api.path.clone()),
          api
            .clone()
            .param_map
            .into_iter()
            .map(|(param, param_type)| format!(
              "{}: {}",
              param,
              match param_type {
                ParamType::String => "String",
                ParamType::Integer => "Long",
              }
            ))
            .collect::<Vec<_>>()
            .join(", ")
        )
      })
      .collect()
  }

  pub fn generate_command_scala(method: Method) -> Option<String> {
    fn content_to_string(content: Content) -> String {
      match content {
        Content::Object(properties) => properties
          .into_iter()
          .map(|property| format!("{}: {}", property.key, content_to_string(property.value)))
          .collect::<Vec<_>>()
          .join(",\n"),
        Content::String => "String".to_string(),
        Content::Integer => "Int or Long".to_string(),
        Content::Number => "Float".to_string(),
        Content::Boolean => "Boolean".to_string(),
        Content::Array(content) => content_to_string(*content),
      }
    }

    method
      .request_body_opt
      .map(|request_body| format!("case class Class({})", content_to_string(request_body)))
  }

  pub fn generate_command_ts(method: Method) -> Option<String> {
    fn content_to_string(content: Content) -> String {
      match content {
        Content::Object(properties) => properties
          .into_iter()
          .map(|property| format!("{}: {}", property.key, content_to_string(property.value)))
          .collect::<Vec<_>>()
          .join(",\n"),
        Content::String => "string".to_string(),
        Content::Integer => "number".to_string(),
        Content::Number => "number".to_string(),
        Content::Boolean => "boolean".to_string(),
        Content::Array(content) => content_to_string(*content),
      }
    }

    method
      .request_body_opt
      .map(|request_body| format!("type Type={{{}}}", content_to_string(request_body)))
  }

  #[cfg(test)]
  mod tests {
    use super::*;
    extern crate yaml_rust;
    use yaml_rust::YamlLoader;

    #[test]
    fn it_from_yaml() {
      let yaml = "
        openapi: 3.0.0
        info:
          title: local
          version: '1.0'
        servers:
          - url: 'http://localhost:3000'
        paths:
          '/users/{userId}':
            parameters:
              - schema:
                  type: string
                name: userId
                in: path
                required: true
              
            get:
              summary: 候補者詳細GET
              tags: []
              responses:
                '200':
                  description: OK
                  headers: {}
                  content:
                    application/json:
                      schema:
                        type: object
                        properties:
                          hogeId:
                            type: string
                          foo:
                            type: integer
              operationId: get-users-userId
            put:
              summary: 候補者詳細PUT
              operationId: put-users-userId
              responses:
                '200':
                  description: OK
              requestBody:
                content:
                  application/json:
                    schema:
                      type: object
                      properties:
                        hasDateAndPlace:
                          type: string
                          maxLength: 100
                        location:
                          type: string
                          enum:
                            - S
                            - A
                            - B
                            - NG
        components:
          schemas: {}
            ";

      let docs = YamlLoader::load_from_str(yaml).unwrap();

      // Multi document support, doc is a yaml::Yaml
      let doc = &docs[0];

      let vec: Vec<Api> = vec![Api {
        path: "/users/{userId}".to_string(),
        param_map: hashmap! {"userId".to_string() => ParamType::String},
        method_map: hashmap! {
          "get".to_string() => Method{
            operation_id: "get-users-userId".to_string(),
            summary: "候補者詳細GET".to_string(),
            response_opt: Some(Content::Object(vec![
              Property{key: "hogeId".to_string(), value: Content::String},
              Property{key: "foo".to_string(), value: Content::Integer}
            ])),
           request_body_opt: None
           },
          "put".to_string() => Method{
            operation_id: "put-users-userId".to_string(),
            summary: "候補者詳細PUT".to_string(),
            response_opt:  None,
            request_body_opt:  Some(Content::Object(vec![
              Property{key: "hasDateAndPlace".to_string(), value: Content::String},
              Property{key: "location".to_string(), value: Content::String}
            ]))
          },
        },
      }];

      assert_eq!(vec, from_yaml(&doc));
    }
  }

  #[test]
  fn it_to_play_routings() {
    let api = Api {
      path: "/users/{userId}".to_string(),
      param_map: hashmap! {"userId".to_string() => ParamType::String},
      method_map: hashmap! {
        "get".to_string() => Method{
          operation_id: "get-users-userId".to_string(),
          summary: "候補者詳細GET".to_string(),
          response_opt: None,
         request_body_opt: None
         },
        "put".to_string() => Method{
          operation_id: "put-users-userId".to_string(),
          summary: "候補者詳細PUT".to_string(),
          response_opt:  None,
          request_body_opt: None
        },
      },
    };

    assert_eq!(
      vec![
        "GET /users/:userId {Method Name}(userId: String)",
        "PUT /users/:userId {Method Name}(userId: String)"
      ]
      .sort(),
      to_play_routings(api).sort()
    );
  }

  #[test]
  fn it_generate_command_scala() {
    let method = Method {
      operation_id: "put-users-userId".to_string(),
      summary: "候補者詳細PUT".to_string(),
      response_opt: None,
      request_body_opt: Some(Content::Object(vec![
        Property {
          key: "hasDateAndPlace".to_string(),
          value: Content::String,
        },
        Property {
          key: "location".to_string(),
          value: Content::String,
        },
      ])),
    };
    assert_eq!(
      Some("case class Class(hasDateAndPlace: String,\nlocation: String)".to_string()),
      generate_command_scala(method)
    )
  }
  #[test]
  fn it_generate_command_ts() {
    let method = Method {
      operation_id: "put-users-userId".to_string(),
      summary: "候補者詳細PUT".to_string(),
      response_opt: None,
      request_body_opt: Some(Content::Object(vec![
        Property {
          key: "hasDateAndPlace".to_string(),
          value: Content::String,
        },
        Property {
          key: "location".to_string(),
          value: Content::String,
        },
      ])),
    };
    assert_eq!(
      Some("type Type={hasDateAndPlace: string,\nlocation: string}".to_string()),
      generate_command_ts(method)
    )
  }
}

extern crate yaml_rust;
use std::env;
use std::fs;
use yaml_rust::YamlLoader;
extern crate regex;

fn main() -> std::io::Result<()> {
  let args: Vec<String> = env::args().collect();
  if args.len() > 0 {
    let yaml = fs::read_to_string(&*args[1])?;
    let docs = YamlLoader::load_from_str(&*yaml).unwrap();
    let doc = &docs[0];

    fs::remove_dir_all("dist")?;
    fs::create_dir("dist")?;
    let apis = apis::from_yaml(doc);
    for api in apis {
      let mut routes = apis::to_play_routings(api.clone());
      routes.sort();

      fs::write("dist/routes", routes.join("\n"))?;
      for ref method in api.method_map.values() {
        let m = method.clone();
        // create command
        let command_scala_opt = apis::generate_command_scala(m.clone());

        for command_scala in command_scala_opt.iter() {
          let ref dir = format!("dist/{}/command", m.clone().operation_id);
          fs::create_dir_all(dir)?;
          let file = format!("{}/{}.scala", dir, m.clone().operation_id);
          fs::write(file, command_scala)?;
        }

        let command_ts_opt = apis::generate_command_ts(m.clone());

        for command_ts in command_ts_opt.iter() {
          let ref dir = format!("dist/{}/command", m.clone().operation_id);
          fs::create_dir_all(dir)?;
          let file = format!("{}/{}.ts", dir, m.clone().operation_id);
          fs::write(file, command_ts)?;
        }

        // create view model
        let view_model_scala_opt = apis::generate_view_model_scala(m.clone());

        for view_model_scala in view_model_scala_opt.iter() {
          let ref dir = format!("dist/{}/viewmodel", m.clone().operation_id);
          fs::create_dir_all(dir)?;
          let file = format!("{}/{}.scala", dir, m.clone().operation_id);
          fs::write(file, view_model_scala)?;
        }

        let view_model_ts_opt = apis::generate_view_model_ts(m.clone());

        for view_model_ts in view_model_ts_opt.iter() {
          let ref dir = format!("dist/{}/viewmodel", m.clone().operation_id);
          fs::create_dir_all(dir)?;
          let file = format!("{}/{}.ts", dir, m.clone().operation_id);
          fs::write(file, view_model_ts)?;
        }
      }
    }

    Ok(())
  } else {
    panic!("command use: ./draco-open-api input.yaml")
  }
}

#[macro_use]
extern crate maplit;
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
    Date,
  }

  #[derive(PartialEq, Clone, Debug)]
  pub struct Property {
    pub key: String,
    pub value: Content,
    pub or_null: bool,
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
              .map(|key| {
                let prop_type = base_doument["properties"][key]["type"].clone();

                Property {
                  key: key.to_string(),
                  value: {
                    match prop_type.as_str() {
                      Some("string") => base_doument["properties"][key]["format"]
                        .clone()
                        .as_str()
                        .map(|format| {
                          if format == "date" {
                            Content::Date
                          } else {
                            panic!("unsupported format type: {}", format)
                          }
                        })
                        .unwrap_or(Content::String),
                      Some("integer") => Content::Integer,
                      Some("number") => Content::Number,
                      Some("boolean") => Content::Boolean,
                      // type is list
                      None => {
                        let prop_types = prop_type
                          .clone()
                          .into_iter()
                          .filter(|t| t.as_str() != Some("null"))
                          .collect::<Vec<_>>();
                        if prop_types.len() == 1 {
                          match prop_types[0].as_str() {
                            Some("string") => base_doument["properties"][key]["format"]
                              .clone()
                              .as_str()
                              .map(|format| {
                                if format == "date" {
                                  Content::Date
                                } else {
                                  panic!("unsupported format type: {}", format)
                                }
                              })
                              .unwrap_or(Content::String),
                            Some("integer") => Content::Integer,
                            Some("number") => Content::Number,
                            Some("boolean") => Content::Boolean,
                            _ => panic!(
                              "unsuppoted property type: ({}: {})",
                              key,
                              prop_types[0].as_str().unwrap_or("None")
                            ),
                          }
                        } else {
                          panic!("property type must have num of 2. info: {:?}", prop_types)
                        }
                      }
                      Some("object") | Some("array") => {
                        create_schema(base_doument["properties"][key].clone())
                          .expect("fail to create nested object")
                      }
                      _ => panic!(
                        "unsuppoted nested property type: ({}: {})",
                        key,
                        prop_type.as_str().unwrap_or("None")
                      ),
                    }
                  },
                  or_null: prop_type.into_iter().any(|t| t.as_str() == Some("null")),
                }
              })
              .collect::<Vec<_>>();

            Some(Content::Object(properties))
          } else if schema_type == "array" {
            create_schema(base_doument["items"].clone())
              .map(|items| Content::Array(Box::new(items)))
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
            "post" => "POST",
            "put" => "PUT",
            "delete" => "DELETE",
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

  fn head_uppercase(str: String) -> String {
    str[0..1].to_uppercase() + &str[1..]
  }

  fn content_to_string_scala(class_name: String, content: Content, is_command: bool) -> String {
    match content {
      Content::Object(properties) => {
        format!(
          "case class {}({})",
          class_name,
          properties
            .clone()
            .into_iter()
            .map(|property| {
              format!(
                "{}: {}",
                property.key,
                match property.value {
                  Content::Object(_) => head_uppercase(property.key.to_string()),
                  _ => content_to_string_scala("".to_string(), property.value, is_command),
                }
              )
            })
            .collect::<Vec<_>>()
            .join(",\n")
        ) + "\n"
          + &properties
            .into_iter()
            .filter(|property| match property.value {
              Content::Object(_) => true,
              _ => false,
            })
            .map(|property| {
              content_to_string_scala(
                head_uppercase(property.key.to_string()),
                property.value,
                is_command,
              )
            })
            .collect::<Vec<_>>()
            .join(",\n")
      }
      Content::String => "String".to_string(),
      Content::Integer => "Int or Long".to_string(),
      Content::Number => "Float".to_string(),
      Content::Boolean => "Boolean".to_string(),
      Content::Date => (if is_command {
        "ZonedDateTime"
      } else {
        "Instant"
      })
      .to_string(),
      Content::Array(content) => format!(
        "Seq[{}]",
        content_to_string_scala("".to_string(), *content, is_command)
      ),
    }
  }

  fn content_to_string_ts(type_name: String, content: Content) -> String {
    match content {
      Content::Object(properties) => {
        format!(
          "type {}={{{}}}",
          type_name,
          properties
            .clone()
            .into_iter()
            .map(|property| {
              format!(
                "{}: {}",
                property.key,
                match property.value {
                  Content::Object(_) => head_uppercase(property.key.to_string()),
                  _ => content_to_string_ts("".to_string(), property.value),
                }
              )
            })
            .collect::<Vec<_>>()
            .join(",\n")
        ) + "\n"
          + &properties
            .into_iter()
            .filter(|property| match property.value {
              Content::Object(_) => true,
              _ => false,
            })
            .map(|property| {
              content_to_string_ts(head_uppercase(property.key.to_string()), property.value)
            })
            .collect::<Vec<_>>()
            .join(",\n")
      }
      Content::String => "string".to_string(),
      Content::Integer => "number".to_string(),
      Content::Number => "number".to_string(),
      Content::Boolean => "boolean".to_string(),
      Content::Date => "Date".to_string(),
      Content::Array(content) => content_to_string_ts("".to_string(), *content) + "[]",
    }
  }

  pub fn generate_command_scala(method: Method) -> Option<String> {
    method
      .request_body_opt
      .map(|request_body| content_to_string_scala("Command".to_string(), request_body, true))
  }

  pub fn generate_command_ts(method: Method) -> Option<String> {
    method
      .request_body_opt
      .map(|request_body| content_to_string_ts("Command".to_string(), request_body))
  }

  pub fn generate_view_model_scala(method: Method) -> Option<String> {
    method
      .response_opt
      .map(|response| content_to_string_scala("ViewModel".to_string(), response, false))
  }

  pub fn generate_view_model_ts(method: Method) -> Option<String> {
    method
      .response_opt
      .map(|response| content_to_string_ts("ViewModel".to_string(), response))
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
              description: ''
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
                          type: boolean
                        foo:
                          type:
                            - integer
                            - 'null'
                        bar_at:
                          type: string
                          format: date
            operationId: get-users-userId
            description: 候補者詳細GET
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
                      location:
                        type: string
                        enum:
                          - S
                          - A
                          - B
                          - NG
            description: 候補者詳細PUT
        /users:
          get:
            summary: ユーザ取得
            tags: []
            responses:
              '200':
                description: OK
                content:
                  application/json:
                    schema:
                      type: array
                      items:
                        type: object
                        properties:
                          userId:
                            type: string
                          age:
                            type: integer
                          family:
                            type: object
                            properties:
                              name:
                                type: string
                              age:
                                type: integer
            operationId: get-users
            description: ユーザ取得
      operationId: get-users
      description: ユーザ取得
      components:
        schemas: {}                 
            ";

      let docs = YamlLoader::load_from_str(yaml).unwrap();

      // Multi document support, doc is a yaml::Yaml
      let doc = &docs[0];

      let vec: Vec<Api> = vec![
        Api {
          path: "/users/{userId}".to_string(),
          param_map: hashmap! {"userId".to_string() => ParamType::String},
          method_map: hashmap! {
            "get".to_string() => Method{
              operation_id: "get-users-userId".to_string(),
              summary: "候補者詳細GET".to_string(),
              response_opt: Some(Content::Object(vec![
                Property{key: "hogeId".to_string(), value: Content::Boolean, or_null: false},
                Property{key: "foo".to_string(), value: Content::Integer, or_null: true},
                Property{key: "bar_at".to_string(), value: Content::Date, or_null: false}
              ])),
             request_body_opt: None
             },
            "put".to_string() => Method{
              operation_id: "put-users-userId".to_string(),
              summary: "候補者詳細PUT".to_string(),
              response_opt:  None,
              request_body_opt:  Some(Content::Object(vec![
                Property{key: "hasDateAndPlace".to_string(), value: Content::String, or_null: false},
                Property{key: "location".to_string(), value: Content::String, or_null: false}
              ]))
            },
          },
        },
        Api {
          path: "/users".to_string(),
          param_map: HashMap::new(),
          method_map: hashmap! {
            "get".to_string() => Method{
              operation_id: "get-users".to_string(),
              summary: "ユーザ取得".to_string(),
              response_opt: Some(Content::Array(Box::new(Content::Object(vec![
                Property{key: "userId".to_string(), value: Content::String, or_null: false},
                Property{key: "age".to_string(), value: Content::Integer, or_null: false},
                Property{key: "family".to_string(), value: Content::Object(vec![
                  Property{key: "name".to_string(), value: Content::String, or_null: false},
                  Property{key: "age".to_string(), value: Content::Integer, or_null: false}
                ]), or_null: false}
              ])))),
             request_body_opt: None
             },
          },
        },
      ];

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

    let mut expected = vec![
      "GET /users/:userId {Method Name}(userId: String)",
      "PUT /users/:userId {Method Name}(userId: String)",
    ];

    let mut actual = to_play_routings(api);

    expected.sort();
    actual.sort();

    assert_eq!(expected, actual);
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
          or_null: false,
        },
        Property {
          key: "location".to_string(),
          value: Content::String,
          or_null: false,
        },
        Property {
          key: "idList".to_string(),
          value: Content::Array(Box::new(Content::String)),
          or_null: false,
        },
        Property {
          key: "familyCommand".to_string(),
          value: Content::Object(vec![
            Property {
              key: "name".to_string(),
              value: Content::String,
              or_null: false,
            },
            Property {
              key: "age".to_string(),
              value: Content::Integer,
              or_null: false,
            },
          ]),
          or_null: false,
        },
      ])),
    };
    assert_eq!(
      Some(
        "case class Command(hasDateAndPlace: String,\nlocation: String,\nidList: Seq[String],\nfamilyCommand: FamilyCommand)"
          .to_string()
          + "\ncase class FamilyCommand(name: String,\nage: Int or Long)\n"
      ),
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
          or_null: false,
        },
        Property {
          key: "location".to_string(),
          value: Content::String,
          or_null: false,
        },
        Property {
          key: "idList".to_string(),
          value: Content::Array(Box::new(Content::String)),
          or_null: false,
        },
        Property {
          key: "familyCommand".to_string(),
          value: Content::Object(vec![
            Property {
              key: "name".to_string(),
              value: Content::String,
              or_null: false,
            },
            Property {
              key: "age".to_string(),
              value: Content::Integer,
              or_null: false,
            },
          ]),
          or_null: false,
        },
      ])),
    };
    assert_eq!(
      Some(
        "type Command={hasDateAndPlace: string,\nlocation: string,\nidList: string[],\nfamilyCommand: FamilyCommand}".to_string()
        .to_string()
          + "\ntype FamilyCommand={name: string,\nage: number}\n"
      ),
      generate_command_ts(method)
    )
  }
}

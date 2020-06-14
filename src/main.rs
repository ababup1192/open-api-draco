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

  #[derive(PartialEq, Debug)]
  pub struct Api {
    pub path: String,
    pub param_map: HashMap<String, ParamType>,
  }

  #[derive(PartialEq, Debug)]
  pub enum ParamType {
    Integer,
    String,
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

    paths
      .as_hash()
      .unwrap()
      .keys()
      .map(|path| {
        let path = path.as_str().unwrap();
        Api {
          path: path.to_string(),
          param_map: paths[path]["parameters"]
            .clone()
            .into_iter()
            .map(create_param_tuple)
            .collect::<HashMap<_, _>>(),
        }
      })
      .collect()
  }
  pub fn to_play_path(api: Api) -> String {
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

    api
      .path
      .split("/")
      .map(|variable| modify_identifer(variable.to_string()))
      .collect::<Vec<String>>()
      .join("/")
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
                            type:
                              - 'null'
                              - integer
              operationId: get-users-userId
            put:
              summary: 候補者詳細POST
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
                        '': {}
        components:
          schemas: {}
            ";

      let docs = YamlLoader::load_from_str(yaml).unwrap();

      // Multi document support, doc is a yaml::Yaml
      let doc = &docs[0];

      let vec: Vec<Api> = vec![Api {
        path: "/users/{userId}".to_string(),
        param_map: hashmap! {"userId".to_string() => ParamType::String},
      }];

      assert!(vec == from_yaml(&doc));
    }
  }

  #[test]
  fn it_to_play_path() {
    let api = Api {
      path: "/users/{userId}".to_string(),
      param_map: HashMap::new(),
    };

    assert_eq!("/users/:userId", to_play_path(api));
  }
}

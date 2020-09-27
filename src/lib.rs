use context_based_variable_substitution::*;
use yaml_rust::Yaml;
use yaml_rust::YamlLoader;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::io::{ErrorKind, Error};

pub fn get_env_str(key: &str) -> Option<String> {
    match env::var(key) {
        Ok(s) => Some(s),
        Err(_) => None,
    }
}

pub fn get_yaml_type(yaml_obj: &Yaml) -> String {
    let s = match yaml_obj {
        Yaml::Real(_) => "real",
        Yaml::Integer(_) => "integer",
        Yaml::String(_) => "string",
        Yaml::Boolean(_) => "boolean",
        Yaml::Array(_) => "array",
        Yaml::Hash(_) => "object",
        Yaml::Alias(_) => "alias",
        Yaml::Null => "null",
        Yaml::BadValue => "BAD_YAML_VALUE",
    };
    s.into()
}

pub fn get_string_from_yaml_object(yaml_obj: &Yaml) -> Option<String> {
    let s = match yaml_obj {
        Yaml::Real(r) => r.clone(),
        Yaml::Integer(i) => i.to_string(),
        Yaml::String(s) => s.clone(),
        Yaml::Boolean(b) => b.to_string(),
        Yaml::Null => "null".into(),

        _ => return None,
        // TODO: is it possible to transclude in place
        // segments of yaml? say the user had something like:
        // custom:
        //    field: ${{ other.thing }}
        // other:
        //    thing:
        //       hello: world
        //
        // could that then return to custom.field.hello = world?
        // Yaml::Array(_) => "array",
        // Yaml::Hash(_) => "object",
        // Yaml::Alias(_) => "alias",
        // Yaml::BadValue => "BAD_YAML_VALUE",
    };
    s.into()
}


pub struct YamlContext<'a> {
    pub yaml: &'a Yaml,
}
impl<'a> Context for YamlContext<'a> {
    fn get_value_from_key(&self, key: &str) -> Option<String> {
        let key_split = key.split(".");
        let mut yobj = self.yaml;
        for k in key_split {
            if yobj.is_array() {
                // then we index as if k is a usize:
                if let Ok(k_usize) = k.parse::<usize>() {
                    yobj = &yobj[k_usize];
                    continue;
                }
            }
            yobj = &yobj[k];
        }
        if yobj.is_badvalue() {
            None
        } else {
            get_string_from_yaml_object(yobj)
        }
    }
}

pub struct ArgEnvContext<'a> {
    cli_args: &'a Vec<String>,
}
impl<'a> Context for ArgEnvContext<'a> {
    fn get_value_from_key(&self, key: &str) -> Option<String> {
        if key.starts_with("ENV:") {
            // we pass a slice to not pass the actual
            // 'ENV:' prefix
            let env_name = &key[4..];
            return get_env_str(env_name);
        } else {
            // otherwise, try to use the cli args to get
            // an argument via index
            self.cli_args.get_value_from_key(key)
        }
    }
}

pub fn load_yaml_from_str(
    yaml_str: &str,
) -> Result<Vec<Yaml>, Error> {
    let yaml_doc = match YamlLoader::load_from_str(&yaml_str) {
        Ok(d) => d,
        Err(e) => {
            let err_kind = ErrorKind::InvalidInput;
            let err_msg = format!("Failed to parse yaml file:\n{}", e);
            return Err(Error::new(err_kind, err_msg));
        },
    };
    if yaml_doc.len() == 0 {
        let err_kind = ErrorKind::InvalidInput;
        let err_msg = format!("Cannot proceed with empty yaml file");
        return Err(Error::new(err_kind, err_msg));
    }

    Ok(yaml_doc)
}

// given a yaml text as a string, perform substitutions
// first via the cli and environemnt variables context
// and then again with the context of the yaml object
// this second pass allows yaml fields to reference each other
pub fn read_yaml_string_from_string(
    yaml_str: &str,
    cli_args: Vec<String>,
) -> Result<String, Error> {
    let arg_and_env_context = ArgEnvContext {
        cli_args: &cli_args,
    };
    // first pass:
    // we give it the context of the cli args, and env vars
    // and ignore all else. We fill in the dynamic vars here
    let mut yaml_out_str = replace_all_from(
        &yaml_str,
        &arg_and_env_context,
        FailureMode::FM_ignore,
    );
    let yaml_doc = load_yaml_from_str(&yaml_out_str)?;
    // and after that, we create a temporary, dummy, yaml context
    // to be used to fill in the rest of the variable references
    // using the filled in context from the envs and args above
    // ie: ${{ custom.somevar.arg1 }} if the yaml is like:
    // custom:
    //    somevar: { name: hello }
    // this means we create a yaml object twice, so maybe not the most efficient...
    let yaml_context = YamlContext {
        yaml: &yaml_doc[0],
    };
    // this time we panic if we fail to find the variable the
    // user is looking for
    yaml_out_str = replace_all_from(
        &yaml_out_str,
        &yaml_context,
        FailureMode::FM_panic,
    );
    Ok(yaml_out_str)
}

// given a path to a file (and cli args for context)
// return a yaml object with variables substituted according
// to the cli, env, and other yaml variable context
pub fn read_yaml_from_file(
    file_path: &str,
    cli_args: Vec<String>,
) -> Result<Vec<Yaml>, Error> {
    let mut file = File::open(file_path)?;
    let mut yaml_str = String::new();
    file.read_to_string(&mut yaml_str)?;
    let yaml_str = read_yaml_string_from_string(yaml_str.as_str(), cli_args)?;
    let yaml_doc = load_yaml_from_str(&yaml_str)?;
    Ok(yaml_doc)
}

// same as read_yaml_from_file, but instead of returning the
// yaml rust object, return just the final string with all the
// variables substituted
pub fn read_yaml_string_from_file(
    file_path: &str,
    cli_args: Vec<String>,
) -> Result<String, Error> {
    let mut file = File::open(file_path)?;
    let mut yaml_str = String::new();
    file.read_to_string(&mut yaml_str)?;
    read_yaml_string_from_string(yaml_str.as_str(), cli_args)
}


#[cfg(test)]
mod tests {
    use super::*;
    const TEST_FILE: &str = "test.yml";
    const TEST_COMMENTS_FILE: &str = "test_comments.yml";
    // not a very thorough test suite, but im not 100% sure
    // of how i want this lib to work, so i just provide one
    // test of the most basic functionality so that i dont
    // accidentally break this in the future:
    #[test]
    fn works() {
        std::env::set_var("TITLE", "BEAUTIFUL");
        let cli_arg_context = vec!["some_arg".into(), "other_arg".into()];
        let my_yaml_docs = read_yaml_from_file(TEST_FILE, cli_arg_context).unwrap();
        let my_yaml_doc = &my_yaml_docs[0];
        // test that we can reference other variables in the yaml
        assert_eq!(
            my_yaml_doc["something"]["is"]["here"].as_str().unwrap(),
            "b"
        );
        // test that we can reference cli args via their position
        assert_eq!(
            my_yaml_doc["something"]["is"]["and"].as_str().unwrap(),
            "also here: some_arg"
        );
        // test that we can reference environment variables
        assert_eq!(
            my_yaml_doc["title"].as_str().unwrap(),
            "hello BEAUTIFUL world"
        );
        // test that defaults work
        assert_eq!(
            my_yaml_doc["segments"][3].as_str().unwrap(),
            "default if arg not provided"
        );
    }

    #[test]
    fn getting_doc_as_string_works() {
        std::env::set_var("TITLE", "BEAUTIFUL");
        let cli_arg_context = vec!["some_arg".into(), "other_arg".into()];
        let my_yaml_string = read_yaml_string_from_file(TEST_FILE, cli_arg_context).unwrap();

        assert!(my_yaml_string.contains("title: hello BEAUTIFUL world"));
        assert!(my_yaml_string.contains("and: \"also here: some_arg\""));
        assert!(my_yaml_string.contains("here: b"));
        assert!(my_yaml_string.contains("some_setting: default if arg not provided"));
    }

    #[test]
    #[ignore = "not implemented yet... need to not try to substitute commented variables"]
    fn variables_in_comments_dont_cause_errors() {
        let cli_arg_context = vec!["some_arg".into()];
        let my_yaml_docs = read_yaml_from_file(TEST_COMMENTS_FILE, cli_arg_context).unwrap();
        let my_yaml_doc = &my_yaml_docs[0];
    }
}

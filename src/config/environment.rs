use crate::utils::Error;
use float_eq::float_eq;
use std::{collections::{HashMap, HashSet}, env, path::Path, str::FromStr, sync::{Arc, RwLock}};
use url::Url;
use lazy_static::lazy_static;

use super::{OpenapiSource, RudraConfig, Runtime};

const ENV_VAR_APP_BASE_URL: &str = "RUDRA_APP_BASE_URL";
const ENV_VAR_DEBUG: &str = "RUDRA_DEBUG";
const ENV_VAR_OPENAPI_SOURCE: &str = "RUDRA_OPENAPI_SOURCE";
const ENV_VAR_ACCOUNT_FOR_FORBIDDEN: &str = "RUDRA_ACCOUNT_FOR_FORBIDDEN";
const ENV_VAR_ACCOUNT_FOR_UNAUTORIZED: &str = "RUDRA_ACCOUNT_FOR_UNAUTORIZED";
const ENV_VAR_TEST_COVERAGE: &str = "RUDRA_TEST_COVERAGE";
const ENV_VAR_PORT: &str = "RUDRA_PORT";
const ENV_VAR_MAPPING: &str = "RUDRA_MAPPING";

const DEFAULT_TEST_COVERAGE: f32 = 0.7;
const DEFAULT_PORT: u16 = 13750;

const MAPPING_SEPERATOR: &str = "RUDRA_LINE_SEPERATOR";
const MAPPING_SUBDELIMITER: &str = ";";

lazy_static! {
    static ref IS_DEBUG: RwLock<bool> = RwLock::new(true);
}

impl RudraConfig {
    pub fn from_raw(env_vars: &HashMap<String, String>) -> Result<RudraConfig, Error> {
        // Check if all enviroment variables exist
        if !key_exists_and_is_not_empty(ENV_VAR_MAPPING, env_vars)
            && (!key_exists_and_is_not_empty(ENV_VAR_OPENAPI_SOURCE, env_vars)
                || !key_exists_and_is_not_empty(ENV_VAR_APP_BASE_URL, env_vars))
        {
            return Err(Error::MissingConfiguration);
        }
        if key_exists_and_is_not_empty(ENV_VAR_MAPPING, env_vars)
            && (key_exists_and_is_not_empty(ENV_VAR_PORT, env_vars)
                || key_exists_and_is_not_empty(ENV_VAR_OPENAPI_SOURCE, env_vars)
                || key_exists_and_is_not_empty(ENV_VAR_APP_BASE_URL, env_vars))
        {
            return Err(Error::ConflictingConfiguration);
        }

        // fetch values from enviroment variables
        let debug = get_bool_env_var(ENV_VAR_DEBUG, env_vars);
        let security_accounts_for_forbidden = get_bool_env_var(ENV_VAR_ACCOUNT_FOR_FORBIDDEN, env_vars);
        let security_accounts_for_unautorized = get_bool_env_var(ENV_VAR_ACCOUNT_FOR_UNAUTORIZED, env_vars);
        let test_coverage = match env_vars.get(ENV_VAR_TEST_COVERAGE) {
            Some(coverage_str) => translate_test_coverage(coverage_str)?,
            None => 0.7,
        };

        let runtimes = if !key_exists_and_is_not_empty(ENV_VAR_MAPPING, env_vars) {
            let openapi_source_str = match env_vars.get(ENV_VAR_OPENAPI_SOURCE) {
                Some(openapi_source) => openapi_source,
                None => return Err(Error::MissingConfiguration),
            };
            let app_base_url_str = match env_vars.get(ENV_VAR_APP_BASE_URL) {
                Some(openapi_source) => openapi_source,
                None => return Err(Error::MissingConfiguration),
            };
            let port_str = match env_vars.get(ENV_VAR_PORT) {
                Some(port_str) => if port_str == "" {
                    None
                } else {
                    Some(port_str.as_str())
                },
                None => None,
            };
            vec![
                parse_runtime(openapi_source_str, app_base_url_str, port_str)?
            ]
        } else {
            let mapping_str = match env_vars.get(ENV_VAR_MAPPING) {
                Some(mapping_str) => mapping_str,
                None => return Err(Error::MissingMapping),
            };
            parse_complex_mapping(mapping_str)?
        };

        // adjust global debug setting
        if let Ok(mut is_debug) = IS_DEBUG.write() {
            *is_debug = debug
        } else {
            return Err(Error::UnknownInternalError("debug double write".to_string()))
        }

        Ok(RudraConfig {
            debug,
            security_accounts_for_forbidden,
            security_accounts_for_unautorized,
            test_coverage,
            runtimes,
        })
    }

    pub fn from_env() -> Result<RudraConfig, Error> {
        let mut env_vars = HashMap::new();
        for var in env::vars() {
            env_vars.insert(var.0, var.1);
        }
        RudraConfig::from_raw(&env_vars)
    }

    pub fn global_is_debug() -> bool {
        // unwrap is okay, since there should only be one write operaion
        *IS_DEBUG.read().unwrap()
    }
}

fn key_exists_and_is_not_empty(key: &str, env_vars: &HashMap<String, String>) -> bool {
    match env_vars.get(key) {
        Some(content) => content != "",
        None => false,
    }
}

fn parse_complex_mapping(mapping_str: &str) -> Result<Vec<Arc<Runtime>>, Error> {
    let mut runtimes = vec![];

    for line in mapping_str.split(MAPPING_SEPERATOR) {
        // ignore empty lines that might consist out of tabs or spaces
        if line.trim() == "" {
            continue;
        }
        let index = 0;

        let (app_base_url_str, index) = parse_untill_mapping_subdelimiter(index, &line)?;
        let (openapi_source_str, index) = parse_untill_mapping_subdelimiter(index, &line)?;
        let (port_str, _) = parse_untill_mapping_subdelimiter(index, &line)?;

        let app_base_url_str = replace_escaped_sequences(app_base_url_str);
        let openapi_source_str = replace_escaped_sequences(openapi_source_str);
        let port_str = replace_escaped_sequences(port_str);

        runtimes.push(parse_runtime(&openapi_source_str, &app_base_url_str, Some(&port_str))?);
    }
    if runtimes.len() == 0 {
        return Err(Error::MissingMapping)
    }
    if !check_runtime_compatability(&runtimes) {
        println!("{:?}", runtimes);
        return Err(Error::MappingDuplicatePorts);
    }
    Ok(runtimes)
}

fn parse_untill_mapping_subdelimiter<'a>(index: usize, base: &'a str) -> Result<(&'a str, usize), Error> {
    let mut final_index = index;
    let mut is_escaped = false;
    while is_escaped || match base.get(final_index..final_index+1) {
        Some(MAPPING_SUBDELIMITER) => false,
        Some(_) => true,
        None => false,
    } {
        if base.get(final_index..final_index+1) == Some("\\") {
            is_escaped = true;
        } else {
            is_escaped = false;
        }
        final_index += 1;
    }
    final_index += 1;
    if base.len() < final_index {
        return Err(Error::MappingMissingSemicolon(base.to_string()))
    }
    match base.get(index..final_index - 1) {
        Some(subpart) => Ok((subpart, final_index)),
        None => Err(Error::MappingMissingSemicolon(base.to_string()))
    }
}

fn check_runtime_compatability(runtimes: &Vec<Arc<Runtime>>) -> bool {
    let mut ports = HashSet::new();
    for runtime in runtimes {
        if !ports.insert(runtime.port) {
            return false;
        }
    }
    true
}

fn replace_escaped_sequences(base: &str) -> String {
    base.replace("\\;", ";")
}

fn parse_runtime(openapi_source_str: &str, app_base_url_str: &str, port_str: Option<&str>) -> Result<Arc<Runtime>, Error> {
    let openapi_source = match Url::from_str(openapi_source_str.trim()) {
        Ok(openapi_url) => OpenapiSource::Url(openapi_url),
        Err(_) => OpenapiSource::Path(Box::from(Path::new(openapi_source_str.trim()))),
    };
    let app_base_url = match Url::from_str(app_base_url_str.trim()) {
        Ok(app_base_url) => app_base_url,
        Err(parse_error) => return Err(Error::InvalidApplicationURL(parse_error.to_string())),
    };

    let port = match port_str {
        Some(port_str) => match port_str.trim().parse() {
            Ok(port) => port,
            Err(_) => return Err(Error::InvalidPortNumber(String::from(port_str))),
        },
        _ => DEFAULT_PORT,
    };

    Ok(Arc::from(Runtime{openapi_source, app_base_url, port}))
}

fn get_bool_env_var(key: &str, env_vars: &HashMap<String, String>) -> bool {
    match env_vars.get(key) {
        Some(bool_var) => {
            bool_var.as_str() != "0" && bool_var.as_str() != "" && bool_var.as_str() != "false"
        }
        None => false,
    }
}

fn translate_test_coverage(coverage_str: &str) -> Result<f32, Error> {
    if coverage_str.trim() == "" {
        return Ok(DEFAULT_TEST_COVERAGE);
    }
    let mut coverage = if coverage_str.trim().ends_with("%") {
        match coverage_str[0..coverage_str.len() - 1].parse() {
            Ok(coverage) => coverage,
            Err(_) => return Err(Error::InvalidTestCoverage),
        }
    } else {
        match coverage_str.parse() {
            Ok(coverage) => coverage,
            Err(_) => return Err(Error::InvalidTestCoverage),
        }
    };
    if coverage > 1.0 {
        coverage /= 100.0;
    }
    if float_eq!(coverage, 0.0, abs <= 0.0001) {
        println!("Warning: test coverage is set to 0%");
    }

    if coverage > 1.0 || coverage < 0.0 {
        Err(Error::InvalidTestCoverage)
    } else {
        Ok(coverage)
    }
}

#[cfg(test)]
mod test {
    use float_eq::assert_float_eq;
    use std::{collections::HashMap, path::Path};

    use crate::config::{
        environment::{
            get_bool_env_var, key_exists_and_is_not_empty, translate_test_coverage,
            DEFAULT_TEST_COVERAGE, ENV_VAR_MAPPING, ENV_VAR_PORT, parse_complex_mapping, replace_escaped_sequences, ENV_VAR_ACCOUNT_FOR_UNAUTORIZED,
        },
        OpenapiSource,
    };

    use super::{RudraConfig, ENV_VAR_APP_BASE_URL, ENV_VAR_DEBUG, ENV_VAR_OPENAPI_SOURCE, parse_untill_mapping_subdelimiter, ENV_VAR_ACCOUNT_FOR_FORBIDDEN};

    fn generate_config_map() -> HashMap<String, String> {
        let mut config_map = HashMap::new();

        config_map.insert(String::from(ENV_VAR_DEBUG), String::from("1"));
        config_map.insert(
            String::from(ENV_VAR_OPENAPI_SOURCE),
            String::from("./test/resource/swagger.json"),
        );
        config_map.insert(
            String::from(ENV_VAR_APP_BASE_URL),
            String::from("http://localhost:8080"),
        );
        config_map
    }

    #[test]
    fn can_fetch_valid_openapi_path() {
        let config_map = generate_config_map();
        assert_eq!(
            RudraConfig::from_raw(&config_map).unwrap().runtimes[0].openapi_source,
            OpenapiSource::Path(Box::from(Path::new("./test/resource/swagger.json")))
        );
    }

    #[test]
    fn can_fetch_valid_url() {
        assert_eq!(
            RudraConfig::from_raw(&generate_config_map())
                .unwrap()
                .runtimes[0]
                .app_base_url
                .as_str(),
            "http://localhost:8080/"
        );
    }

    #[test]
    fn can_catch_invalid_url() {
        let mut config_map = generate_config_map();
        config_map.insert(ENV_VAR_APP_BASE_URL.to_string(), String::from("jjjjjj"));
        match RudraConfig::from_raw(&config_map) {
            Ok(_) => panic!("Should throw error here"),
            Err(_) => (),
        }
    }

    #[test]
    fn missing_keys_lead_to_err() {
        let mut config_map = generate_config_map();
        config_map.remove(ENV_VAR_APP_BASE_URL);
        assert!(RudraConfig::from_raw(&config_map).is_err());
    }

    #[test]
    fn nonzero_bool_is_true() {
        let mut config_map = generate_config_map();
        assert!(get_bool_env_var(ENV_VAR_DEBUG, &config_map));
        config_map.insert(ENV_VAR_DEBUG.to_string(), String::from("2"));
        assert!(get_bool_env_var(ENV_VAR_DEBUG, &config_map));
    }

    #[test]
    fn zero_or_empty_bool_is_false() {
        let mut config_map = generate_config_map();

        config_map.insert(ENV_VAR_DEBUG.to_string(), String::from("0"));
        assert!(!get_bool_env_var(ENV_VAR_DEBUG, &config_map));

        config_map.insert(ENV_VAR_DEBUG.to_string(), String::from(""));
        assert!(!get_bool_env_var(ENV_VAR_DEBUG, &config_map));
    }

    #[test]
    fn non_existant_bool_is_false_no_error() {
        let mut config_map = generate_config_map();
        config_map.remove(ENV_VAR_DEBUG);
        assert!(!get_bool_env_var(ENV_VAR_DEBUG, &config_map));
    }

    #[test]
    fn debug_val_is_used() {
        let config_map = generate_config_map();
        assert!(RudraConfig::from_raw(&config_map).unwrap().debug);
    }

    #[test]
    fn account_for_security_val_is_used() {
        let config_map = generate_config_map();
        assert!(
            !RudraConfig::from_raw(&config_map)
                .unwrap()
                .security_accounts_for_forbidden
        );
    }

    #[test]
    fn test_coverage_translator_can_recognise_float() {
        assert_float_eq!(
            translate_test_coverage("0.86").unwrap(),
            0.86,
            abs <= 0.0001
        );
    }

    #[test]
    fn test_coverage_recognises_percentage_with_sign() {
        assert_float_eq!(translate_test_coverage("86%").unwrap(), 0.86, abs <= 0.0001);
        assert_float_eq!(
            translate_test_coverage("85.5%").unwrap(),
            0.855,
            abs <= 0.0001
        );
    }

    #[test]
    fn test_coverage_recognises_percentage_without_sign() {
        assert_float_eq!(translate_test_coverage("86").unwrap(), 0.86, abs <= 0.0001);
    }

    #[test]
    fn test_coverage_throws_error_if_over_100_percent() {
        assert!(translate_test_coverage("866").is_err());
    }

    #[test]
    fn test_coverage_throws_error_if_invalid_number() {
        assert!(translate_test_coverage("foo%").is_err());
    }

    #[test]
    fn test_coverage_empty_sting_leads_to_default() {
        assert_eq!(translate_test_coverage("").unwrap(), DEFAULT_TEST_COVERAGE);
    }

    #[test]
    fn defaults_to_70_percent_test_coverage() {
        let config_map = generate_config_map();
        assert_float_eq!(
            RudraConfig::from_raw(&config_map).unwrap().test_coverage,
            0.7,
            abs <= 0.0001
        );
    }

    #[test]
    fn configuration_defaults_to_port_13750() {
        let config_map = generate_config_map();
        assert_eq!(
            RudraConfig::from_raw(&config_map).unwrap().runtimes[0].port,
            13750
        );
    }

    #[test]
    fn configuration_recognises_port_number() {
        let mut config_map = generate_config_map();
        config_map.insert(ENV_VAR_PORT.to_string(), "9999".to_string());
        assert_eq!(
            RudraConfig::from_raw(&config_map).unwrap().runtimes[0].port,
            9999
        );
    }

    #[test]
    fn configuration_throws_error_for_invalid_port() {
        let mut config_map = generate_config_map();
        config_map.insert(ENV_VAR_PORT.to_string(), "albert".to_string());
        assert!(RudraConfig::from_raw(&config_map).is_err());
        config_map.insert(ENV_VAR_PORT.to_string(), "65537".to_string()); // 2^ 16 + 1 (tcp only
                                                                          // allows 16 bits)
        assert!(RudraConfig::from_raw(&config_map).is_err());
    }

    #[test]
    fn throws_error_if_any_parallel_configuration_with_mapping_is_donw() {
        let mut config_map = generate_config_map();

        config_map.insert(
            ENV_VAR_MAPPING.to_string(),
            "https://localhost:8090; docs/swagger1.yaml; 13751;".to_string(),
        );
        assert!(RudraConfig::from_raw(&config_map).is_err());

        config_map.remove(ENV_VAR_APP_BASE_URL);
        assert!(RudraConfig::from_raw(&config_map).is_err());

        config_map.remove(ENV_VAR_OPENAPI_SOURCE);
        config_map.insert(
            String::from(ENV_VAR_APP_BASE_URL),
            String::from("http://localhost:8080"),
        );
        assert!(RudraConfig::from_raw(&config_map).is_err());

        config_map.remove(ENV_VAR_APP_BASE_URL);
        config_map.insert(ENV_VAR_PORT.to_string(), "8080".to_string()); // 2^ 16 + 1 (tcp only
        assert!(RudraConfig::from_raw(&config_map).is_err());
    }

    #[test]
    fn throws_error_if_no_configuration_or_mapping_is_provided() {
        let config_map = HashMap::new();
        assert!(RudraConfig::from_raw(&config_map).is_err());
    }

    #[test]
    fn can_recognise_if_env_var_is_empty() {
        let mut config_map = HashMap::new();
        const KEY: &str = "KEY";
        config_map.insert(KEY.to_string(), "".to_string());
        assert!(!key_exists_and_is_not_empty(KEY, &config_map));
    }

    #[test]
    fn can_recognise_if_env_var_is_not_empty() {
        let mut config_map = HashMap::new();
        const KEY: &str = "KEY";
        config_map.insert(KEY.to_string(), "test".to_string());
        assert!(key_exists_and_is_not_empty(KEY, &config_map));
    }

    #[test]
    fn can_recognise_if_env_var_doesnt_exist() {
        let config_map = HashMap::new();
        const KEY: &str = "KEY";
        assert!(!key_exists_and_is_not_empty(KEY, &config_map));
    }

    #[test]
    fn passing_in_basic_parameters_leads_to_default_runtime_being_initialized() {
        let mut config_map = generate_config_map();
        config_map.insert(ENV_VAR_PORT.to_string(), "8080".to_string());
        let config = RudraConfig::from_raw(&config_map).unwrap();

        assert_eq!(config.runtimes.len(), 1);
        assert_eq!(config.runtimes[0].port, 8080);
    }

    #[test]
    fn parses_basic_mapping() {
        let runtimes = parse_complex_mapping("https://localhost:8090; docs/swagger1.yaml; 13751;RUDRA_LINE_SEPERATORhttps://example:8091; docs/swagger2.yaml; 13752;").unwrap();
        assert_eq!(runtimes.len(), 2);

        assert!(runtimes.iter().any(|x| x.port == 13751));
        assert!(runtimes.iter().any(|x| x.port == 13752));

        assert!(runtimes.iter().any(|x| x.openapi_source == OpenapiSource::Path(Box::from(Path::new("docs/swagger1.yaml")))));
        assert!(runtimes.iter().any(|x| x.openapi_source == OpenapiSource::Path(Box::from(Path::new("docs/swagger2.yaml")))));

        assert!(runtimes.iter().any(|x| x.app_base_url.as_str() == "https://localhost:8090/"));
        assert!(runtimes.iter().any(|x| x.app_base_url.as_str() == "https://example:8091/"));
    }

    #[test]
    fn allows_different_whitespace_ammounts() {
        let runtimes = parse_complex_mapping("\n   https://localhost:8090; docs/swagger1.yaml     ; 13751   ;\n\n");
        assert!(runtimes.is_ok())
    }

    #[test]
    fn allows_escaping_of_semicolon() {
        let runtimes = parse_complex_mapping(r"https://localhost:8090; docs/swagger\;1.yaml; 13751;").unwrap();
        assert_eq!(runtimes[0].openapi_source, OpenapiSource::Path(Box::from(Path::new("docs/swagger;1.yaml"))));
    }

    #[test]
    fn mapping_gets_recognised_in_happy_case() {
        let mut config_map = HashMap::new();
        config_map.insert(
            ENV_VAR_MAPPING.to_string(), 
            "https://localhost:8090; docs/swagger1.yaml; 13751;RUDRA_LINE_SEPERATORhttps://example:8091; docs/swagger2.yaml; 13752;".to_string()
        );
        let config = RudraConfig::from_raw(&config_map).unwrap();
        assert_eq!(config.runtimes.len(), 2)
    }

    #[test]
    fn duplicate_ports_lead_to_error_in_mapping() {
        let mut config_map = HashMap::new();
        config_map.insert(
            ENV_VAR_MAPPING.to_string(), 
            "https://localhost:8090; docs/swagger1.yaml; 13751;RUDRA_LINE_SEPERATORhttps://example:8091; docs/swagger2.yaml; 13751;".to_string()
        );
        assert!(RudraConfig::from_raw(&config_map).is_err());
    }

    #[test]
    fn parses_till_limit() {
        let test_str = "test test; 123";
        let index = 0;
        let result = parse_untill_mapping_subdelimiter(index, test_str).unwrap();

        assert_eq!(result.0, "test test");
    }

    #[test]
    fn skips_over_escaped_chars() {
        let test_str = "test\\; \\test; 123";
        let index = 0;
        let result = parse_untill_mapping_subdelimiter(index, test_str).unwrap();

        assert_eq!(result.0, "test\\; \\test");

    }

    #[test]
    fn returns_error_if_missing_delimiter() {
        let test_str = "test test";
        let index = 0;
        let result = parse_untill_mapping_subdelimiter(index, test_str);
        assert!(result.is_err());
    }

    #[test]
    fn remove_escaped_sequences_replaces_escape_sequences() {
        let test_str = "test\\; \\test; 123";
        assert_eq!(replace_escaped_sequences(test_str), "test; \\test; 123");

    }

    #[test]
    fn remove_escaped_sequences_ignored_unescaped() {
        let test_str = "test; \\test; 123";
        assert_eq!(replace_escaped_sequences(test_str), test_str);
    }

    #[test]
    fn regocnises_forbidden_security_flag_in_config() {
        let mut env_vars = generate_config_map();
        env_vars.insert(ENV_VAR_ACCOUNT_FOR_FORBIDDEN.to_string(), "1".to_string());

        let config = RudraConfig::from_raw(&env_vars).unwrap();
        assert!(config.security_accounts_for_forbidden);
    }

    #[test]
    fn regocnises_unautorized_security_flag_in_config() {
        let mut env_vars = generate_config_map();
        env_vars.insert(ENV_VAR_ACCOUNT_FOR_UNAUTORIZED.to_string(), "1".to_string());

        let config = RudraConfig::from_raw(&env_vars).unwrap();
        assert!(config.security_accounts_for_unautorized);
    }

    #[test]
    fn security_is_off_by_default() {
        let env_vars = generate_config_map();
        let config = RudraConfig::from_raw(&env_vars).unwrap();
        assert!(!config.security_accounts_for_unautorized);
        assert!(!config.security_accounts_for_forbidden);
    }
}

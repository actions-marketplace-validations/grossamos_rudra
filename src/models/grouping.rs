use super::{EndpointConfiguration, Method, OpenapiPath};

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct Grouping {
    methods: Vec<Method>,
    status: Vec<u16>,
    path: OpenapiPath,
}

impl Grouping {
    pub fn incompases_endpoint_config(&self, endpoint: &EndpointConfiguration) -> bool {
        self.methods.contains(&endpoint.method)
            && self.status.contains(&endpoint.status_code)
            && self.path.incompases_openapi_path(&endpoint.path)
    }
}

#[cfg(test)]
mod tests {
    use std::{str::FromStr, sync::Arc};

    use crate::{models::{Method, OpenapiPath, EndpointConfiguration}, utils::test::create_mock_runtime};

    use super::Grouping;

    #[test]
    fn grouping_detects_incompased_endpoint() {
        let grouping = Grouping {
            methods: vec![Method::GET],
            status: vec![200],
            path: OpenapiPath::from_str("/foo/{bar}").unwrap(),
        };
        let endpoint= EndpointConfiguration::new(
            Method::GET,
            "/foo/69",
            200,
            Arc::from(create_mock_runtime()),
            false,
        )
        .unwrap();

        assert!(grouping.incompases_endpoint_config(&endpoint));
    }

    #[test]
    fn different_status_leads_to_not_incompased() {
        let grouping = Grouping {
            methods: vec![Method::POST],
            status: vec![418],
            path: OpenapiPath::from_str("/foo/{bar}").unwrap(),
        };
        let endpoint= EndpointConfiguration::new(
            Method::GET,
            "/foo/69",
            200,
            Arc::from(create_mock_runtime()),
            false,
        )
        .unwrap();

        assert!(!grouping.incompases_endpoint_config(&endpoint));
    }

    #[test]
    fn different_method_leads_to_not_incompased() {
        let grouping = Grouping {
            methods: vec![Method::POST],
            status: vec![200],
            path: OpenapiPath::from_str("/foo/{bar}").unwrap(),
        };
        let endpoint= EndpointConfiguration::new(
            Method::GET,
            "/foo/69",
            200,
            Arc::from(create_mock_runtime()),
            false,
        )
        .unwrap();

        assert!(!grouping.incompases_endpoint_config(&endpoint));
    }
}

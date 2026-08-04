use arc_admin_backend::{API_PREFIX, API_ROUTE_CONTRACT};
use axum::http::Uri;
use serde::de::IgnoredAny;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Deserialize)]
struct OpenApiDocument {
    servers: Vec<OpenApiServer>,
    paths: BTreeMap<String, BTreeMap<String, IgnoredAny>>,
}

#[derive(Deserialize)]
struct OpenApiServer {
    url: String,
}

#[test]
fn openapi_operations_match_backend_contract() {
    let specification_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../docs/openapi.yaml");
    let specification = std::fs::read_to_string(&specification_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", specification_path.display()));
    let document: OpenApiDocument =
        serde_saphyr::from_str(&specification).expect("parse OpenAPI YAML");
    assert!(
        !document.servers.is_empty(),
        "OpenAPI has at least one server"
    );
    for server in &document.servers {
        let uri = server
            .url
            .parse::<Uri>()
            .unwrap_or_else(|error| panic!("parse OpenAPI server URL {:?}: {error}", server.url));
        assert_eq!(uri.path(), API_PREFIX, "OpenAPI server path drift");
    }

    let http_methods = ["get", "post", "put", "patch", "delete"];
    let mut documented = BTreeSet::new();
    for (path, operations) in document.paths {
        for method in http_methods {
            if operations.contains_key(method) {
                documented.insert((format!("{API_PREFIX}{path}"), method.to_string()));
            }
        }
    }

    let expected = API_ROUTE_CONTRACT
        .iter()
        .flat_map(|(path, methods)| {
            methods
                .iter()
                .map(|method| ((*path).to_string(), (*method).to_string()))
        })
        .collect::<BTreeSet<_>>();

    assert_eq!(documented, expected, "OpenAPI route contract drift");
}

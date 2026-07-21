use uhura_host::{
    CheckedRoutePattern, ClientCandidate, PlayAdmissionRejection, RoutePathClaim, RoutePathDecode,
    RoutePathScope,
};

const ROUTE_RULE: &str = "spock/reserved-client-route";
const ROUTE_CODE: &str = "SPK1001";

const SPOCK_ROUTE_CLAIMS: &[(RoutePathClaim<'static>, &str)] = &[
    (
        RoutePathClaim {
            path: "/api",
            scope: RoutePathScope::Namespace,
            decode: RoutePathDecode::PercentDecodedOnce,
        },
        "/api",
    ),
    (
        RoutePathClaim {
            path: "/graphql",
            scope: RoutePathScope::Namespace,
            decode: RoutePathDecode::PercentDecodedOnce,
        },
        "/graphql",
    ),
    (
        RoutePathClaim {
            path: "/rest",
            scope: RoutePathScope::Namespace,
            decode: RoutePathDecode::PercentDecodedOnce,
        },
        "/rest",
    ),
    (
        RoutePathClaim {
            path: "/storage",
            scope: RoutePathScope::Namespace,
            decode: RoutePathDecode::PercentDecodedOnce,
        },
        "/storage",
    ),
    (
        RoutePathClaim {
            path: "/~",
            scope: RoutePathScope::Prefix,
            decode: RoutePathDecode::PercentDecodedOnce,
        },
        "/~*",
    ),
];

/// One semantically checked Uhura route claimed by the aggregate Spock host
/// before the application history adapter can see it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClientRouteCollision {
    pub table: String,
    pub constructor: String,
    pub pattern: String,
    pub namespace: &'static str,
}

impl ClientRouteCollision {
    pub(crate) const fn code(&self) -> &'static str {
        ROUTE_CODE
    }

    pub(crate) const fn rule(&self) -> &'static str {
        ROUTE_RULE
    }

    pub(crate) fn message(&self) -> String {
        format!(
            "checked route table `{}` maps constructor `{}` to pattern `{}`, which overlaps Spock-owned namespace `{}`; framework protocol routes are resolved before the Uhura application router",
            self.table, self.constructor, self.pattern, self.namespace
        )
    }
}

/// Apply only Spock composition policy to Uhura's already-checked semantic
/// route view. Source parsing, aliases, constant lowering, and route validity
/// remain exclusively owned by Uhura.
pub(crate) fn checked_client_route_collisions(
    routes: Option<&[CheckedRoutePattern]>,
) -> Vec<ClientRouteCollision> {
    let Some(routes) = routes else {
        return Vec::new();
    };
    routes
        .iter()
        .filter_map(|route| {
            let (_, namespace) = SPOCK_ROUTE_CLAIMS
                .iter()
                .find(|(claim, _)| route.overlaps(*claim))?;
            Some(ClientRouteCollision {
                table: route.table().to_string(),
                constructor: route.constructor().to_string(),
                pattern: route.display_pattern().to_string(),
                namespace,
            })
        })
        .collect()
}

/// Apply Spock's aggregate-host ownership policy before an Uhura candidate is
/// published. The rejection remains part of that coherent candidate, allowing
/// Editor to advance while Uhura atomically retains last-good Play.
pub(crate) fn apply_checked_client_route_admission(
    candidate: &mut ClientCandidate,
) -> Vec<ClientRouteCollision> {
    let collisions = checked_client_route_collisions(candidate.checked_route_patterns());
    for collision in &collisions {
        candidate.reject_play_admission(PlayAdmissionRejection::new(
            ROUTE_CODE,
            ROUTE_RULE,
            collision.message(),
        ));
    }
    collisions
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use tempfile::tempdir;
    use uhura_host::{build_candidate, capture_project_snapshot};

    use super::*;

    fn write_project(root: &Path, pattern: &str) {
        fs::write(
            root.join("uhura.toml"),
            r#"[project]
name = "test.routes"
version = 1
language = "0.4"

[modules]
app = "app.uhura"
"#,
        )
        .unwrap();
        fs::write(
            root.join("app.uhura"),
            format!(
                r#"use uhura::web_router::{{Router, Routes as WebRoutes}};

pub enum Location {{
  Page,
}}

pub const ROUTES: WebRoutes<Location> = WebRoutes::from([
  ("Page", "{pattern}"),
]);

pub machine App {{
  port router = Router<Location> {{ routes: ROUTES }};

  outcomes {{
    commit Accepted,
  }}

  on router.Changed(location) {{
    Accepted
  }}
}}
"#
            ),
        )
        .unwrap();
        fs::write(
            root.join("host.toml"),
            r#"[entry.app]
machine = "crate::App"
lifetime = "application-session"

[entry.app.ports]
router = "web.history"
"#,
        )
        .unwrap();
    }

    #[test]
    fn composition_policy_consumes_the_checked_candidate_route_view() {
        for (pattern, namespace) in [
            ("/api", "/api"),
            ("/api%2Fshadow", "/api"),
            ("/graphql/v2", "/graphql"),
            ("/graphql%2Fv2", "/graphql"),
            ("/rest/v2/items", "/rest"),
            ("/rest%2Fv2/items", "/rest"),
            ("/storage/v2/object", "/storage"),
            ("/storage%2Fv2/object", "/storage"),
        ] {
            let root = tempdir().unwrap();
            write_project(root.path(), pattern);
            let candidate = build_candidate(&capture_project_snapshot(root.path()), 1);
            assert!(
                candidate.summary().play_ok,
                "{pattern} diagnostics: {:#}",
                candidate.diagnostics().play
            );
            let collisions = checked_client_route_collisions(candidate.checked_route_patterns());
            assert_eq!(collisions.len(), 1, "{pattern}");
            assert_eq!(collisions[0].table, "test.routes@1::ROUTES");
            assert_eq!(collisions[0].constructor, "Page");
            assert_eq!(collisions[0].pattern, pattern);
            assert_eq!(collisions[0].namespace, namespace);
        }
    }

    #[test]
    fn similarly_prefixed_checked_routes_remain_application_owned() {
        for pattern in ["/apiary", "/graphical", "/restroom", "/storage-unit"] {
            let root = tempdir().unwrap();
            write_project(root.path(), pattern);
            let candidate = build_candidate(&capture_project_snapshot(root.path()), 1);
            assert!(candidate.summary().play_ok, "{pattern}");
            assert!(
                checked_client_route_collisions(candidate.checked_route_patterns()).is_empty(),
                "{pattern}"
            );
        }
    }
}

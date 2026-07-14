/// Exclusive owner selected before any subsystem fallback runs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouteOwner {
    Framework,
    Authority,
    Client,
    ProtocolNotFound,
    NotFound,
}

/// Classify one URI path into the combined host's non-overlapping route map.
///
/// Query strings are transport metadata and must be removed by the caller.
/// Reserved protocol namespaces never reach the client's history fallback.
#[must_use]
pub fn classify_route(path: &str, client_configured: bool) -> RouteOwner {
    let Some(decoded) = decode_for_ownership(path) else {
        return RouteOwner::ProtocolNotFound;
    };
    let path = decoded.as_str();

    if path == "/~health" || path == "/~project" || path.starts_with("/~project/") {
        return RouteOwner::Framework;
    }

    if authority_path(path) {
        return RouteOwner::Authority;
    }

    if client_configured && client_path(path) {
        return RouteOwner::Client;
    }

    if reserved_protocol_path(path) {
        return RouteOwner::ProtocolNotFound;
    }

    if client_configured {
        RouteOwner::Client
    } else if path == "/" {
        RouteOwner::Framework
    } else {
        RouteOwner::NotFound
    }
}

/// Decode exactly one percent-encoding layer for namespace ownership only.
///
/// The original URI is still passed to the owning subsystem. This prevents an
/// encoded spelling of a reserved protocol namespace from reaching the client
/// history fallback without changing asset identity or decoding twice.
fn decode_for_ownership(path: &str) -> Option<String> {
    if !path.as_bytes().contains(&b'%') {
        return (!path.contains('\\') && !path.contains('\0')).then(|| path.to_string());
    }
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            decoded.push(bytes[index]);
            index += 1;
            continue;
        }
        let high = *bytes.get(index + 1)?;
        let low = *bytes.get(index + 2)?;
        decoded.push(hex_value(high)? << 4 | hex_value(low)?);
        index += 3;
    }
    let decoded = String::from_utf8(decoded).ok()?;
    (!decoded.contains('\\') && !decoded.contains('\0')).then_some(decoded)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn authority_path(path: &str) -> bool {
    path == "/~contract"
        || path == "/~personas"
        || path == "/~whoami"
        || path == "/~studio"
        || path.starts_with("/~studio/")
        || path == "/graphql/v1"
        || path.starts_with("/graphql/v1/")
        || path == "/rest/v1"
        || path.starts_with("/rest/v1/")
        || path == "/storage/v1"
        || path.starts_with("/storage/v1/")
}

fn client_path(path: &str) -> bool {
    path == "/"
        || path == "/play"
        || path.starts_with("/play/")
        || path == "/favicon.ico"
        || path == "/assets"
        || path.starts_with("/assets/")
        || path == "/api/editor"
        || path.starts_with("/api/editor/")
        || path == "/api/play"
        || path.starts_with("/api/play/")
}

fn reserved_protocol_path(path: &str) -> bool {
    path.starts_with("/~")
        || ["/api", "/graphql", "/rest", "/storage"]
            .iter()
            .any(|prefix| path == *prefix || path.starts_with(&format!("{prefix}/")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_surfaces_have_one_owner() {
        let cases = [
            ("/~health", RouteOwner::Framework),
            ("/~project/status", RouteOwner::Framework),
            ("/~studio", RouteOwner::Authority),
            ("/~studio/assets/app.js", RouteOwner::Authority),
            ("/~contract", RouteOwner::Authority),
            ("/~personas", RouteOwner::Authority),
            ("/graphql/v1", RouteOwner::Authority),
            ("/rest/v1/rpc/do-thing", RouteOwner::Authority),
            ("/storage/v1/object/id", RouteOwner::Authority),
            ("/", RouteOwner::Client),
            ("/play", RouteOwner::Client),
            ("/assets/app.js", RouteOwner::Client),
            ("/api/editor/state", RouteOwner::Client),
            ("/api/play/ir.json", RouteOwner::Client),
            ("/profile/mira", RouteOwner::Client),
        ];
        for (path, expected) in cases {
            assert_eq!(classify_route(path, true), expected, "{path}");
        }
    }

    #[test]
    fn unknown_protocol_paths_never_reach_the_client_spa() {
        for path in [
            "/~unknown",
            "/api/unknown",
            "/graphql/v2",
            "/rest/v2/users",
            "/storage/v2/object",
        ] {
            assert_eq!(
                classify_route(path, true),
                RouteOwner::ProtocolNotFound,
                "{path}"
            );
        }
    }

    #[test]
    fn backend_only_root_is_framework_owned_and_other_pages_are_not_found() {
        assert_eq!(classify_route("/", false), RouteOwner::Framework);
        assert_eq!(classify_route("/profile/mira", false), RouteOwner::NotFound);
        assert_eq!(
            classify_route("/api/play/ir.json", false),
            RouteOwner::ProtocolNotFound
        );
    }

    #[test]
    fn similarly_prefixed_non_protocol_names_can_be_client_routes() {
        for path in ["/apiary", "/restroom", "/graphical", "/storage-unit"] {
            assert_eq!(classify_route(path, true), RouteOwner::Client, "{path}");
        }
    }

    #[test]
    fn encoded_reserved_namespaces_never_reach_the_client_spa() {
        for path in [
            "/%61pi/unknown",
            "/%7Eunknown",
            "/graphql%2Fv2",
            "/re%73t/v2/users",
            "/storage%2fv2/object",
            "/api/%00bad",
            "/api/%GG",
        ] {
            assert_eq!(
                classify_route(path, true),
                RouteOwner::ProtocolNotFound,
                "{path}"
            );
        }
    }
}

use std::io::Write;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::auth::{resolve_api_base_url, resolve_api_key};
use crate::utils::api_get;

#[derive(Deserialize, Serialize)]
struct TeamResponse {
    team_id: i64,
    team_name: String,
}

pub fn run(
    api_key: Option<&str>,
    api_base_url: Option<&str>,
    mut writer: impl Write,
) -> Result<()> {
    let api_key = resolve_api_key(api_key)?;
    let api_base_url = resolve_api_base_url(api_base_url);
    let url = format!("{api_base_url}/v1/team");
    let mut response = api_get(&url, &api_key, &[])?;
    let team: TeamResponse = response.body_mut().read_json()?;
    serde_json::to_writer(&mut writer, &team)?;
    writeln!(writer)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils::parse_ndjson;

    #[test]
    fn test_run() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("GET", "/v1/team")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"team_id":1,"team_name":"Test Team"}"#)
            .create();

        let mut buf = Vec::new();
        run(Some("test-key"), Some(&server.url()), &mut buf).unwrap();
        mock.assert();

        let rows = parse_ndjson(&buf);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["team_id"], 1);
        assert_eq!(rows[0]["team_name"], "Test Team");
    }
}

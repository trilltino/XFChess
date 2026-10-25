use anyhow::Context as _;
use braid_http::{protocol::format_update, BraidClient, BraidRequest, Patch, Update, Version};
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    time::Duration,
};

struct Node(Child);
impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
#[ignore = "Requires Node.js and BRAID_JS_SOURCE pointing to an upstream braid-http checkout"]
async fn javascript_and_rust_exchange_snapshots_and_range_patches() -> anyhow::Result<()> {
    let source = std::env::var("BRAID_JS_SOURCE").context("set BRAID_JS_SOURCE")?;
    let script = r#"
const http = require('http'), path = require('path');
global.window = {fetch: global.fetch};
const root = process.argv[1];
const {braidify} = require(path.join(root, 'braid-http-server.js'));
const {parse_update} = require(path.join(root, 'braid-http-client.js'));
const server = http.createServer(async (req, res) => {
  if (req.url === '/parse') {
    const chunks = [];
    for await (const chunk of req) chunks.push(chunk);
    const result = parse_update({input: new Uint8Array(Buffer.concat(chunks))});
    res.setHeader('Content-Type', 'application/json');
    res.end(JSON.stringify({version: result.version, body: result.body && [...result.body],
      patches: result.patches && result.patches.map(p => ({unit: p.unit, range: p.range, content: [...p.content]}))}));
    return;
  }
  braidify(req, res);
  res.startSubscription();
  await res.sendUpdate({version: ['snapshot'], body: Buffer.from([255, 254])});
  await res.sendUpdate({version: ['single'], parents: ['snapshot'], patch: {unit: 'json', range: '.fen', content: '"ok"'}});
  await res.sendUpdate({version: ['multi'], parents: ['single'], patches: [
    {unit: 'bytes', range: '[0:1]', content: Buffer.from([0, 255])},
    {unit: 'text', range: '[1:2]', content: '\u2603'}]});
  res.end();
});
server.listen(0, '127.0.0.1', () => console.log('http://127.0.0.1:' + server.address().port));
"#;
    let mut node = Node(
        Command::new("node")
            .args(["-e", script, &source])
            .stdout(Stdio::piped())
            .spawn()
            .context("required operation value")?,
    );
    let mut url = String::new();
    BufReader::new(node.0.stdout.take().context("required operation value")?)
        .read_line(&mut url)
        .context("required operation value")?;
    let url = url.trim();
    assert!(url.starts_with("http://127.0.0.1:"));
    let client = BraidClient::new().context("required operation value")?;
    let mut subscription = client
        .subscribe(url, BraidRequest::new())
        .await
        .context("required operation value")?;
    let mut updates = Vec::new();
    while let Some(update) = tokio::time::timeout(Duration::from_secs(5), subscription.next())
        .await
        .context("required operation value")?
    {
        updates.push(update.context("required operation value")?);
    }
    assert_eq!(updates.len(), 3);
    assert!(updates.iter().all(|update| update.status == 200));
    assert_eq!(
        updates[0]
            .body
            .as_ref()
            .context("required operation value")?
            .as_ref(),
        &[255, 254]
    );
    let single = updates[1]
        .patches
        .as_ref()
        .context("required operation value")?;
    assert_eq!(single[0].unit, "json");
    assert_eq!(single[0].range, ".fen");
    assert_eq!(single[0].content.as_ref(), b"\"ok\"");
    let multi = updates[2]
        .patches
        .as_ref()
        .context("required operation value")?;
    assert_eq!(multi.len(), 2);
    assert_eq!(multi[0].content.as_ref(), &[0, 255]);
    assert_eq!(multi[1].content_str(), Some("\u{2603}"));

    for update in [
        Update::snapshot(Version::new("rust-snapshot"), vec![0, 255]),
        Update::patched(
            Version::new("rust-patches"),
            vec![
                Patch::json(".fen", "\"ok\""),
                Patch::bytes("[0:1]", vec![0, 255]),
            ],
        ),
    ] {
        let wire = format_update(&update).context("required operation value")?;
        let response: serde_json::Value = client
            .client()
            .post(format!("{url}/parse"))
            .body(wire)
            .send()
            .await
            .context("required operation value")?
            .json()
            .await
            .context("required operation value")?;
        assert_eq!(
            response["version"][0].as_str(),
            Some(update.version[0].to_string().as_str())
        );
        if let Some(body) = update.body {
            assert_eq!(response["body"], serde_json::json!(body.to_vec()));
        }
        if let Some(patches) = update.patches {
            let expected: Vec<_> = patches.iter().map(|patch| serde_json::json!({"unit": patch.unit, "range": patch.range, "content": patch.content.to_vec()})).collect();
            assert_eq!(response["patches"], serde_json::json!(expected));
        }
    }
    Ok(())
}

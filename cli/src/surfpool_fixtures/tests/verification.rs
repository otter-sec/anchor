use {
    super::*,
    crate::{Pubkey, RpcClient},
    serde_json::{json, Map, Value},
    std::{
        io::{BufRead, BufReader, Read, Write},
        net::TcpListener,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        thread,
        time::{Duration, Instant},
    },
};

#[test]
fn skipped_or_changed_snapshot_accounts_fail_verification() {
    use {
        crate::{RpcClient, RpcRequest},
        std::collections::HashMap,
    };
    let root = tempfile::tempdir().unwrap();
    let address = Pubkey::new_unique();
    let expected = account(address, "base64");
    let image = super::super::accounts::image(&expected).unwrap();
    let path = root.path().join("snapshot.json");
    fs::write(&path, json!({address.to_string():image}).to_string()).unwrap();
    let external = root.path().join("external.json");
    fs::write(
        &external,
        json!({Pubkey::new_unique().to_string():null}).to_string(),
    )
    .unwrap();
    let external_flags = vec![
        "--no-deploy".into(),
        "--snapshot".into(),
        external.display().to_string(),
    ];
    verify(&RpcClient::new_mock("succeeds"), &external_flags).unwrap();
    let flags = vec![
        "--snapshot".into(),
        path.display().to_string(),
        "--snapshot".into(),
        external.display().to_string(),
    ];
    for field in [
        None,
        Some("owner"),
        Some("lamports"),
        Some("data"),
        Some("executable"),
        Some("missing"),
        Some("short-response"),
    ] {
        let mut actual = expected["account"].clone();
        match field {
            Some("owner") => actual["owner"] = json!(Pubkey::new_unique().to_string()),
            Some("lamports") => actual["lamports"] = json!(43),
            Some("data") => actual["data"] = json!(["BAUG", "base64"]),
            Some("executable") => actual["executable"] = json!(true),
            Some("missing") => actual = Value::Null,
            Some("short-response") => {}
            None => {}
            other => panic!("unexpected field {other:?}"),
        }
        let mocks = HashMap::from([(
            RpcRequest::GetMultipleAccounts,
            json!({"context":{"slot":1},"value":if field == Some("short-response") { vec![] } else { vec![actual] }}),
        )]);
        let client = RpcClient::new_mock_with_mocks("succeeds", mocks);
        let result = verify(&client, &flags);
        if field.is_none() {
            result.unwrap();
        } else {
            assert!(result.unwrap_err().to_string().contains("FIXTURE_MISMATCH"));
        }
    }
}

#[test]
fn verification_batches_accounts_with_bounded_requests() {
    let root = tempfile::tempdir().unwrap();
    let image = json!({"owner":"11111111111111111111111111111111", "lamports":1,
        "executable":false,"rentEpoch":0,"data":"","parsedData":null});
    let images: Map<String, Value> = (0..101)
        .map(|_| (Pubkey::new_unique().to_string(), image.clone()))
        .collect();
    let path = root.path().join("snapshot.json");
    std::fs::write(&path, serde_json::to_vec(&images).unwrap()).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let finished = Arc::new(AtomicBool::new(false));
    let stop = finished.clone();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(15);
        while !stop.load(Ordering::Acquire) {
            assert!(Instant::now() < deadline, "RPC test exceeded its deadline");
            let (stream, _) = match listener.accept() {
                Ok(connection) => connection,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(error) => panic!("accept RPC request: {error}"),
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut reader = BufReader::new(stream);
            let mut length = None;
            loop {
                let mut line = String::new();
                assert!(reader.read_line(&mut line).unwrap() > 0);
                if line == "\r\n" {
                    break;
                }
                if let Some((name, value)) = line.split_once(':') {
                    if name.eq_ignore_ascii_case("content-length") {
                        length = Some(value.trim().parse::<usize>().unwrap());
                    }
                }
            }
            let mut body = vec![0; length.expect("RPC content length")];
            reader.read_exact(&mut body).unwrap();
            let request: Value = serde_json::from_slice(&body).unwrap();
            let account = json!({"owner":"11111111111111111111111111111111", "lamports":1,
                "executable":false,"rentEpoch":0,"data":["","base64"]});
            let value = match request["method"].as_str().unwrap() {
                "getAccountInfo" => {
                    requests.push(1);
                    account
                }
                "getMultipleAccounts" => {
                    let count = request["params"][0].as_array().unwrap().len();
                    requests.push(count);
                    Value::Array(vec![account; count])
                }
                method => panic!("unexpected RPC method {method}"),
            };
            let response = json!({"jsonrpc":"2.0","id":request["id"],"result":{"context":{"slot":1},"value":value}}).to_string();
            let mut stream = reader.into_inner();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: \
                 {}\r\nConnection: close\r\n\r\n{response}",
                response.len()
            )
            .unwrap();
        }
        requests
    });
    let result = verify(
        &RpcClient::new(url),
        &["--snapshot".into(), path.display().to_string()],
    );
    finished.store(true, Ordering::Release);
    let requests = server.join().unwrap();
    result.unwrap();
    assert_eq!(requests, vec![100, 1]);
}

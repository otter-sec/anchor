use {
    super::verify,
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
fn review_verification_batches_accounts_with_bounded_requests() {
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

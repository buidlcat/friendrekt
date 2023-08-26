use serde::{Deserialize, Serialize};

pub struct FastHttp {
    pub rpc: String,
    pub client: reqwest::Client,
}

#[derive(Serialize, Debug)]
struct Params {
    jsonrpc: String,
    method: String,
    params: Vec<String>,
    id: i32,
}

#[derive(Deserialize, Debug)]
struct JSONError {
    code: i32,
    message: String,
}

#[derive(Deserialize, Debug)]
struct Ret {
    jsonrpc: String,
    id: i32,
    result: Option<String>,
    error: Option<JSONError>,
}

impl FastHttp {
    pub fn new(rpc: String) -> Self {
        FastHttp {
            client: reqwest::Client::new(),
            rpc,
        }
    }
    pub async fn send_request(&self, request: String) -> Option<String> {
        let request_params = Params {
            jsonrpc: "2.0".to_string(),
            method: "eth_sendRawTransaction".to_string(),
            params: vec![request],
            id: 1,
        };
        let res = self
            .client
            .post(&self.rpc)
            .json(&request_params)
            .send()
            .await;
        match res {
            Ok(res) => {
                let data: Result<Ret, _> = res.json().await;
                match data {
                    Ok(data) => {
                        if data.result.is_some() {
                            Some(data.result.unwrap())
                        } else if data.error.is_some() {
                            println!("Error: {:?}", data.error.unwrap());
                            return None;
                        } else {
                            println!("Unknown error");
                            return None;
                        }
                    }
                    Err(_) => None,
                }
            }
            Err(_) => None,
        }
    }
}

use futures::{future, Future, Stream};
use hyper::{
    client::HttpConnector, rt, service::service_fn, Body, Client, Request,
    Response, Server
};
use reqwest::blocking::Client;
use serde_json::{json, Value};

fn send_discord_webhook(webhook_url: &str, message: &str) -> Result<(), reqwest::Error> {
    let client = Client::new();

    // Create a JSON payload with the message content
    let payload = json!({
        "content": message,
        "embeds": [
            {
                "title": "This is a test embed",
                "description": "This is a test description",
                "color": 15258703,
                "footer": {
                    "text": "This is a test footer"
                },
                "author": {
                    "name": "This is a test author"
                },
                "fields": [
                    {
                        "name": "This is a test field",
                        "value": "This is a test value",
                        "inline": false
                    }
                ]
            }
        ]
    });

    // Send a POST request to the Discord webhook URL with the payload
    let response = client.post(webhook_url)
        .json(&payload)
        .send()?;

    // Check if the request was successful (status code 200 OK)
    if response.status().is_success() {
        println!("Message sent successfully!");
        Ok(())
    } else {
        // Print the error response if the request was not successful
        let error_response: Value = response.json()?;
        println!("Failed to send message. Discord API error: {:?}", error_response);
        Err(reqwest::Error::new(reqwest::StatusCode::from_u16(response.status().as_u16()).unwrap(), "Discord API error"))
    }
}

// - Route Handlers

fn four_oh_four() -> ResponseFuture {
    let body = Body::from(NOTFOUND);
    Box::new(future::ok(
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(body)
            .unwrap(),
    ))
}

// - the webhooks_build function will receive the POST request from Vercel and Hop.io about CI information builds so it must determine whether the build comes from Vercel or Hop.io and send the appropriate message to Discord.
fn webhooks_build(req: Request<Body>) -> ResponseFuture {
    // - Get the body of the request
    let body = req.into_body();

    // - Convert the body into a Vec<u8> and then convert that into a String
    let full_body = body.concat2().wait().unwrap();
    let full_body_string = String::from_utf8(full_body.to_vec()).unwrap();

    // - Parse the body string into a JSON object
    let json_body: Value = serde_json::from_str(&full_body_string).unwrap();

    println!("JSON body: {:?}", json_body);

    // - If request contains header x-vercel-signature, it is from Vercel
    if let Some(vercel_signature) = req.headers().get("x-vercel-signature") {
        // - Get the value of the x-vercel-signature header
        let vercel_signature = vercel_signature.to_str().unwrap();

        // - Get the value of the VERCEL_WEBHOOK_SECRET environment variable
        let vercel_webhook_secret = env::var("VERCEL_WEBHOOK_SECRET").unwrap();

        // - Create a HMAC-SHA256 hasher
        let mut hasher = Hmac::<Sha256>::new_varkey(vercel_webhook_secret.as_bytes()).unwrap();

        // - Hash the body of the request
        hasher.input(full_body_string.as_bytes());

        // - Get the hash digest
        let hmac = hasher.result();

        // - Convert the hash digest to a hex string
        let hmac_hex = hex::encode(hmac.code());

        // - If the x-vercel-signature header matches the hash digest, the request is from Vercel
        if hmac_hex == vercel_signature {
            // - Get the value of the VERCEL_WEBHOOK_URL environment variable
            let vercel_webhook_url = env::var("PRODUCTION_BUILDS_WEBHOOK_URL").unwrap();

            // - Create a message with the build information
            //TODO

            // - Send a message to Discord with the webhook URL and message
            send_discord_webhook(&vercel_webhook_url, &message_content).unwrap();
        }
    }
}

// - Router

fn router(req: Request<Body>, _client: &Client<HttpConnector>) -> ResponseFuture {
    // - Mattern match for both the method and the path of the request
    match (req.method(), req.uri().path()) {
        // POST handlers
        (&Method::POST, "/webhooks/build") => webhooks_build(req),
        // - Anything else handler
        _ => four_oh_four(),
    }
}

// - Main

fn main() {
    pretty_env_logger::init();

    let addr = "127.0.0.1:8000".parse().unwrap();

    rt::run(future::lazy(move || {
        // - Create a Client for all Services
        let client = Client::new();

        // - Define a service containing the router function
        let new_service = move || {
            // - Move a clone of Client into the service_fn
            let client = client.clone();
            service_fn(move |req| router(req, &client))
        };

        // - Define the server - this is what the future_lazy() we're building will resolve to
        let server = Server::bind(&addr)
            .serve(new_service)
            .map_err(|e| eprintln!("Server error: {}", e));

        println!("Listening on http://{}", addr);
        server
    }));
}

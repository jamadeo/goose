use serde::Serialize;
use warp::Filter;

#[derive(Serialize)]
struct FetchNameResponse {
    name: String,
}

#[tokio::main]
async fn main() {
    let fetch_name = warp::path!("fetch_name").map(|| {
        let response = FetchNameResponse {
            name: "stub_system_name".to_string(),
        };
        warp::reply::json(&response)
    });

    println!("Stub server running at http://127.0.0.1:3000");
    warp::serve(fetch_name).run(([127, 0, 0, 1], 3000)).await;
}

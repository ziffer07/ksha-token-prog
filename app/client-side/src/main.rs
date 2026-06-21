use axum::{Router, response::{Html, IntoResponse, Response}, routing::get};
use askama::Template;

struct HtmlTemplate<T> (T);
impl <T> IntoResponse for HtmlTemplate<T>
where 
    T: Template,
{  
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(html) => Html(html).into_response(),
            Err(_) => "Internal Server Error".into_response(),
        }
    }
}

#[derive(Template)]
#[template(path="home.html")]
struct HomeTemplate{}


async fn home_handler() -> impl IntoResponse {
    let template = HomeTemplate{};
    HtmlTemplate(template)
}

fn create_router() -> Router {
    Router::new().route("/", get(home_handler))
}


#[tokio::main]
async fn main() {

    let app = create_router();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    println!("Listening on http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();

    
}
mod pipeline;
mod database;
mod frontend;
mod application;

#[cfg(test)]
mod tests {
    use std::fmt::{Error};

    use hyper::{Response};
    use mongodb::bson::doc;
    use routerify::prelude::RequestExt;
    use serde::{Deserialize, Serialize};

    use crate::application::App;
    use crate::database::Database;
    use crate::frontend::Frontend;
    use crate::pipeline::{Pipeline, Request};
    use crate::pipeline::Combine;

    #[derive(Serialize, Deserialize)]
    struct User {
        pub name: String,
    }

    async fn auth_filter(_: App, req: &Request) -> Result<User, String> {
        let user = User {
            name: String::from(req.headers().get("User-Agent").unwrap().to_str().unwrap())
        };
        Ok(user)
    }

    fn date_time() -> String {
        let dt: String = chrono::Local::now().format("%Y-%m-%d][%H:%M:%S").to_string();
        dt
    }

    #[tokio::test]
    async fn it_works() {
        let get_user = move |app, req| async move {
            Pipeline::receive(req, app)
                .then(|_, req| async move {
                    req.param("id").map(|x| x.clone())
                })
                .if_none(("No id provided".into(), 400))
                .then(|app, oid| async move {
                    app
                        .database()
                        .retrieve_one::<User>(String::from("users"), doc! {"id":oid})
                        .await
                })
                .catch(|err| {
                    (format!("Database error: {}", err.to_string()), 500)
                })
                .if_none(("User not found".into(), 404))
                .then(|_, object| async move {
                    serde_json::to_string(&object)
                })
                .catch(|_| {
                    ("Serialization error", 500)
                })
                .finish().await
        };

        let hello = move |app, req| async move {
            Pipeline::receive(req, app)
                .then(|a, req| async move {
                    auth_filter(a, &req).await.adding(req)
                })
                .catch(|err| {
                    (format!("Database error: {}", err.to_string()), 401)
                })
                .then(|_, (u, req)| async move {
                    (date_time(), u)
                })
                .then(|_, (time, u)| async move {
                    "Hello, ".to_owned() + u.name.as_str() + "! It is currently: " + &*time
                })
                .finish().await
        };
        let db = Database::new("mongodb://localhost:27017").await;
        let app = App::new_with_database(db);
        let frontend = Frontend { app, router_builder: Default::default() };
        frontend
            // .get("/user/:id", get_user)
            .get("/hello", hello)
            .get("/users/:id", get_user)
            .launch().await.unwrap()
    }
}
